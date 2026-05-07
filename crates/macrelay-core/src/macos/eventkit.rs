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

/// Search calendar events using EventKit (the proper macOS API).
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
pub async fn search_events_applescript(
    days_ahead: u32,
    query: Option<&str>,
) -> Result<Vec<EventInfo>> {
    // The function name is unchanged so the call site in calendar/mod.rs
    // doesn't need to change. Despite the name, no AppleScript runs here
    // anymore. (Renaming the function would be a wider diff and break the
    // callable surface — kept stable on purpose.)
    let query_owned = query.map(|s| s.to_lowercase());
    tokio::task::spawn_blocking(move || search_events_eventkit_blocking(days_ahead, query_owned))
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking join failed: {e}"))?
}

/// Synchronous EventKit query. Must run inside `spawn_blocking`.
fn search_events_eventkit_blocking(
    days_ahead: u32,
    query_lc: Option<String>,
) -> Result<Vec<EventInfo>> {
    use objc2_event_kit::{EKEntityType, EKEventStore};
    use objc2_foundation::{NSDate, NSDateFormatter};

    // EventKit auth is checked at the handler level via PermissionManager;
    // by the time we get here, the store should be usable. If it isn't,
    // eventsMatchingPredicate will return an empty array and we'll just
    // return zero events — same behaviour as the old AppleScript path
    // wrapping `try` blocks around inaccessible calendars.
    let store: objc2::rc::Retained<EKEventStore> = unsafe { EKEventStore::new() };

    // macOS 14+ requires every process to call requestFullAccessToEvents
    // before EKEventStore returns calendars from cloud sources (iCloud,
    // Google, Exchange). The TCC grant alone is not sufficient — without
    // this call, calendarsForEntityType returns only local calendars
    // (Birthdays, on-device), and eventsMatchingPredicate returns 0 events
    // for queries that should hit iCloud-hosted events. The request is
    // async (block-based completion handler); we wrap it in a dispatch
    // semaphore so we can wait synchronously. We're already inside a
    // tokio spawn_blocking, so the wait doesn't park an executor thread.
    request_full_access_to_events_blocking(&store);

    // Hydrate the calendar list (after the access request, this includes
    // cloud sources). We don't use this list directly — eventsMatchingPredicate
    // with `calendars: nil` searches everything — but reading it warms up
    // the store and is also useful for the diagnostic eprintln below.
    let cals = unsafe { store.calendarsForEntityType(EKEntityType::Event) };

    let auth = unsafe { EKEventStore::authorizationStatusForEntityType(EKEntityType::Event) };
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
    // We don't gate on the return value any more for that reason; we
    // attempt the query and let `eventsMatchingPredicate` answer
    // authoritatively. If access is genuinely missing, it returns an
    // empty array and we surface a useful empty-result message; if
    // access is present, we get events.
    if matches!(auth.0, 0..=2) {
        // 0=NotDetermined, 1=Restricted, 2=Denied — these we trust
        // immediately. The ambiguous-on-mac14+ values (3, 4) we let
        // eventsMatchingPredicate decide.
        return Err(anyhow::anyhow!(
            "Calendar access not authorized (EKAuthorizationStatus={}). \
             Open System Settings > Privacy & Security > Calendars and \
             enable MacRelay.",
            auth.0
        ));
    }

    // Build the date range. NSDate uses NSTimeInterval (seconds since 1970).
    // Per objc2-foundation 0.3, `NSDate::now` and `dateWithTimeIntervalSinceNow`
    // are safe (no `unsafe` block needed) — they're side-effect free factory
    // methods. The objc2 binding marks methods `unsafe` only when they can
    // violate Rust safety invariants; pure value constructors do not.
    let now = NSDate::now();
    let end = NSDate::dateWithTimeIntervalSinceNow(days_ahead as f64 * 86400.0);

    // `calendars: nil` searches every calendar the user has access to —
    // exactly what the old script did with `repeat with c in calendars`,
    // but pushed into the indexed query path.
    let predicate =
        unsafe { store.predicateForEventsWithStartDate_endDate_calendars(&now, &end, None) };
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

/// Synchronously request full Calendar access via the macOS 14+ API.
///
/// The framework method is `requestFullAccessToEventsWithCompletion:`, which
/// takes a block-based callback. We invoke it and block on a dispatch
/// semaphore until the completion handler fires, so the caller can treat
/// it as a normal synchronous call. This MUST be invoked from a thread
/// that has a runloop OR be wrapped in spawn_blocking (which is already
/// the case here) — completion blocks dispatch onto a private EventKit
/// queue so they will fire regardless of caller runloop state.
///
/// If access is already granted, the completion fires immediately with
/// `granted: true`. If denied, it fires immediately with `granted: false`.
/// If undetermined, macOS shows the system permission dialog and the
/// completion fires after the user clicks. We give it a 30 second cap to
/// avoid hanging forever if the user ignores the dialog — that matches
/// the existing AppleScript subprocess timeout philosophy and produces
/// the same kind of recoverable error.
pub(crate) fn request_full_access_to_events_blocking(store: &objc2_event_kit::EKEventStore) {
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
    let _ = rx.recv_timeout(std::time::Duration::from_secs(30));
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
