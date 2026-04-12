use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// All toggleable services, in menu display order.
pub const SERVICES: &[ServiceDef] = &[
    ServiceDef {
        key: "calendar",
        label: "Calendar",
    },
    ServiceDef {
        key: "reminders",
        label: "Reminders",
    },
    ServiceDef {
        key: "contacts",
        label: "Contacts",
    },
    ServiceDef {
        key: "mail",
        label: "Mail",
    },
    ServiceDef {
        key: "messages",
        label: "Messages",
    },
    ServiceDef {
        key: "notes",
        label: "Notes",
    },
    ServiceDef {
        key: "stickies",
        label: "Stickies",
    },
    ServiceDef {
        key: "shortcuts",
        label: "Shortcuts",
    },
    ServiceDef {
        key: "location",
        label: "Location",
    },
    ServiceDef {
        key: "maps",
        label: "Maps",
    },
    ServiceDef {
        key: "ui",
        label: "UI Automation",
    },
    ServiceDef {
        key: "system",
        label: "System",
    },
];

pub struct ServiceDef {
    pub key: &'static str,
    pub label: &'static str,
}

/// Persisted preferences for the menu bar app.
#[derive(Debug, Serialize, Deserialize)]
pub struct MenuBarConfig {
    /// Which services are enabled (key -> enabled).
    pub services: BTreeMap<String, bool>,
}

impl Default for MenuBarConfig {
    fn default() -> Self {
        let mut services = BTreeMap::new();
        for svc in SERVICES {
            services.insert(svc.key.to_string(), true);
        }
        Self { services }
    }
}

impl MenuBarConfig {
    /// Path to the config file: ~/Library/Application Support/MacRelay/config.json
    pub fn path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        PathBuf::from(home)
            .join("Library/Application Support/MacRelay")
            .join("config.json")
    }

    /// Load from disk, or return defaults if missing/corrupt.
    pub fn load() -> Self {
        let path = Self::path();
        match std::fs::read_to_string(&path) {
            Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Persist to disk.
    pub fn save(&self) {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(&path, json);
        }
    }

    /// Is a given service enabled?
    pub fn is_enabled(&self, key: &str) -> bool {
        self.services.get(key).copied().unwrap_or(true)
    }

    /// Toggle a service and save.
    pub fn toggle(&mut self, key: &str) {
        let entry = self.services.entry(key.to_string()).or_insert(true);
        *entry = !*entry;
        self.save();
    }

    /// Get the list of enabled service keys.
    pub fn enabled_services(&self) -> Vec<&str> {
        SERVICES
            .iter()
            .filter(|s| self.is_enabled(s.key))
            .map(|s| s.key)
            .collect()
    }

    /// Are all services enabled?
    pub fn all_enabled(&self) -> bool {
        SERVICES.iter().all(|s| self.is_enabled(s.key))
    }

    /// Write the Claude Desktop config to include macrelay with the right --service args.
    pub fn write_claude_desktop_config(&self) {
        self.write_claude_config_to_path(&claude_desktop_config_path(), &macrelay_binary_path());
    }

    /// Write Claude Desktop config to a specific path. Testable without touching real config.
    pub(crate) fn write_claude_config_to_path(&self, config_path: &PathBuf, binary_path: &str) {
        // Read existing config or start fresh. If the file exists but contains
        // non-object JSON (null, array, etc.), treat it as empty.
        let mut config: serde_json::Value = std::fs::read_to_string(config_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_else(|| serde_json::json!({}));

        if !config.is_object() {
            config = serde_json::json!({});
        }

        let mcp_servers = config
            .as_object_mut()
            .expect("guaranteed object after guard")
            .entry("mcpServers")
            .or_insert_with(|| serde_json::json!({}));

        let entry = if self.all_enabled() {
            // No --service flags needed when all are enabled
            serde_json::json!({
                "command": binary_path,
            })
        } else {
            let enabled = self.enabled_services();
            if enabled.is_empty() {
                // Remove the entry entirely if nothing is enabled
                if let Some(servers) = mcp_servers.as_object_mut() {
                    servers.remove("macrelay");
                }
                write_json_config(config_path, &config);
                return;
            }
            let mut args: Vec<String> = Vec::new();
            for svc in &enabled {
                args.push("--service".to_string());
                args.push(svc.to_string());
            }
            serde_json::json!({
                "command": binary_path,
                "args": args,
            })
        };

        if let Some(servers) = mcp_servers.as_object_mut() {
            servers.insert("macrelay".to_string(), entry);
        }

        write_json_config(config_path, &config);
    }
}

fn claude_desktop_config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(home).join("Library/Application Support/Claude/claude_desktop_config.json")
}

fn macrelay_binary_path() -> String {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    format!("{home}/.local/bin/macrelay")
}

fn write_json_config(path: &PathBuf, value: &serde_json::Value) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(value) {
        let _ = std::fs::write(path, json);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    /// Helper: create a temp config path for testing write_claude_config_to_path.
    fn temp_config_path(dir: &TempDir) -> PathBuf {
        dir.path().join("claude_desktop_config.json")
    }

    // ── Pure unit tests (no I/O) ──────────────────────────────────────────

    #[test]
    fn default_config_has_all_services_enabled() {
        let config = MenuBarConfig::default();
        assert!(config.all_enabled());
        assert_eq!(config.enabled_services().len(), SERVICES.len());
    }

    #[test]
    fn toggle_flips_existing_service() {
        let mut config = MenuBarConfig::default();
        assert!(config.is_enabled("calendar"));

        // toggle() should flip true -> false (we don't call save in test since HOME is real)
        let entry = config
            .services
            .entry("calendar".to_string())
            .or_insert(true);
        *entry = !*entry;
        assert!(!config.is_enabled("calendar"));
        assert!(!config.all_enabled());

        // Flip back
        let entry = config
            .services
            .entry("calendar".to_string())
            .or_insert(true);
        *entry = !*entry;
        assert!(config.is_enabled("calendar"));
        assert!(config.all_enabled());
    }

    #[test]
    fn toggle_handles_unknown_key() {
        let mut config = MenuBarConfig::default();
        // toggle on a key not in the default map: or_insert(true) then flip -> false
        let entry = config
            .services
            .entry("nonexistent".to_string())
            .or_insert(true);
        *entry = !*entry;
        assert!(!config.is_enabled("nonexistent"));
    }

    #[test]
    fn enabled_services_filters_correctly() {
        let mut config = MenuBarConfig::default();
        config.services.insert("mail".to_string(), false);
        config.services.insert("notes".to_string(), false);
        let enabled = config.enabled_services();
        assert!(!enabled.contains(&"mail"));
        assert!(!enabled.contains(&"notes"));
        assert!(enabled.contains(&"calendar"));
    }

    #[test]
    fn services_list_matches_server() {
        let keys: Vec<&str> = SERVICES.iter().map(|s| s.key).collect();
        assert!(keys.contains(&"calendar"));
        assert!(keys.contains(&"reminders"));
        assert!(keys.contains(&"contacts"));
        assert!(keys.contains(&"mail"));
        assert!(keys.contains(&"messages"));
        assert!(keys.contains(&"notes"));
        assert!(keys.contains(&"stickies"));
        assert!(keys.contains(&"shortcuts"));
        assert!(keys.contains(&"location"));
        assert!(keys.contains(&"maps"));
        assert!(keys.contains(&"ui"));
        assert!(keys.contains(&"system"));
        assert_eq!(keys.len(), 12);
    }

    #[test]
    fn is_enabled_defaults_true_for_unknown_key() {
        let config = MenuBarConfig::default();
        assert!(config.is_enabled("some_future_service"));
    }

    #[test]
    fn serialization_round_trip() {
        let mut config = MenuBarConfig::default();
        config.services.insert("mail".to_string(), false);
        let json = serde_json::to_string(&config).unwrap();
        let loaded: MenuBarConfig = serde_json::from_str(&json).unwrap();
        assert!(!loaded.is_enabled("mail"));
        assert!(loaded.is_enabled("calendar"));
    }

    // ── Config writer tests (tempdir I/O) ─────────────────────────────────

    #[test]
    fn write_config_all_enabled_writes_command_only() {
        let dir = TempDir::new().unwrap();
        let path = temp_config_path(&dir);
        let config = MenuBarConfig::default();

        config.write_claude_config_to_path(&path, "/usr/local/bin/macrelay");

        let written: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let macrelay = &written["mcpServers"]["macrelay"];
        assert_eq!(macrelay["command"], "/usr/local/bin/macrelay");
        assert!(macrelay.get("args").is_none());
    }

    #[test]
    fn write_config_partial_enabled_writes_service_args() {
        let dir = TempDir::new().unwrap();
        let path = temp_config_path(&dir);
        let mut config = MenuBarConfig::default();
        config.services.insert("mail".to_string(), false);
        config.services.insert("notes".to_string(), false);

        config.write_claude_config_to_path(&path, "/usr/local/bin/macrelay");

        let written: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let macrelay = &written["mcpServers"]["macrelay"];
        assert_eq!(macrelay["command"], "/usr/local/bin/macrelay");
        let args: Vec<&str> = macrelay["args"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        // Should have --service flags but NOT mail or notes
        assert!(args.contains(&"--service"));
        assert!(args.contains(&"calendar"));
        assert!(!args.contains(&"mail"));
        assert!(!args.contains(&"notes"));
    }

    #[test]
    fn write_config_none_enabled_removes_macrelay_entry() {
        let dir = TempDir::new().unwrap();
        let path = temp_config_path(&dir);
        let mut config = MenuBarConfig::default();
        for svc in SERVICES {
            config.services.insert(svc.key.to_string(), false);
        }

        config.write_claude_config_to_path(&path, "/usr/local/bin/macrelay");

        let written: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert!(written["mcpServers"].get("macrelay").is_none());
    }

    #[test]
    fn write_config_preserves_other_mcp_servers() {
        let dir = TempDir::new().unwrap();
        let path = temp_config_path(&dir);

        // Write pre-existing config with another MCP server
        let existing = serde_json::json!({
            "mcpServers": {
                "other-server": {"command": "/usr/bin/other"},
            },
            "preferences": {"theme": "dark"},
        });
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(serde_json::to_string_pretty(&existing).unwrap().as_bytes())
            .unwrap();

        let config = MenuBarConfig::default();
        config.write_claude_config_to_path(&path, "/usr/local/bin/macrelay");

        let written: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        // other-server still present
        assert_eq!(
            written["mcpServers"]["other-server"]["command"],
            "/usr/bin/other"
        );
        // macrelay added
        assert_eq!(
            written["mcpServers"]["macrelay"]["command"],
            "/usr/local/bin/macrelay"
        );
        // preferences preserved
        assert_eq!(written["preferences"]["theme"], "dark");
    }

    #[test]
    fn write_config_handles_missing_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nonexistent/subdir/config.json");
        let config = MenuBarConfig::default();

        config.write_claude_config_to_path(&path, "/usr/local/bin/macrelay");

        let written: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(
            written["mcpServers"]["macrelay"]["command"],
            "/usr/local/bin/macrelay"
        );
    }

    #[test]
    fn write_config_handles_corrupt_json() {
        let dir = TempDir::new().unwrap();
        let path = temp_config_path(&dir);
        std::fs::write(&path, "not valid json {{{").unwrap();

        let config = MenuBarConfig::default();
        config.write_claude_config_to_path(&path, "/usr/local/bin/macrelay");

        let written: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(
            written["mcpServers"]["macrelay"]["command"],
            "/usr/local/bin/macrelay"
        );
    }

    #[test]
    fn write_config_handles_non_object_json() {
        let dir = TempDir::new().unwrap();
        let path = temp_config_path(&dir);
        // Valid JSON but not an object — this used to panic
        std::fs::write(&path, "null").unwrap();

        let config = MenuBarConfig::default();
        config.write_claude_config_to_path(&path, "/usr/local/bin/macrelay");

        let written: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(
            written["mcpServers"]["macrelay"]["command"],
            "/usr/local/bin/macrelay"
        );
    }
}
