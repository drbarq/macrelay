use anyhow::{Context, Result};
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

/// Default timeout for AppleScript execution (30 seconds).
/// This is long enough for legitimate operations (large mailbox scans,
/// bulk reminder queries) but short enough to catch hung permission dialogs
/// before the MCP client's own timeout (typically 4 minutes) fires.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Extended timeout for operations known to be slow (e.g. Shortcuts execution,
/// large data queries across many accounts).
pub const EXTENDED_TIMEOUT: Duration = Duration::from_secs(60);

/// A trait for executing AppleScript and JXA, allowing for mocks in unit tests.
pub trait ScriptRunner: Send + Sync {
    fn run_applescript(&self, script: &str) -> Result<String>;
    fn run_applescript_with_timeout(&self, script: &str, timeout: Duration) -> Result<String>;
    fn run_jxa(&self, script: &str) -> Result<String>;
}

/// The real, production implementation that uses `osascript`.
pub struct OsascriptRunner;

impl ScriptRunner for OsascriptRunner {
    fn run_applescript(&self, script: &str) -> Result<String> {
        run_applescript_impl(script, DEFAULT_TIMEOUT)
    }

    fn run_applescript_with_timeout(&self, script: &str, timeout: Duration) -> Result<String> {
        run_applescript_impl(script, timeout)
    }

    fn run_jxa(&self, script: &str) -> Result<String> {
        run_jxa_impl(script)
    }
}

tokio::task_local! {
    /// A task-local override for the ScriptRunner.
    /// This allows async tests to inject a mock runner without affecting other concurrent tests.
    pub static MOCK_RUNNER: Arc<dyn ScriptRunner>;
}

/// Helper to get the current runner.
fn current_runner() -> Arc<dyn ScriptRunner> {
    MOCK_RUNNER
        .try_with(|r| r.clone())
        .unwrap_or_else(|_| Arc::new(OsascriptRunner))
}

/// Execute an AppleScript and return the output.
pub fn run_applescript(script: &str) -> Result<String> {
    current_runner().run_applescript(script)
}

/// Execute an AppleScript with a custom timeout.
pub fn run_applescript_with_timeout(script: &str, timeout: Duration) -> Result<String> {
    current_runner().run_applescript_with_timeout(script, timeout)
}

/// Execute a JXA (JavaScript for Automation) script.
pub fn run_jxa(script: &str) -> Result<String> {
    current_runner().run_jxa(script)
}

// -----------------------------------------------------------------------------
// Internal implementations
// -----------------------------------------------------------------------------

fn run_applescript_impl(script: &str, timeout: Duration) -> Result<String> {
    let mut child = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .context("Failed to spawn osascript")?;

    let start = std::time::Instant::now();
    let poll_interval = Duration::from_millis(100);

    // Poll the child process with a real, enforced timeout.
    // Without this, osascript can hang indefinitely when macOS shows a
    // permission dialog (e.g. "MacRelay would like to access your Reminders")
    // and the user hasn't responded yet.
    loop {
        match child.try_wait() {
            Ok(Some(_status)) => {
                // Process exited — collect output normally.
                break;
            }
            Ok(None) => {
                if start.elapsed() > timeout {
                    // Kill the hung process and return a helpful error.
                    let _ = child.kill();
                    let _ = child.wait(); // Reap the zombie
                    return Err(anyhow::anyhow!(
                        "AppleScript timed out after {}s. \
                        This usually means macOS is showing a permission dialog \
                        that hasn't been answered. Check for a system dialog asking \
                        to grant access, then try again.",
                        timeout.as_secs()
                    ));
                }
                std::thread::sleep(poll_interval);
            }
            Err(e) => {
                return Err(anyhow::Error::from(e).context("Failed to poll osascript process"));
            }
        }
    }

    let output = child
        .wait_with_output()
        .context("Failed to read osascript output")?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(intercept_applescript_error(&stderr))
    }
}

fn intercept_applescript_error(stderr: &str) -> anyhow::Error {
    use crate::permissions::{PermissionManager, PermissionType};

    // -1743: Not authorized to send Apple events
    if stderr.contains("-1743") {
        return anyhow::anyhow!(PermissionManager::permission_error(
            PermissionType::Accessibility
        ));
    }

    // -25211: osascript is not allowed assistive access
    if stderr.contains("-25211") {
        return anyhow::anyhow!(PermissionManager::permission_error(
            PermissionType::Accessibility
        ));
    }

    // 1002: osascript is not allowed to send keystrokes
    if stderr.contains("1002") {
        return anyhow::anyhow!(PermissionManager::permission_error(
            PermissionType::Accessibility
        ));
    }

    // -600: Application isn't running
    if stderr.contains("-600") {
        // Extract the app name from the error message if possible
        // Typical format: "Calendar got an error: Application isn't running. (-600)"
        let app_name = stderr
            .split(" got an error")
            .next()
            .and_then(|s| s.rsplit("error: ").next())
            .unwrap_or("The target application");
        return anyhow::anyhow!("{app_name} is not running. Please open the app and try again.");
    }

    anyhow::anyhow!("AppleScript error: {stderr}")
}

fn run_jxa_impl(script: &str) -> Result<String> {
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
        Err(intercept_applescript_error(&stderr))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore]
    fn test_simple_applescript() {
        let result = run_applescript("return 2 + 2");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "4");
    }

    #[test]
    #[ignore]
    fn test_applescript_string() {
        let result = run_applescript(r#"return "hello world""#);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "hello world");
    }

    #[test]
    #[ignore]
    fn test_invalid_applescript() {
        let result = run_applescript("this is not valid applescript at all xyz");
        assert!(result.is_err());
    }

    #[test]
    #[ignore]
    fn test_simple_jxa() {
        let result = run_jxa("2 + 2");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "4");
    }

    #[tokio::test]
    async fn test_mock_runner() {
        struct TestMock;
        impl ScriptRunner for TestMock {
            fn run_applescript(&self, _script: &str) -> Result<String> {
                Ok("mocked".to_string())
            }
            fn run_applescript_with_timeout(
                &self,
                _script: &str,
                _timeout: Duration,
            ) -> Result<String> {
                Ok("mocked".to_string())
            }
            fn run_jxa(&self, _script: &str) -> Result<String> {
                Ok("mocked".to_string())
            }
        }

        let mock = Arc::new(TestMock);

        let result = MOCK_RUNNER
            .scope(mock, async { run_applescript("return 2 + 2") })
            .await;

        assert_eq!(result.unwrap(), "mocked");
    }

    #[test]
    fn test_intercept_error_1743_returns_accessibility() {
        let err = intercept_applescript_error("execution error: -1743");
        assert!(
            err.to_string()
                .contains("Permission required: Accessibility")
        );
    }

    #[test]
    fn test_intercept_error_25211_returns_accessibility() {
        let err = intercept_applescript_error("execution error: -25211");
        assert!(
            err.to_string()
                .contains("Permission required: Accessibility")
        );
    }

    #[test]
    fn test_intercept_error_1002_returns_accessibility() {
        let err = intercept_applescript_error(
            "Error: osascript is not allowed to send keystrokes. (1002)",
        );
        assert!(
            err.to_string()
                .contains("Permission required: Accessibility")
        );
    }

    #[test]
    fn test_intercept_unknown_error_returns_raw() {
        let err = intercept_applescript_error("some random error -999");
        assert!(
            err.to_string()
                .contains("AppleScript error: some random error")
        );
    }

    #[test]
    fn test_intercept_error_600_app_not_running() {
        let err = intercept_applescript_error(
            "execution error: Contacts got an error: Application isn't running. (-600)",
        );
        let msg = err.to_string();
        assert!(
            msg.contains("not running"),
            "Expected 'not running' in error, got: {msg}"
        );
    }

    #[test]
    fn test_default_timeout_is_30s() {
        assert_eq!(DEFAULT_TIMEOUT, Duration::from_secs(30));
    }

    #[test]
    fn test_extended_timeout_is_60s() {
        assert_eq!(EXTENDED_TIMEOUT, Duration::from_secs(60));
    }

    #[test]
    #[ignore] // Requires real osascript — run locally only
    fn test_timeout_kills_hung_script() {
        // This script sleeps for 10 seconds, but we give it only 1 second.
        // The timeout should kill it and return an error.
        let result = run_applescript_impl("delay 10", Duration::from_secs(1));
        assert!(result.is_err(), "Expected timeout error");
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("timed out"),
            "Expected 'timed out' in error, got: {err}"
        );
    }

    #[test]
    #[ignore] // Requires real osascript — run locally only
    fn test_timeout_does_not_fire_for_fast_scripts() {
        // A fast script should complete well within the timeout.
        let result = run_applescript_impl("return 2 + 2", Duration::from_secs(5));
        assert!(result.is_ok(), "Expected success, got: {:?}", result);
        assert_eq!(result.unwrap(), "4");
    }
}
