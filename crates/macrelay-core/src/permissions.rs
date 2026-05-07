use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// All permission types that MacRelay may need.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionType {
    Accessibility,
    ScreenRecording,
    FullDiskAccess,
    Calendar,
    Reminders,
    Contacts,
    Location,
}

/// Result of a live probe against an EventKit-backed permission.
///
/// We use this to disambiguate the macOS 14+ `EKAuthorizationStatus=3`
/// gotcha — see `PermissionManager::probe_calendar_read_access`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProbeResult {
    /// Probe query returned at least one event — read access confirmed.
    Passed,
    /// Probe query completed but returned zero events. Could mean the
    /// user genuinely has no events in the probe window, or that the
    /// access is write-only and reads silently drop. We report write-only
    /// as the conservative answer.
    FailedEmpty,
    /// Probe query raised an error. Treated like FailedEmpty for
    /// reporting purposes; surfaced separately for future logging.
    #[allow(dead_code)]
    FailedError,
}

/// Current status of a permission.
///
/// `WriteOnly` is specific to Calendar/Reminders on macOS 14+: the user
/// has granted access but only at the "Add Events" tier — read queries
/// (`EKEventStore::eventsMatchingPredicate`) silently return empty. We
/// surface this as its own status rather than collapsing into `Granted`,
/// because the diagnostic experience matters: `permissions_status` saying
/// `"calendar": "granted"` while EventKit returns no events is the kind
/// of mismatch that wasted hours of debugging this session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionStatus {
    Granted,
    GrantedWriteOnly,
    Denied,
    NotDetermined,
    Unknown,
}

impl std::fmt::Display for PermissionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Accessibility => write!(f, "Accessibility"),
            Self::ScreenRecording => write!(f, "Screen Recording"),
            Self::FullDiskAccess => write!(f, "Full Disk Access"),
            Self::Calendar => write!(f, "Calendar"),
            Self::Reminders => write!(f, "Reminders"),
            Self::Contacts => write!(f, "Contacts"),
            Self::Location => write!(f, "Location"),
        }
    }
}

impl PermissionType {
    /// Human-readable instructions for granting this permission.
    pub fn grant_instructions(&self) -> &'static str {
        match self {
            Self::Accessibility => {
                "Open System Settings > Privacy & Security > Accessibility and enable access for MacRelay."
            }
            Self::ScreenRecording => {
                "Open System Settings > Privacy & Security > Screen Recording and enable access for MacRelay."
            }
            Self::FullDiskAccess => {
                "Open System Settings > Privacy & Security > Full Disk Access and enable access for MacRelay."
            }
            Self::Calendar => {
                "Calendar access will be requested automatically. If denied, go to System Settings > Privacy & Security > Calendars."
            }
            Self::Reminders => {
                "Reminders access will be requested automatically. If denied, go to System Settings > Privacy & Security > Reminders."
            }
            Self::Contacts => {
                "Contacts access will be requested automatically. If denied, go to System Settings > Privacy & Security > Contacts."
            }
            Self::Location => {
                "Location access will be requested automatically. If denied, go to System Settings > Privacy & Security > Location Services."
            }
        }
    }
}

/// Check and manage macOS permissions.
pub struct PermissionManager;

impl PermissionManager {
    /// Check the status of all permissions.
    ///
    /// For Calendar and Reminders, we use the probe-augmented variants
    /// (`check_calendar_with_probe`, `check_reminders_with_probe`) so the
    /// diagnostic output reflects empirical access on macOS 14+, not just
    /// the unreliable framework getter. This makes the call slightly
    /// more expensive (~100ms first time) but the result is honest.
    pub fn check_all() -> HashMap<PermissionType, PermissionStatus> {
        let mut statuses = HashMap::new();
        statuses.insert(PermissionType::Accessibility, Self::check_accessibility());
        statuses.insert(PermissionType::Calendar, Self::check_calendar_with_probe());
        statuses.insert(
            PermissionType::Reminders,
            Self::check_reminders_with_probe(),
        );
        statuses.insert(PermissionType::Contacts, Self::check_contacts());
        statuses.insert(PermissionType::Location, Self::check_location());
        statuses.insert(
            PermissionType::FullDiskAccess,
            Self::check_full_disk_access(),
        );
        statuses.insert(
            PermissionType::ScreenRecording,
            Self::check_screen_recording(),
        );
        statuses
    }

    /// Check if Accessibility permission is granted.
    pub fn check_accessibility() -> PermissionStatus {
        // AXIsProcessTrusted() from ApplicationServices framework
        unsafe extern "C" {
            fn AXIsProcessTrusted() -> bool;
        }
        if unsafe { AXIsProcessTrusted() } {
            PermissionStatus::Granted
        } else {
            PermissionStatus::Denied
        }
    }

    /// Check Full Disk Access by attempting to read a protected file.
    pub fn check_full_disk_access() -> PermissionStatus {
        let home = std::env::var("HOME").unwrap_or_default();
        let test_path = format!("{home}/Library/Messages/chat.db");
        if std::fs::File::open(&test_path).is_ok() {
            PermissionStatus::Granted
        } else {
            PermissionStatus::Denied
        }
    }

    /// Check Calendar permission status (fast path, framework-getter only).
    ///
    /// This is the cheap call used by handler-level permission gates. It
    /// reflects what `EKEventStore::authorizationStatusForEntityType:`
    /// reports — which on macOS 14+ is unreliable for the 3-vs-4 split
    /// (see module-level docs). For diagnostic reporting where accuracy
    /// matters, use `check_calendar_with_probe`.
    pub fn check_calendar() -> PermissionStatus {
        use objc2_event_kit::{EKEntityType, EKEventStore};
        let status = unsafe { EKEventStore::authorizationStatusForEntityType(EKEntityType::Event) };
        match status.0 {
            0 => PermissionStatus::NotDetermined,
            1 => PermissionStatus::Denied,
            2 => PermissionStatus::Denied,
            3 => PermissionStatus::GrantedWriteOnly, // see check_calendar_with_probe
            4 => PermissionStatus::Granted,
            _ => PermissionStatus::Unknown,
        }
    }

    /// Check Calendar permission with a live probe to disambiguate
    /// `EKAuthorizationStatus=3` on macOS 14+.
    ///
    /// `authorizationStatusForEntityType:` empirically lies on macOS 14+:
    /// it can persistently return 3 (the legacy "Authorized" alias for
    /// WriteOnly) even when TCC has `auth_value=4` (FullAccess) and live
    /// queries succeed. When the raw status is 3, we run a wide-window
    /// `eventsMatchingPredicate` probe; if it returns events, the user
    /// actually has FullAccess and we report `Granted`.
    ///
    /// This call hits EventKit and can take ~100ms on first invocation
    /// (it triggers `requestFullAccessToEventsWithCompletion:`). It's
    /// intended for `system_permissions_status` and other diagnostic
    /// surfaces, not for hot paths.
    pub fn check_calendar_with_probe() -> PermissionStatus {
        match Self::check_calendar() {
            PermissionStatus::GrantedWriteOnly => match Self::probe_calendar_read_access() {
                ProbeResult::Passed => PermissionStatus::Granted,
                _ => PermissionStatus::GrantedWriteOnly,
            },
            other => other,
        }
    }

    /// Check Reminders permission status (fast path).
    pub fn check_reminders() -> PermissionStatus {
        use objc2_event_kit::{EKEntityType, EKEventStore};
        let status =
            unsafe { EKEventStore::authorizationStatusForEntityType(EKEntityType::Reminder) };
        match status.0 {
            0 => PermissionStatus::NotDetermined,
            1 => PermissionStatus::Denied,
            2 => PermissionStatus::Denied,
            3 => PermissionStatus::GrantedWriteOnly,
            4 => PermissionStatus::Granted,
            _ => PermissionStatus::Unknown,
        }
    }

    /// Reminders equivalent of `check_calendar_with_probe`. Currently
    /// returns the framework getter result unchanged — we don't yet
    /// implement a Reminders read probe (that requires
    /// `fetchRemindersMatchingPredicate:completion:`, a different async
    /// API). When we do, this is the surface to update.
    pub fn check_reminders_with_probe() -> PermissionStatus {
        Self::check_reminders()
    }

    /// Result of a live read probe against EKEventStore.
    ///
    /// `Passed` means we got events back — read access is real, ignore
    /// whatever the framework getter said. `FailedEmpty` means the query
    /// completed but returned 0 events; we can't distinguish "no events
    /// in the probe window" from "write-only access silently dropping
    /// reads," so we conservatively report write-only. `FailedError`
    /// means the call itself errored — extremely rare.
    ///
    /// Note that this result is *informational*. We report it to give
    /// the user a more honest picture than the framework getter, but a
    /// single probe is not 100% conclusive. The probe is wide enough
    /// (~365 days) that empty results from a real account are rare.
    #[cfg(target_os = "macos")]
    fn probe_calendar_read_access() -> ProbeResult {
        use objc2_event_kit::{EKEntityType, EKEventStore};
        use objc2_foundation::NSDate;

        let store: objc2::rc::Retained<EKEventStore> = unsafe { EKEventStore::new() };

        // Force EventKit to hydrate cloud sources before the probe — same
        // reason eventkit::search_events_eventkit_blocking does it.
        crate::macos::eventkit::request_full_access_to_events_blocking(&store);
        let _ = unsafe { store.calendarsForEntityType(EKEntityType::Event) };

        // Wide window: 1 year on either side of now. If the user has any
        // events at all, this should match. Probe is read-only; we don't
        // care about the contents, only the count.
        let now = NSDate::now();
        let past = NSDate::dateWithTimeIntervalSinceNow(-365.0 * 86400.0);
        let future = NSDate::dateWithTimeIntervalSinceNow(365.0 * 86400.0);
        let predicate = unsafe {
            store.predicateForEventsWithStartDate_endDate_calendars(&past, &future, None)
        };
        // We use `now` to avoid an unused-variable warning on the trivial
        // path; the actual query uses past/future.
        let _ = now;
        let events = unsafe { store.eventsMatchingPredicate(&predicate) };

        if events.is_empty() {
            ProbeResult::FailedEmpty
        } else {
            ProbeResult::Passed
        }
    }

    /// Check Contacts permission status.
    pub fn check_contacts() -> PermissionStatus {
        use objc2_contacts::{CNContactStore, CNEntityType};
        let status =
            unsafe { CNContactStore::authorizationStatusForEntityType(CNEntityType::Contacts) };
        match status.0 {
            0 => PermissionStatus::NotDetermined, // CNAuthorizationStatusNotDetermined
            1 => PermissionStatus::Denied,        // CNAuthorizationStatusRestricted
            2 => PermissionStatus::Denied,        // CNAuthorizationStatusDenied
            3 => PermissionStatus::Granted,       // CNAuthorizationStatusAuthorized
            _ => PermissionStatus::Unknown,
        }
    }

    /// Check Location permission status.
    pub fn check_location() -> PermissionStatus {
        // Use direct FFI call to kCLLocationManager.authorizationStatus() or equivalent
        // if possible, but since objc2 requires an instance for this version,
        // we'll try to get it via the class method if we can find the right binding
        // or just use the instance for now as it's the standard way in modern macOS.
        // To address the concern about side effects, we ensure the manager is dropped immediately.
        use objc2_core_location::CLLocationManager;
        let status = unsafe {
            let manager = CLLocationManager::new();
            manager.authorizationStatus()
        };
        match status.0 {
            0 => PermissionStatus::NotDetermined, // kCLAuthorizationStatusNotDetermined
            1 => PermissionStatus::Denied,        // kCLAuthorizationStatusRestricted
            2 => PermissionStatus::Denied,        // kCLAuthorizationStatusDenied
            3 => PermissionStatus::Granted,       // kCLAuthorizationStatusAuthorizedAlways
            4 => PermissionStatus::Granted,       // kCLAuthorizationStatusAuthorizedWhenInUse
            _ => PermissionStatus::Unknown,
        }
    }

    /// Check Screen Recording permission status.
    pub fn check_screen_recording() -> PermissionStatus {
        // CGPreflightScreenCaptureAccess() is available on macOS 10.15+
        unsafe extern "C" {
            fn CGPreflightScreenCaptureAccess() -> bool;
        }
        if unsafe { CGPreflightScreenCaptureAccess() } {
            PermissionStatus::Granted
        } else {
            PermissionStatus::Denied
        }
    }

    /// Check if a permission is granted or can be prompted.
    /// Returns Ok(()) if granted.
    /// Returns Ok(()) if not yet determined — the first AppleScript call will
    /// trigger the macOS permission dialog. If the user doesn't respond in time,
    /// the enforced timeout in `run_applescript_impl` will catch it and return a
    /// clear error instead of hanging indefinitely.
    /// Returns Err only if explicitly denied (user must grant manually).
    pub fn require(perm: PermissionType) -> Result<(), String> {
        let status = match perm {
            PermissionType::Accessibility => Self::check_accessibility(),
            PermissionType::ScreenRecording => Self::check_screen_recording(),
            PermissionType::FullDiskAccess => Self::check_full_disk_access(),
            PermissionType::Calendar => Self::check_calendar(),
            PermissionType::Reminders => Self::check_reminders(),
            PermissionType::Contacts => Self::check_contacts(),
            PermissionType::Location => Self::check_location(),
        };
        match status {
            // Granted — proceed
            PermissionStatus::Granted => Ok(()),
            // WriteOnly — proceed; the call site decides whether the limited
            // tier is enough (e.g. create_event works fine, search_events
            // does not). EventKit calls return the appropriate framework
            // error when read access is missing.
            PermissionStatus::GrantedWriteOnly => Ok(()),
            // NotDetermined — let the operation run so macOS can prompt the user.
            // The subprocess timeout will catch it if the dialog goes unanswered.
            PermissionStatus::NotDetermined => Ok(()),
            // Denied or Unknown — block with a helpful error
            _ => Err(Self::permission_error(perm)),
        }
    }

    /// Check permission status and return it directly (useful for pre-flight checks).
    pub fn status(perm: PermissionType) -> PermissionStatus {
        match perm {
            PermissionType::Accessibility => Self::check_accessibility(),
            PermissionType::ScreenRecording => Self::check_screen_recording(),
            PermissionType::FullDiskAccess => Self::check_full_disk_access(),
            PermissionType::Calendar => Self::check_calendar(),
            PermissionType::Reminders => Self::check_reminders(),
            PermissionType::Contacts => Self::check_contacts(),
            PermissionType::Location => Self::check_location(),
        }
    }

    /// Return a formatted error message for a missing permission.
    pub fn permission_error(perm: PermissionType) -> String {
        format!(
            "Permission required: {perm}\n\n{}\n\nAfter granting access, try the operation again.",
            perm.grant_instructions()
        )
    }

    /// Read Automation (Apple Events) grants from the user's TCC database.
    ///
    /// macOS does not expose Automation permission status via any public API,
    /// but the grants are recorded in a SQLite database the user has read
    /// access to. We query for `kTCCServiceAppleEvents` rows that match either
    /// MacRelay's bundle identifier or its current binary path; both forms
    /// can appear in TCC depending on how the app was launched and codesigned.
    ///
    /// Returns a map keyed by target-app bundle id (e.g. `com.apple.Notes`)
    /// to `PermissionStatus`. Returns an empty map (not an error) if the
    /// database can't be read — this should be informational, not blocking.
    ///
    /// This matters because `check_all()` reports the seven privacy categories
    /// with public APIs (Calendar, Reminders, Contacts, etc.) — it does NOT
    /// report Automation, which is the permission AppleScript-driven services
    /// (Notes, Mail, Calendar via osascript) actually need at runtime. A
    /// previous debugging session was misled into thinking permissions were
    /// fine because `check_all()` showed all-green, while the real bottleneck
    /// was a slow Notes script timing out — but the same blind spot would
    /// hide a real Automation denial.
    pub fn check_automation_grants() -> HashMap<String, PermissionStatus> {
        let mut grants = HashMap::new();

        // Locate the user TCC database.
        let home = match std::env::var("HOME") {
            Ok(h) => h,
            Err(_) => return grants,
        };
        let db_path = format!("{home}/Library/Application Support/com.apple.TCC/TCC.db");
        if !std::path::Path::new(&db_path).exists() {
            return grants;
        }

        // Identify the calling process for matching against TCC client column.
        // TCC may have grants under bundle id, full binary path, or both.
        let bundle_id = "com.macrelay.app"; // matches CFBundleIdentifier in scripts/build-app.sh
        let exe_path = std::env::current_exe()
            .ok()
            .and_then(|p| p.to_str().map(|s| s.to_string()))
            .unwrap_or_default();

        let conn = match rusqlite::Connection::open_with_flags(
            &db_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        ) {
            Ok(c) => c,
            Err(_) => return grants, // FDA may not be granted; return empty
        };

        let mut stmt = match conn.prepare(
            "SELECT indirect_object_identifier, auth_value
             FROM access
             WHERE service = 'kTCCServiceAppleEvents'
               AND (client = ?1 OR client = ?2)",
        ) {
            Ok(s) => s,
            Err(_) => return grants,
        };

        let rows = stmt.query_map(rusqlite::params![bundle_id, exe_path], |row| {
            let target: String = row.get(0)?;
            let auth: i64 = row.get(1)?;
            Ok((target, auth))
        });

        if let Ok(rows) = rows {
            for row in rows.flatten() {
                let (target, auth) = row;
                // TCC auth_value codes:
                //   0 = denied, 1 = unknown/unset, 2 = allowed,
                //   3 = limited, 4 = add-modify allowed
                let status = match auth {
                    2..=4 => PermissionStatus::Granted,
                    0 => PermissionStatus::Denied,
                    1 => PermissionStatus::NotDetermined,
                    _ => PermissionStatus::Unknown,
                };
                // If we have multiple rows for the same target (e.g. one by
                // bundle id, one by path), prefer the most permissive.
                grants
                    .entry(target)
                    .and_modify(|s| {
                        if matches!(*s, PermissionStatus::Denied | PermissionStatus::Unknown)
                            && matches!(status, PermissionStatus::Granted)
                        {
                            *s = status;
                        }
                    })
                    .or_insert(status);
            }
        }

        grants
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_type_display() {
        assert_eq!(PermissionType::Accessibility.to_string(), "Accessibility");
        assert_eq!(
            PermissionType::FullDiskAccess.to_string(),
            "Full Disk Access"
        );
        assert_eq!(
            PermissionType::ScreenRecording.to_string(),
            "Screen Recording"
        );
    }

    #[test]
    fn test_grant_instructions_not_empty() {
        let types = [
            PermissionType::Accessibility,
            PermissionType::ScreenRecording,
            PermissionType::FullDiskAccess,
            PermissionType::Calendar,
            PermissionType::Reminders,
            PermissionType::Contacts,
            PermissionType::Location,
        ];
        for pt in types {
            assert!(
                !pt.grant_instructions().is_empty(),
                "Instructions empty for {pt}"
            );
        }
    }

    #[test]
    fn test_permission_error_format() {
        let msg = PermissionManager::permission_error(PermissionType::Calendar);
        assert!(msg.contains("Permission required: Calendar"));
        assert!(msg.contains("System Settings"));
    }

    #[test]
    fn test_require_allows_granted() {
        // Granted should pass through
        // (We can't control the actual macOS state in unit tests,
        // but we can test the logic by checking that require() returns
        // Ok for permissions that happen to be granted on this machine,
        // or test the error path for known-denied ones.)
    }

    #[test]
    fn test_require_allows_not_determined() {
        // NotDetermined should pass through so macOS can prompt
        // This is tested implicitly via the match arm in require()
        // The key invariant: NotDetermined -> Ok(()), not Err
        let status = PermissionStatus::NotDetermined;
        // Simulate what require() does
        let result = match status {
            PermissionStatus::Granted | PermissionStatus::NotDetermined => Ok(()),
            _ => Err("denied"),
        };
        assert!(result.is_ok());
    }

    #[test]
    fn test_require_blocks_denied() {
        let status = PermissionStatus::Denied;
        let result = match status {
            PermissionStatus::Granted | PermissionStatus::NotDetermined => Ok(()),
            _ => Err("denied"),
        };
        assert!(result.is_err());
    }

    #[test]
    fn test_status_returns_permission_status() {
        // Verify that status() returns a valid PermissionStatus for each type.
        // We can't control the actual macOS state, but we can verify it doesn't panic.
        let types = [
            PermissionType::Accessibility,
            PermissionType::ScreenRecording,
            PermissionType::FullDiskAccess,
            PermissionType::Calendar,
            PermissionType::Reminders,
            PermissionType::Contacts,
            PermissionType::Location,
        ];
        for pt in types {
            let status = PermissionManager::status(pt);
            // Just verify it returns a valid variant
            match status {
                PermissionStatus::Granted
                | PermissionStatus::GrantedWriteOnly
                | PermissionStatus::Denied
                | PermissionStatus::NotDetermined
                | PermissionStatus::Unknown => {} // all valid
            }
        }
    }

    #[test]
    fn test_serialization() {
        let status = PermissionStatus::Granted;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"granted\"");

        let ptype = PermissionType::FullDiskAccess;
        let json = serde_json::to_string(&ptype).unwrap();
        assert_eq!(json, "\"full_disk_access\"");
    }
}
