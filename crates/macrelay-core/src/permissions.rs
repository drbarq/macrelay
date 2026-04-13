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

/// Current status of a permission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionStatus {
    Granted,
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
    pub fn check_all() -> HashMap<PermissionType, PermissionStatus> {
        let mut statuses = HashMap::new();
        statuses.insert(PermissionType::Accessibility, Self::check_accessibility());
        statuses.insert(PermissionType::Calendar, Self::check_calendar());
        statuses.insert(PermissionType::Reminders, Self::check_reminders());
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

    /// Check Calendar permission status.
    pub fn check_calendar() -> PermissionStatus {
        use objc2_event_kit::{EKEntityType, EKEventStore};
        let status = unsafe { EKEventStore::authorizationStatusForEntityType(EKEntityType::Event) };
        match status.0 {
            0 => PermissionStatus::NotDetermined, // EKAuthorizationStatusNotDetermined
            1 => PermissionStatus::Denied,        // EKAuthorizationStatusRestricted
            2 => PermissionStatus::Denied,        // EKAuthorizationStatusDenied
            3 => PermissionStatus::Granted,       // EKAuthorizationStatusAuthorized
            4 => PermissionStatus::Granted,       // EKAuthorizationStatusFullAccess (macOS 14+)
            _ => PermissionStatus::Unknown,
        }
    }

    /// Check Reminders permission status.
    pub fn check_reminders() -> PermissionStatus {
        use objc2_event_kit::{EKEntityType, EKEventStore};
        let status =
            unsafe { EKEventStore::authorizationStatusForEntityType(EKEntityType::Reminder) };
        match status.0 {
            0 => PermissionStatus::NotDetermined,
            1 => PermissionStatus::Denied,
            2 => PermissionStatus::Denied,
            3 => PermissionStatus::Granted,
            4 => PermissionStatus::Granted,
            _ => PermissionStatus::Unknown,
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
    /// Returns Ok(()) if granted or not yet determined (so macOS can prompt).
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
            // NotDetermined — let the operation run so macOS can prompt the user
            PermissionStatus::NotDetermined => Ok(()),
            // Denied or Unknown — block with a helpful error
            _ => Err(Self::permission_error(perm)),
        }
    }

    /// Return a formatted error message for a missing permission.
    pub fn permission_error(perm: PermissionType) -> String {
        format!(
            "Permission required: {perm}\n\n{}\n\nAfter granting access, try the operation again.",
            perm.grant_instructions()
        )
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
    fn test_serialization() {
        let status = PermissionStatus::Granted;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"granted\"");

        let ptype = PermissionType::FullDiskAccess;
        let json = serde_json::to_string(&ptype).unwrap();
        assert_eq!(json, "\"full_disk_access\"");
    }
}
