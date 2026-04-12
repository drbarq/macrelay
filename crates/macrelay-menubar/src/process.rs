use std::process::Command;

/// Check if a macrelay server process is currently running (not counting ourselves).
pub fn is_macrelay_running() -> bool {
    let output = Command::new("pgrep").args(["-x", "macrelay"]).output();

    match output {
        Ok(o) => o.status.success(),
        Err(_) => false,
    }
}

/// Status text for the menu.
pub fn status_text(running: bool) -> &'static str {
    if running {
        "● Running"
    } else {
        "○ Stopped"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_text_displays_correctly() {
        assert_eq!(status_text(true), "● Running");
        assert_eq!(status_text(false), "○ Stopped");
    }
}
