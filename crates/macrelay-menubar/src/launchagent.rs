use std::path::PathBuf;

const LABEL: &str = "com.macrelay.menubar";

fn plist_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(home)
        .join("Library/LaunchAgents")
        .join(format!("{LABEL}.plist"))
}

fn menubar_binary_path() -> String {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    format!("{home}/.local/bin/macrelay-menubar")
}

/// Generate the plist XML content for the LaunchAgent.
fn generate_plist(binary: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{LABEL}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{binary}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <false/>
</dict>
</plist>
"#
    )
}

/// Check if the LaunchAgent is currently installed.
pub fn is_launch_at_login_enabled() -> bool {
    plist_path().exists()
}

/// Install the LaunchAgent plist so the menu bar app starts at login.
pub fn enable_launch_at_login() {
    let path = plist_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let plist = generate_plist(&menubar_binary_path());
    let _ = std::fs::write(&path, plist);
}

/// Remove the LaunchAgent plist.
pub fn disable_launch_at_login() {
    let path = plist_path();
    let _ = std::fs::remove_file(&path);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plist_path_is_in_launch_agents() {
        let path = plist_path();
        assert!(
            path.to_string_lossy()
                .contains("Library/LaunchAgents/com.macrelay.menubar.plist")
        );
    }

    #[test]
    fn binary_path_is_local_bin() {
        let path = menubar_binary_path();
        assert!(path.ends_with("/.local/bin/macrelay-menubar"));
    }

    #[test]
    fn plist_content_contains_required_keys() {
        let plist = generate_plist("/usr/local/bin/macrelay-menubar");

        assert!(plist.contains("<key>Label</key>"));
        assert!(plist.contains(&format!("<string>{LABEL}</string>")));
        assert!(plist.contains("<key>ProgramArguments</key>"));
        assert!(plist.contains("<string>/usr/local/bin/macrelay-menubar</string>"));
        assert!(plist.contains("<key>RunAtLoad</key>"));
        assert!(plist.contains("<true/>"));
        assert!(plist.contains("<key>KeepAlive</key>"));
        assert!(plist.contains("<false/>"));
    }

    #[test]
    fn plist_is_valid_xml_structure() {
        let plist = generate_plist("/usr/local/bin/macrelay-menubar");
        assert!(plist.starts_with("<?xml version=\"1.0\""));
        assert!(plist.contains("<!DOCTYPE plist"));
        assert!(plist.contains("<plist version=\"1.0\">"));
        assert!(plist.contains("</plist>"));
    }

    #[test]
    fn enable_disable_round_trip() {
        let dir = tempfile::TempDir::new().unwrap();
        let test_path = dir.path().join("test.plist");

        // Simulate enable by writing plist to a test path
        let plist = generate_plist("/usr/local/bin/macrelay-menubar");
        std::fs::write(&test_path, &plist).unwrap();
        assert!(test_path.exists());

        // Simulate disable by removing
        std::fs::remove_file(&test_path).unwrap();
        assert!(!test_path.exists());
    }
}
