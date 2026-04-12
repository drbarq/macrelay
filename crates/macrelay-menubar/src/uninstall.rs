use std::path::PathBuf;
use std::process::Command;

/// Remove all MacRelay files, configs, and entries. Returns a summary of what was removed.
pub fn uninstall() -> Vec<String> {
    let mut actions = Vec::new();
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());

    // ── Standalone binaries ──────────────────────────────────────────
    for bin in &["macrelay", "macrelay-menubar"] {
        let path = format!("{home}/.local/bin/{bin}");
        if std::fs::remove_file(&path).is_ok() {
            actions.push(format!("Removed {path}"));
        }
    }

    // ── LaunchAgent ──────────────────────────────────────────────────
    let plist = format!("{home}/Library/LaunchAgents/com.macrelay.menubar.plist");
    if std::path::Path::new(&plist).exists() {
        let uid = unsafe { libc::getuid() };
        let _ = Command::new("launchctl")
            .args(["bootout", &format!("gui/{uid}"), &plist])
            .output();
        if std::fs::remove_file(&plist).is_ok() {
            actions.push("Removed LaunchAgent".to_string());
        }
    }

    // ── Preferences ──────────────────────────────────────────────────
    let prefs_dir = format!("{home}/Library/Application Support/MacRelay");
    if std::fs::remove_dir_all(&prefs_dir).is_ok() {
        actions.push("Removed preferences".to_string());
    }

    // ── Claude Desktop config entry ──────────────────────────────────
    remove_from_client_config(
        &PathBuf::from(format!(
            "{home}/Library/Application Support/Claude/claude_desktop_config.json"
        )),
        &mut actions,
        "Claude Desktop",
    );

    // ── Claude Desktop extension ───────────────────────────────────────
    let ext_dir =
        format!("{home}/Library/Application Support/Claude/Claude Extensions/com.macrelay.app");
    if std::fs::remove_dir_all(&ext_dir).is_ok() {
        actions.push("Removed Claude Desktop extension".to_string());
    }

    // ── Claude Code config entry ─────────────────────────────────────
    remove_from_client_config(
        &PathBuf::from(format!("{home}/.claude/mcp.json")),
        &mut actions,
        "Claude Code",
    );

    // ── Homebrew cask ─────────────────────────────────────────────────
    // GUI apps don't have /opt/homebrew/bin in PATH, so try both locations
    let brew_bin = if std::path::Path::new("/opt/homebrew/bin/brew").exists() {
        "/opt/homebrew/bin/brew"
    } else if std::path::Path::new("/usr/local/bin/brew").exists() {
        "/usr/local/bin/brew"
    } else {
        "brew" // fallback to PATH lookup
    };
    let brew_check = Command::new(brew_bin)
        .args(["list", "--cask", "macrelay"])
        .output();
    if let Ok(output) = brew_check
        && output.status.success()
    {
        let _ = Command::new(brew_bin)
            .args(["uninstall", "--cask", "macrelay"])
            .output();
        actions.push("Uninstalled Homebrew cask".to_string());
    }

    // ── App bundle (delete self — safe on macOS while running) ────────
    if std::fs::remove_dir_all("/Applications/MacRelay.app").is_ok() {
        actions.push("Removed /Applications/MacRelay.app".to_string());
    }

    if actions.is_empty() {
        actions.push("Nothing to remove — already clean".to_string());
    }

    actions
}

fn remove_from_client_config(path: &PathBuf, actions: &mut Vec<String>, label: &str) {
    let contents = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => return,
    };
    let mut config: serde_json::Value = match serde_json::from_str(&contents) {
        Ok(v) => v,
        Err(_) => return,
    };

    let mut removed = false;
    if let Some(obj) = config.as_object_mut()
        && let Some(servers) = obj.get_mut("mcpServers").and_then(|s| s.as_object_mut())
    {
        // Remove both old lowercase and new capitalized keys
        removed |= servers.remove("MacRelay").is_some();
        removed |= servers.remove("macrelay").is_some();
    }

    if removed {
        if let Ok(json) = serde_json::to_string_pretty(&config) {
            let _ = std::fs::write(path, json);
        }
        actions.push(format!("Removed from {label} config"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn remove_from_client_config_removes_both_key_variants() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.json");

        // Old lowercase key
        let config = serde_json::json!({
            "mcpServers": {
                "macrelay": {"command": "/usr/local/bin/macrelay"},
                "other": {"command": "/usr/bin/other"},
            },
        });
        std::fs::write(&path, serde_json::to_string_pretty(&config).unwrap()).unwrap();

        let mut actions = Vec::new();
        remove_from_client_config(&path, &mut actions, "Test");

        assert_eq!(actions.len(), 1);
        let written: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert!(written["mcpServers"].get("macrelay").is_none());
        assert!(written["mcpServers"].get("MacRelay").is_none());
        assert_eq!(written["mcpServers"]["other"]["command"], "/usr/bin/other");
    }

    #[test]
    fn remove_from_client_config_noop_if_missing() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nonexistent.json");
        let mut actions = Vec::new();
        remove_from_client_config(&path, &mut actions, "Test");
        assert!(actions.is_empty());
    }

    #[test]
    fn remove_from_client_config_handles_new_key() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.json");

        let config = serde_json::json!({
            "mcpServers": {
                "MacRelay": {"command": "/Applications/MacRelay.app/Contents/MacOS/macrelay"},
            },
        });
        std::fs::write(&path, serde_json::to_string_pretty(&config).unwrap()).unwrap();

        let mut actions = Vec::new();
        remove_from_client_config(&path, &mut actions, "Test");

        assert_eq!(actions.len(), 1);
        let written: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert!(written["mcpServers"].get("MacRelay").is_none());
    }
}
