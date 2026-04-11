use anyhow::{Context, Result};
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

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
        run_applescript_impl(script, Duration::from_secs(60))
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
}
