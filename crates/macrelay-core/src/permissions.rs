use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// All permission types that mac-app-oss may need.
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
                "Open System Settings > Privacy & Security > Accessibility and enable access for mac-app-oss."
            }
            Self::ScreenRecording => {
                "Open System Settings > Privacy & Security > Screen Recording and enable access for mac-app-oss."
            }
            Self::FullDiskAccess => {
                "Open System Settings > Privacy & Security > Full Disk Access and enable access for mac-app-oss."
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
        statuses.insert(PermissionType::Calendar, PermissionStatus::Unknown);
        statuses.insert(PermissionType::Reminders, PermissionStatus::Unknown);
        statuses.insert(PermissionType::Contacts, PermissionStatus::Unknown);
        statuses.insert(PermissionType::Location, PermissionStatus::Unknown);
        statuses.insert(PermissionType::FullDiskAccess, Self::check_full_disk_access());
        statuses.insert(PermissionType::ScreenRecording, PermissionStatus::Unknown);
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
        if std::fs::metadata(&test_path).is_ok() {
            PermissionStatus::Granted
        } else {
            PermissionStatus::Denied
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
        assert_eq!(PermissionType::FullDiskAccess.to_string(), "Full Disk Access");
        assert_eq!(PermissionType::ScreenRecording.to_string(), "Screen Recording");
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
            assert!(!pt.grant_instructions().is_empty(), "Instructions empty for {pt}");
        }
    }

    #[test]
    fn test_permission_error_format() {
        let msg = PermissionManager::permission_error(PermissionType::Calendar);
        assert!(msg.contains("Permission required: Calendar"));
        assert!(msg.contains("System Settings"));
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
