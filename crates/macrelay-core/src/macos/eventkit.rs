use anyhow::Result;

use super::escape::escape_applescript_string;

/// List all calendars using AppleScript.
pub async fn list_calendars() -> Result<Vec<CalendarInfo>> {
    let script = r#"
        tell application "Calendar"
            set output to ""
            repeat with c in calendars
                set output to output & name of c & "|" & writable of c & linefeed
            end repeat
            return output
        end tell
    "#;

    let output = crate::macos::applescript::run_applescript(script)?;
    let mut calendars = Vec::new();
    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split('|').collect();
        let title = parts.first().unwrap_or(&"").to_string();
        let writable = parts.get(1).map(|s| *s == "true").unwrap_or(false);
        calendars.push(CalendarInfo {
            title,
            calendar_type: "local".to_string(),
            allows_modify: writable,
        });
    }
    Ok(calendars)
}

/// Search calendar events in `[start_ts, end_ts]` using EventKit.
///
/// The previous implementation used AppleScript with `every event of c
/// whose start date >= now and start date <= endDate`, iterated across
/// every calendar. Calendar.app's scripting bridge has no real index over
/// event dates, so each `whose` evaluation is an O(N) linear scan over the
/// calendar's full history — measured at 147 seconds for a single calendar
/// containing years of events, even when zero events match the date range.
/// Across 11 calendars the total elapsed reached 240+ seconds, far beyond
/// the 30s subprocess timeout, with no way for the user to disambiguate
/// "slow query" from "stuck permission dialog."
///
/// EventKit talks directly to Calendar's underlying SQLite store with a
/// proper indexed predicate. Same data, milliseconds instead of minutes,
/// scales to any history size.
///
/// We run the synchronous EventKit calls inside `spawn_blocking` so we
/// don't park a tokio executor thread on the framework call. The store
/// is short-lived (constructed and dropped within this function) — that's
/// fine for read-only queries and avoids any cross-thread concerns with
/// EKEventStore's notification observers.
///
/// `start_ts` and `end_ts` are Unix epoch seconds. Negative values (events
/// before 1970) and far-future values are accepted — `NSDate` happily
/// represents anything an `f64` can express; no clamping needed.
pub async fn search_events_in_range(
    start_ts: i64,
    end_ts: i64,
    query: Option<&str>,
) -> Result<Vec<EventInfo>> {
    let query_owned = query.map(|s| s.to_lowercase());
    tokio::task::spawn_blocking(move || {
        search_events_eventkit_blocking(start_ts, end_ts, query_owned)
    })
    .await
    .map_err(|e| anyhow::anyhow!("spawn_blocking join failed: {e}"))?
}

/// Fetch (start, end) busy intervals — used by `find_available_times`.
///
/// Same EventKit query as `search_events_in_range`, but stripped down to
/// raw epoch-second pairs since the free-slot calculator only needs the
/// occupancy timeline. Avoids round-tripping through `EventInfo` and the
/// `NSDateFormatter` string formatting we don't use here.
pub async fn fetch_busy_intervals(start_ts: i64, end_ts: i64) -> Result<Vec<(i64, i64)>> {
    tokio::task::spawn_blocking(move || fetch_busy_intervals_blocking(start_ts, end_ts))
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking join failed: {e}"))?
}

/// Open an `EKEventStore`, complete the macOS 14+ Full Access handshake,
/// and check authorization. Common preamble shared by both EventKit
/// query paths. On `AccessRequestOutcome::TimedOut` we surface a
/// Calendar-specific error; the bug report flagged the prior wording
/// (which mentioned AppleScript / Automation) as misleading.
fn open_event_store_with_access() -> Result<objc2::rc::Retained<objc2_event_kit::EKEventStore>> {
    use objc2_event_kit::{EKEntityType, EKEventStore};

    let store: objc2::rc::Retained<EKEventStore> = unsafe { EKEventStore::new() };

    // macOS 14+ requires every process to call requestFullAccessToEvents
    // before EKEventStore returns calendars from cloud sources (iCloud,
    // Google, Exchange). The TCC grant alone is not sufficient — without
    // this call, calendarsForEntityType returns only local calendars
    // (Birthdays, on-device), and eventsMatchingPredicate returns 0 events
    // for queries that should hit iCloud-hosted events. The request is
    // async (block-based completion handler); we wrap it in an mpsc
    // channel so we can wait synchronously. We're already inside a
    // tokio spawn_blocking, so the wait doesn't park an executor thread.
    match request_full_access_to_events_blocking(&store) {
        AccessRequestOutcome::Completed => {}
        AccessRequestOutcome::TimedOut => {
            return Err(anyhow::anyhow!(
                "Calendar Full Access dialog wasn't answered within 30s. \
                 Open System Settings > Privacy & Security > Calendars > \
                 MacRelay and switch to 'Full Access', then retry. (This \
                 is the EventKit access prompt, not the Automation prompt.)"
            ));
        }
    }

    let auth = unsafe { EKEventStore::authorizationStatusForEntityType(EKEntityType::Event) };
    let cals = unsafe { store.calendarsForEntityType(EKEntityType::Event) };
    tracing::debug!(
        target: "macrelay::calendar",
        auth = auth.0,
        calendars = cals.len(),
        "EventKit query starting"
    );

    // The legacy `authorizationStatusForEntityType:` API can return a
    // stale value on macOS 14+: even when TCC.db has auth_value=4
    // (FullAccess), this getter sometimes still returns 3 (the old
    // "Authorized" alias for WriteOnly) until the EventKit daemon
    // refreshes its in-process cache. We saw this empirically — TCC
    // showed 4, the System Settings UI showed Full Access, but
    // EKAuthorizationStatus stayed at 3 across multiple binary launches.
    // We only trust the unambiguous denied values (0/1/2); 3/4 we let
    // eventsMatchingPredicate answer authoritatively.
    if matches!(auth.0, 0..=2) {
        // 0=NotDetermined, 1=Restricted, 2=Denied
        return Err(anyhow::anyhow!(
            "Calendar access not authorized (EKAuthorizationStatus={}). \
             Open System Settings > Privacy & Security > Calendars and \
             enable MacRelay.",
            auth.0
        ));
    }

    Ok(store)
}

/// Synchronous EventKit query. Must run inside `spawn_blocking`.
fn search_events_eventkit_blocking(
    start_ts: i64,
    end_ts: i64,
    query_lc: Option<String>,
) -> Result<Vec<EventInfo>> {
    use objc2_foundation::{NSDate, NSDateFormatter};

    let store = open_event_store_with_access()?;

    // Build the date range. NSDate uses NSTimeInterval (seconds since 1970,
    // f64). Per objc2-foundation 0.3, `dateWithTimeIntervalSince1970` is
    // safe (no `unsafe` block needed) — pure value constructor.
    let start = NSDate::dateWithTimeIntervalSince1970(start_ts as f64);
    let end = NSDate::dateWithTimeIntervalSince1970(end_ts as f64);

    // `calendars: nil` searches every calendar the user has access to —
    // exactly what the old script did with `repeat with c in calendars`,
    // but pushed into the indexed query path.
    let predicate =
        unsafe { store.predicateForEventsWithStartDate_endDate_calendars(&start, &end, None) };
    let event_array = unsafe { store.eventsMatchingPredicate(&predicate) };

    // Date formatter for output strings. We keep the same shape as the
    // previous AppleScript output (`Monday, January 1, 2024 at 10:00:00 AM`)
    // so the EventInfo serialisation is byte-compatible with whatever
    // downstream tooling already consumes it.
    let formatter = NSDateFormatter::new();
    formatter.setDateStyle(objc2_foundation::NSDateFormatterStyle::FullStyle);
    formatter.setTimeStyle(objc2_foundation::NSDateFormatterStyle::MediumStyle);

    let mut events = Vec::with_capacity(event_array.len());
    for event in event_array.iter() {
        let title_ns = unsafe { event.title() };
        let title = title_ns.to_string();

        // Filter by query. We do this in Rust rather than building a
        // compound NSPredicate because EKEventStore's predicates are
        // *only* the date-range form documented by Apple — passing any
        // other predicate raises an exception per the framework docs.
        if let Some(q) = &query_lc
            && !title.to_lowercase().contains(q)
        {
            continue;
        }

        let start = unsafe { event.startDate() };
        let end = unsafe { event.endDate() };
        let start_str = formatter.stringFromDate(&start).to_string();
        let end_str = formatter.stringFromDate(&end).to_string();

        let location = unsafe { event.location() }
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty());

        let is_all_day = unsafe { event.isAllDay() };

        let calendar_name = unsafe { event.calendar() }
            .map(|c| unsafe { c.title() }.to_string())
            .unwrap_or_default();

        events.push(EventInfo {
            title,
            start_date: start_str,
            end_date: end_str,
            is_all_day,
            location,
            notes: None,
            calendar: calendar_name,
        });
    }

    // EventKit doesn't guarantee ordering; sort by start date for stable,
    // human-friendly output (matches user expectation of "next 7 days").
    events.sort_by(|a, b| a.start_date.cmp(&b.start_date));

    Ok(events)
}

/// Synchronous EventKit busy-interval fetch. Must run inside `spawn_blocking`.
fn fetch_busy_intervals_blocking(start_ts: i64, end_ts: i64) -> Result<Vec<(i64, i64)>> {
    use objc2_foundation::NSDate;

    let store = open_event_store_with_access()?;

    let start = NSDate::dateWithTimeIntervalSince1970(start_ts as f64);
    let end = NSDate::dateWithTimeIntervalSince1970(end_ts as f64);

    let predicate =
        unsafe { store.predicateForEventsWithStartDate_endDate_calendars(&start, &end, None) };
    let event_array = unsafe { store.eventsMatchingPredicate(&predicate) };

    let mut intervals = Vec::with_capacity(event_array.len());
    for event in event_array.iter() {
        let s = unsafe { event.startDate() };
        let e = unsafe { event.endDate() };
        // `timeIntervalSince1970` is a pure value getter and binds as
        // safe in objc2-foundation 0.3 — same reason `dateWithTimeInterval…`
        // doesn't need an `unsafe` block above.
        let s_secs = s.timeIntervalSince1970() as i64;
        let e_secs = e.timeIntervalSince1970() as i64;
        intervals.push((s_secs, e_secs));
    }

    Ok(intervals)
}

/// Create a calendar event using AppleScript.
pub async fn create_event(
    title: &str,
    _start_date: &str,
    _end_date: &str,
    is_all_day: bool,
    location: &str,
    notes: &str,
) -> Result<String> {
    let allday_str = if is_all_day { "true" } else { "false" };
    let escaped_title = escape_applescript_string(title);
    let escaped_location = escape_applescript_string(location);
    let escaped_notes = escape_applescript_string(notes);

    // For now, use descriptive date strings. Later we'll add Unix timestamp conversion.
    let script = format!(
        r#"
        tell application "Calendar"
            tell calendar 1
                set newEvent to make new event with properties {{summary:"{escaped_title}", start date:(current date), end date:((current date) + 3600), location:"{escaped_location}", description:"{escaped_notes}", allday event:{allday_str}}}
                return "Event created: {escaped_title}"
            end tell
        end tell
        "#
    );

    crate::macos::applescript::run_applescript(&script)
}

/// Outcome of `request_full_access_to_events_blocking`. Distinguishing
/// "completion handler fired" from "30s elapsed without it firing" lets
/// the caller produce a useful error message — earlier code conflated
/// these and surfaced the same generic timeout text either way.
pub(crate) enum AccessRequestOutcome {
    /// The completion block fired. Whether access was granted is
    /// reflected in `EKEventStore::authorizationStatusForEntityType`
    /// after this returns; we don't capture the bool here because
    /// macOS 14+ makes that getter unreliable anyway.
    Completed,
    /// 30 seconds elapsed and the completion block never fired. The
    /// user almost certainly has the permission dialog up and hasn't
    /// answered.
    TimedOut,
}

/// Synchronously request full Calendar access via the macOS 14+ API.
///
/// The framework method is `requestFullAccessToEventsWithCompletion:`, which
/// takes a block-based callback. We invoke it and block on an mpsc channel
/// until the completion handler fires, so the caller can treat it as a
/// normal synchronous call. This MUST be invoked from a thread that has a
/// runloop OR be wrapped in spawn_blocking (which is already the case
/// here) — completion blocks dispatch onto a private EventKit queue so
/// they fire regardless of caller runloop state.
///
/// If access is already granted, the completion fires immediately with
/// `granted: true`. If denied, it fires immediately with `granted: false`.
/// If undetermined, macOS shows the system permission dialog and the
/// completion fires after the user clicks. We give it a 30 second cap to
/// avoid hanging forever if the user ignores the dialog — caller can
/// produce a Calendar-specific error message off the `TimedOut` variant.
pub(crate) fn request_full_access_to_events_blocking(
    store: &objc2_event_kit::EKEventStore,
) -> AccessRequestOutcome {
    use block2::RcBlock;
    use objc2::runtime::Bool;
    use objc2_foundation::NSError;
    use std::sync::Mutex;
    use std::sync::mpsc;

    let (tx, rx) = mpsc::channel::<()>();
    // Wrap tx in a Mutex<Option<_>> so the block, which may be invoked
    // multiple times in theory, can take the sender exactly once.
    let tx = std::sync::Arc::new(Mutex::new(Some(tx)));
    let tx_clone = std::sync::Arc::clone(&tx);

    // Closure parameter types must match the Objective-C signature exactly:
    // BOOL (objc2::runtime::Bool, not Rust bool) and `NSError * _Nullable`
    // (raw pointer, not Option<&NSError>).
    let block = RcBlock::new(move |_granted: Bool, _err: *mut NSError| {
        if let Some(t) = tx_clone.lock().unwrap().take() {
            let _ = t.send(());
        }
    });

    // The framework's binding takes the completion handler as a raw
    // `*mut Block<...>`. RcBlock only implements Deref (not DerefMut), so
    // we cast through a const pointer. EventKit doesn't mutate the block;
    // it just retains and invokes it.
    let block_ptr = (&*block) as *const _ as *mut _;
    unsafe {
        store.requestFullAccessToEventsWithCompletion(block_ptr);
    }

    // Wait up to 30s for the callback. If access is already granted, this
    // returns essentially instantly. If a dialog is up, we wait for the user.
    match rx.recv_timeout(std::time::Duration::from_secs(30)) {
        Ok(()) => AccessRequestOutcome::Completed,
        Err(_) => AccessRequestOutcome::TimedOut,
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CalendarInfo {
    pub title: String,
    pub calendar_type: String,
    pub allows_modify: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct EventInfo {
    pub title: String,
    pub start_date: String,
    pub end_date: String,
    pub is_all_day: bool,
    pub location: Option<String>,
    pub notes: Option<String>,
    pub calendar: String,
}
