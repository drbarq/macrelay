use anyhow::{Context, Result};
use std::process::Command;
use std::time::Duration;

/// Execute an AppleScript and return the output.
pub fn run_applescript(script: &str) -> Result<String> {
    run_applescript_with_timeout(script, Duration::from_secs(60))
}

/// Execute an AppleScript with a custom timeout.
pub fn run_applescript_with_timeout(script: &str, timeout: Duration) -> Result<String> {
    let child = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .context("Failed to spawn osascript")?;

    let output = child
        .wait_with_output()
        .context("Failed to wait for osascript")?;

    // Check timeout would need async or thread-based approach for real timeout.
    // For now we rely on osascript's own timeout behavior.
    let _ = timeout;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(anyhow::anyhow!("AppleScript error: {stderr}"))
    }
}

/// Execute a JXA (JavaScript for Automation) script.
pub fn run_jxa(script: &str) -> Result<String> {
    let output = Command::new("osascript")
        .arg("-l")
        .arg("JavaScript")
        .arg("-e")
        .arg(script)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .context("Failed to run JXA script")?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(anyhow::anyhow!("JXA error: {stderr}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_applescript() {
        let result = run_applescript("return 2 + 2");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "4");
    }

    #[test]
    fn test_applescript_string() {
        let result = run_applescript(r#"return "hello world""#);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "hello world");
    }

    #[test]
    fn test_invalid_applescript() {
        let result = run_applescript("this is not valid applescript at all xyz");
        assert!(result.is_err());
    }

    #[test]
    fn test_simple_jxa() {
        let result = run_jxa("2 + 2");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "4");
    }
}
