use std::sync::Arc;

use rmcp::model::Tool;
use serde_json::json;

use crate::macos::escape::{escape_applescript_string, escape_jxa_string};
use crate::registry::{ServiceRegistry, ToolHandler, error_result, schema_from_json, text_result};

/// Register all UI viewer tools with the service registry.
pub fn register(registry: &mut ServiceRegistry) {
    registry.register(
        "ui_ui_viewer_list_apps",
        Tool::new(
            "ui_ui_viewer_list_apps",
            "[SYSTEM] List all running (foreground) applications on this Mac, returning name, PID, and bundle identifier for each.",
            schema_from_json(json!({
                "type": "object",
                "properties": {},
            })),
        ),
        handler_list_apps(),
    );

    registry.register(
        "ui_ui_viewer_get_frontmost",
        Tool::new(
            "ui_ui_viewer_get_frontmost",
            "[SYSTEM] Get the frontmost (active) application and its current window title.",
            schema_from_json(json!({
                "type": "object",
                "properties": {},
            })),
        ),
        handler_get_frontmost(),
    );

    registry.register(
        "ui_ui_viewer_get_ui_tree",
        Tool::new(
            "ui_ui_viewer_get_ui_tree",
            "[READ] Get the accessibility UI element tree of a running application. Returns an indented hierarchy showing role, title, and value of each UI element.",
            schema_from_json(json!({
                "type": "object",
                "properties": {
                    "app_name": {
                        "type": "string",
                        "description": "Name of the application process (e.g. \"Safari\", \"Finder\")."
                    },
                    "max_depth": {
                        "type": "integer",
                        "description": "Maximum depth to traverse the UI tree. Default 3."
                    }
                },
                "required": ["app_name"]
            })),
        ),
        handler_get_ui_tree(),
    );

    registry.register(
        "ui_ui_viewer_get_visible_text",
        Tool::new(
            "ui_ui_viewer_get_visible_text",
            "[READ] Extract all visible text (static text elements) from the windows of a running application.",
            schema_from_json(json!({
                "type": "object",
                "properties": {
                    "app_name": {
                        "type": "string",
                        "description": "Name of the application process (e.g. \"Safari\", \"Finder\")."
                    }
                },
                "required": ["app_name"]
            })),
        ),
        handler_get_visible_text(),
    );

    registry.register(
        "ui_ui_viewer_find_elements",
        Tool::new(
            "ui_ui_viewer_find_elements",
            "[READ] Find UI elements in an application's frontmost window matching an optional role and/or title filter.",
            schema_from_json(json!({
                "type": "object",
                "properties": {
                    "app_name": {
                        "type": "string",
                        "description": "Name of the application process (e.g. \"Safari\", \"Finder\")."
                    },
                    "role": {
                        "type": "string",
                        "description": "Accessibility role to filter by (e.g. \"AXButton\", \"AXTextField\", \"AXStaticText\")."
                    },
                    "title": {
                        "type": "string",
                        "description": "Title or name substring to filter by."
                    }
                },
                "required": ["app_name"]
            })),
        ),
        handler_find_elements(),
    );

    registry.register(
        "ui_ui_viewer_capture_snapshot",
        Tool::new(
            "ui_ui_viewer_capture_snapshot",
            "[SYSTEM] Take a screenshot of a specific application window or the full screen. Returns the file path of the saved PNG image.",
            schema_from_json(json!({
                "type": "object",
                "properties": {
                    "app_name": {
                        "type": "string",
                        "description": "Name of the application to capture. If omitted, captures the full screen."
                    }
                }
            })),
        ),
        handler_capture_snapshot(),
    );
}

fn handler_list_apps() -> ToolHandler {
    Arc::new(|_args| {
        Box::pin(async move {
            let script = r#"
tell application "System Events"
    set appList to ""
    repeat with p in (every process whose background only is false)
        set appList to appList & name of p & "||" & (unix id of p) & "||" & (bundle identifier of p) & linefeed
    end repeat
    return appList
end tell
"#;
            match crate::macos::applescript::run_applescript(script) {
                Ok(output) => {
                    let trimmed = output.trim();
                    if trimmed.is_empty() {
                        return Ok(text_result("No running foreground applications found."));
                    }
                    let mut lines: Vec<serde_json::Value> = Vec::new();
                    for line in trimmed.lines() {
                        let parts: Vec<&str> = line.split("||").collect();
                        if parts.len() >= 3 {
                            lines.push(json!({
                                "name": parts[0].trim(),
                                "pid": parts[1].trim(),
                                "bundle_id": parts[2].trim(),
                            }));
                        }
                    }
                    let json = serde_json::to_string_pretty(&lines)?;
                    Ok(text_result(format!(
                        "Found {} running application(s):\n\n{json}",
                        lines.len()
                    )))
                }
                Err(e) => Ok(error_result(format!("Failed to list applications: {e}"))),
            }
        })
    })
}

fn handler_get_frontmost() -> ToolHandler {
    Arc::new(|_args| {
        Box::pin(async move {
            let script = r#"
tell application "System Events"
    set frontApp to first process whose frontmost is true
    set appName to name of frontApp
    set appBundle to bundle identifier of frontApp
    set winTitle to ""
    try
        set winTitle to name of front window of frontApp
    end try
    return appName & "||" & appBundle & "||" & winTitle
end tell
"#;
            match crate::macos::applescript::run_applescript(script) {
                Ok(output) => {
                    let parts: Vec<&str> = output.trim().split("||").collect();
                    if parts.len() >= 3 {
                        let result = json!({
                            "app_name": parts[0].trim(),
                            "bundle_id": parts[1].trim(),
                            "window_title": parts[2].trim(),
                        });
                        let json = serde_json::to_string_pretty(&result)?;
                        Ok(text_result(json))
                    } else {
                        Ok(text_result(output.trim()))
                    }
                }
                Err(e) => Ok(error_result(format!("Failed to get frontmost app: {e}"))),
            }
        })
    })
}

fn handler_get_ui_tree() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            let app_name = match args.get("app_name").and_then(|v| v.as_str()) {
                Some(name) => name,
                None => return Ok(error_result("app_name is required")),
            };

            let max_depth = args.get("max_depth").and_then(|v| v.as_u64()).unwrap_or(3);

            let escaped_app = escape_jxa_string(app_name);

            // Use JXA to walk the accessibility UI tree recursively.
            let script = format!(
                r#"
var app = Application.currentApplication();
app.includeStandardAdditions = true;
var se = Application('System Events');
var proc = se.processes.byName('{escaped_app}');

function walkElement(el, depth, maxDepth) {{
    if (depth > maxDepth) return '';
    var indent = '';
    for (var i = 0; i < depth; i++) indent += '  ';

    var role = '';
    try {{ role = el.role(); }} catch(e) {{}}
    var title = '';
    try {{ title = el.title(); }} catch(e) {{}}
    if (title === null) title = '';
    var value = '';
    try {{ value = el.value(); }} catch(e) {{}}
    if (value === null) value = '';
    var desc = '';
    try {{ desc = el.description(); }} catch(e) {{}}
    if (desc === null) desc = '';

    var line = indent + '[' + role + ']';
    if (title) line += ' title="' + title + '"';
    if (value) line += ' value="' + String(value) + '"';
    if (desc) line += ' desc="' + desc + '"';
    line += '\n';

    try {{
        var children = el.uiElements();
        for (var i = 0; i < children.length; i++) {{
            line += walkElement(children[i], depth + 1, maxDepth);
        }}
    }} catch(e) {{}}

    return line;
}}

var output = '';
try {{
    var windows = proc.windows();
    for (var w = 0; w < windows.length; w++) {{
        output += walkElement(windows[w], 0, {max_depth});
    }}
}} catch(e) {{
    output = 'Error reading UI tree: ' + e.message;
}}

output;
"#
            );

            match crate::macos::applescript::run_jxa(&script) {
                Ok(output) => {
                    let trimmed = output.trim();
                    if trimmed.is_empty() {
                        Ok(text_result(format!(
                            "No UI elements found for application \"{}\".",
                            app_name
                        )))
                    } else {
                        Ok(text_result(format!(
                            "UI tree for \"{}\" (max depth {}):\n\n{}",
                            app_name, max_depth, trimmed
                        )))
                    }
                }
                Err(e) => Ok(error_result(format!(
                    "Failed to get UI tree for \"{}\": {}",
                    app_name, e
                ))),
            }
        })
    })
}

fn handler_get_visible_text() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            let app_name = match args.get("app_name").and_then(|v| v.as_str()) {
                Some(name) => name,
                None => return Ok(error_result("app_name is required")),
            };

            let escaped_app = escape_applescript_string(app_name);

            let script = format!(
                r#"
tell application "System Events"
    tell process "{escaped_app}"
        set allText to ""
        repeat with w in windows
            try
                set textValues to value of every static text of w
                repeat with tv in textValues
                    if tv is not missing value then
                        set allText to allText & (tv as text) & linefeed
                    end if
                end repeat
            end try
        end repeat
        return allText
    end tell
end tell
"#
            );

            match crate::macos::applescript::run_applescript(&script) {
                Ok(output) => {
                    let trimmed = output.trim();
                    if trimmed.is_empty() {
                        Ok(text_result(format!(
                            "No visible text found in application \"{}\".",
                            app_name
                        )))
                    } else {
                        Ok(text_result(format!(
                            "Visible text from \"{}\":\n\n{}",
                            app_name, trimmed
                        )))
                    }
                }
                Err(e) => Ok(error_result(format!(
                    "Failed to get visible text from \"{}\": {}",
                    app_name, e
                ))),
            }
        })
    })
}

fn handler_find_elements() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            let app_name = match args.get("app_name").and_then(|v| v.as_str()) {
                Some(name) => name,
                None => return Ok(error_result("app_name is required")),
            };

            let role = args.get("role").and_then(|v| v.as_str());
            let title = args.get("title").and_then(|v| v.as_str());

            let escaped_app = escape_applescript_string(app_name);

            // Build the whose clause dynamically based on provided filters.
            let mut conditions = Vec::new();
            if let Some(r) = role {
                let escaped_role = escape_applescript_string(r);
                conditions.push(format!(r#"role is "{escaped_role}""#));
            }
            if let Some(t) = title {
                let escaped_title = escape_applescript_string(t);
                conditions.push(format!(r#"name contains "{escaped_title}""#));
            }

            let filter_clause = if conditions.is_empty() {
                String::new()
            } else {
                format!(" whose {}", conditions.join(" and "))
            };

            let script = format!(
                r#"
tell application "System Events"
    tell process "{escaped_app}"
        set results to ""
        try
            set matches to every UI element of window 1{filter_clause}
            repeat with e in matches
                set eName to ""
                try
                    set eName to name of e
                end try
                if eName is missing value then set eName to ""
                set eDesc to ""
                try
                    set eDesc to description of e
                end try
                if eDesc is missing value then set eDesc to ""
                set eRole to ""
                try
                    set eRole to role of e
                end try
                set results to results & eRole & "||" & eName & "||" & eDesc & linefeed
            end repeat
        end try
        return results
    end tell
end tell
"#
            );

            match crate::macos::applescript::run_applescript(&script) {
                Ok(output) => {
                    let trimmed = output.trim();
                    if trimmed.is_empty() {
                        Ok(text_result(format!(
                            "No matching UI elements found in \"{}\".",
                            app_name
                        )))
                    } else {
                        let mut elements: Vec<serde_json::Value> = Vec::new();
                        for line in trimmed.lines() {
                            let parts: Vec<&str> = line.split("||").collect();
                            if parts.len() >= 3 {
                                elements.push(json!({
                                    "role": parts[0].trim(),
                                    "name": parts[1].trim(),
                                    "description": parts[2].trim(),
                                }));
                            }
                        }
                        let json = serde_json::to_string_pretty(&elements)?;
                        Ok(text_result(format!(
                            "Found {} element(s) in \"{}\":\n\n{json}",
                            elements.len(),
                            app_name
                        )))
                    }
                }
                Err(e) => Ok(error_result(format!(
                    "Failed to find elements in \"{}\": {}",
                    app_name, e
                ))),
            }
        })
    })
}

fn handler_capture_snapshot() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            let app_name = args.get("app_name").and_then(|v| v.as_str());

            let output_path = "/tmp/macrelay_snapshot.png";

            match app_name {
                Some(name) => {
                    let escaped_name = escape_applescript_string(name);

                    // First, get the window ID via AppleScript, then use screencapture.
                    // Wrap in try/on error so apps with no visible window get a
                    // graceful message instead of a raw -1719 "Invalid index".
                    let id_script = format!(
                        r#"
tell application "System Events"
    try
        set wid to id of first window of process "{escaped_name}"
        return wid as text
    on error
        return "ERROR:No visible window for application \"{escaped_name}\"."
    end try
end tell
"#
                    );

                    match crate::macos::applescript::run_applescript(&id_script) {
                        Ok(window_id) => {
                            let wid = window_id.trim();
                            if let Some(err) = wid.strip_prefix("ERROR:") {
                                return Ok(error_result(err.to_string()));
                            }
                            let capture_script = format!(
                                r#"do shell script "screencapture -l {} -o -x {}""#,
                                wid, output_path
                            );
                            match crate::macos::applescript::run_applescript(&capture_script) {
                                Ok(_) => Ok(text_result(format!(
                                    "Screenshot of \"{}\" saved to {}",
                                    name, output_path
                                ))),
                                Err(e) => Ok(error_result(format!(
                                    "Failed to capture screenshot of \"{}\": {}",
                                    name, e
                                ))),
                            }
                        }
                        Err(e) => Ok(error_result(format!(
                            "Failed to get window ID for \"{}\": {}",
                            name, e
                        ))),
                    }
                }
                None => {
                    // Full screen capture
                    let capture_script =
                        format!(r#"do shell script "screencapture -o -x {}""#, output_path);
                    match crate::macos::applescript::run_applescript(&capture_script) {
                        Ok(_) => Ok(text_result(format!(
                            "Full screen screenshot saved to {}",
                            output_path
                        ))),
                        Err(e) => Ok(error_result(format!(
                            "Failed to capture full screen screenshot: {}",
                            e
                        ))),
                    }
                }
            }
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::macos::applescript::{MOCK_RUNNER, ScriptRunner};
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::time::Duration;

    #[test]
    fn test_tool_schemas_valid() {
        let mut registry = ServiceRegistry::new();
        register(&mut registry);
        let tools = registry.list_tools();
        assert_eq!(tools.len(), 6, "Expected exactly 6 ui_viewer tools");

        let names: Vec<_> = tools.iter().map(|t| t.name.as_ref()).collect();
        assert!(names.contains(&"ui_ui_viewer_list_apps"));
        assert!(names.contains(&"ui_ui_viewer_get_frontmost"));
        assert!(names.contains(&"ui_ui_viewer_get_ui_tree"));
        assert!(names.contains(&"ui_ui_viewer_get_visible_text"));
        assert!(names.contains(&"ui_ui_viewer_find_elements"));
        assert!(names.contains(&"ui_ui_viewer_capture_snapshot"));
    }

    struct AssertingMock {
        applescript_expectations: Mutex<Vec<(String, String)>>,
        jxa_expectations: Mutex<Vec<(String, String)>>,
    }

    impl AssertingMock {
        fn new() -> Self {
            Self {
                applescript_expectations: Mutex::new(Vec::new()),
                jxa_expectations: Mutex::new(Vec::new()),
            }
        }

        fn expect_applescript(self, fragment: &str, response: &str) -> Self {
            self.applescript_expectations
                .lock()
                .unwrap()
                .push((fragment.to_string(), response.to_string()));
            self
        }

        fn expect_jxa(self, fragment: &str, response: &str) -> Self {
            self.jxa_expectations
                .lock()
                .unwrap()
                .push((fragment.to_string(), response.to_string()));
            self
        }
    }

    impl ScriptRunner for AssertingMock {
        fn run_applescript(&self, script: &str) -> anyhow::Result<String> {
            let mut expectations = self.applescript_expectations.lock().unwrap();
            if expectations.is_empty() {
                panic!("Unexpected applescript call: {}", script);
            }
            let (expected_fragment, response) = expectations.remove(0);
            assert!(
                script.contains(&expected_fragment),
                "script missing fragment {:?}:\n{}",
                expected_fragment,
                script
            );
            Ok(response)
        }

        fn run_applescript_with_timeout(
            &self,
            script: &str,
            _timeout: Duration,
        ) -> anyhow::Result<String> {
            self.run_applescript(script)
        }

        fn run_jxa(&self, script: &str) -> anyhow::Result<String> {
            let mut expectations = self.jxa_expectations.lock().unwrap();
            if expectations.is_empty() {
                panic!("Unexpected JXA call: {}", script);
            }
            let (expected_fragment, response) = expectations.remove(0);
            assert!(
                script.contains(&expected_fragment),
                "JXA script missing fragment {:?}:\n{}",
                expected_fragment,
                script
            );
            Ok(response)
        }
    }

    #[tokio::test]
    async fn test_ui_viewer_list_apps() {
        let mock = Arc::new(AssertingMock::new().expect_applescript(
            "every process whose background only is false",
            "Finder||123||com.apple.finder\nSafari||456||com.apple.Safari",
        ));

        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_list_apps();
                let args = HashMap::new();
                let result = handler(args).await.unwrap();

                assert_eq!(result.is_error, Some(false));
                let content = result.content[0].as_text().unwrap().text.as_str();
                assert!(content.contains("Found 2 running application(s)"));
                assert!(content.contains("\"name\": \"Finder\""));
                assert!(content.contains("\"pid\": \"123\""));
                assert!(content.contains("\"bundle_id\": \"com.apple.finder\""));
            })
            .await;
    }

    #[tokio::test]
    async fn test_ui_viewer_get_frontmost() {
        let mock = Arc::new(AssertingMock::new().expect_applescript(
            "first process whose frontmost is true",
            "Safari||com.apple.Safari||Google Search",
        ));

        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_get_frontmost();
                let args = HashMap::new();
                let result = handler(args).await.unwrap();

                assert_eq!(result.is_error, Some(false));
                let content = result.content[0].as_text().unwrap().text.as_str();
                assert!(content.contains("\"app_name\": \"Safari\""));
                assert!(content.contains("\"bundle_id\": \"com.apple.Safari\""));
                assert!(content.contains("\"window_title\": \"Google Search\""));
            })
            .await;
    }

    #[tokio::test]
    async fn test_ui_viewer_get_ui_tree() {
        let mock = Arc::new(AssertingMock::new().expect_jxa(
            "walkElement",
            "[AXWindow] title=\"Main Window\"\n  [AXButton] title=\"Submit\"",
        ));

        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_get_ui_tree();
                let mut args = HashMap::new();
                args.insert("app_name".to_string(), json!("Safari"));

                let result = handler(args).await.unwrap();

                assert_eq!(result.is_error, Some(false));
                let content = result.content[0].as_text().unwrap().text.as_str();
                assert!(content.contains("UI tree for \"Safari\""));
                assert!(content.contains("[AXWindow] title=\"Main Window\""));
                assert!(content.contains("[AXButton] title=\"Submit\""));
            })
            .await;
    }

    #[tokio::test]
    async fn test_ui_viewer_get_visible_text() {
        let mock = Arc::new(
            AssertingMock::new()
                .expect_applescript("tell process \"Safari\"", "Hello World\nSearch Results"),
        );

        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_get_visible_text();
                let mut args = HashMap::new();
                args.insert("app_name".to_string(), json!("Safari"));

                let result = handler(args).await.unwrap();

                assert_eq!(result.is_error, Some(false));
                let content = result.content[0].as_text().unwrap().text.as_str();
                assert!(content.contains("Visible text from \"Safari\""));
                assert!(content.contains("Hello World"));
                assert!(content.contains("Search Results"));
            })
            .await;
    }

    #[tokio::test]
    async fn test_ui_viewer_find_elements() {
        let mock = Arc::new(
            AssertingMock::new()
                .expect_applescript("role is \"AXButton\"", "AXButton||Submit||Submit button"),
        );

        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_find_elements();
                let mut args = HashMap::new();
                args.insert("app_name".to_string(), json!("Safari"));
                args.insert("role".to_string(), json!("AXButton"));

                let result = handler(args).await.unwrap();

                assert_eq!(result.is_error, Some(false));
                let content = result.content[0].as_text().unwrap().text.as_str();
                assert!(content.contains("Found 1 element(s) in \"Safari\""));
                assert!(content.contains("\"role\": \"AXButton\""));
                assert!(content.contains("\"name\": \"Submit\""));
            })
            .await;
    }

    #[tokio::test]
    async fn test_ui_viewer_capture_snapshot() {
        let mock = Arc::new(
            AssertingMock::new()
                .expect_applescript("id of first window of process \"Safari\"", "123")
                .expect_applescript("screencapture -l 123", ""),
        );

        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_capture_snapshot();

                // Test app-specific capture
                let mut args = HashMap::new();
                args.insert("app_name".to_string(), json!("Safari"));
                let result = handler(args).await.unwrap();
                assert_eq!(result.is_error, Some(false));
                assert!(
                    result.content[0]
                        .as_text()
                        .unwrap()
                        .text
                        .contains("Screenshot of \"Safari\" saved to")
                );
            })
            .await;

        // Test full screen capture
        let mock_full =
            Arc::new(AssertingMock::new().expect_applescript("screencapture -o -x", ""));
        MOCK_RUNNER
            .scope(mock_full, async {
                let handler = handler_capture_snapshot();
                let args = HashMap::new();
                let result = handler(args).await.unwrap();
                assert_eq!(result.is_error, Some(false));
                assert!(
                    result.content[0]
                        .as_text()
                        .unwrap()
                        .text
                        .contains("Full screen screenshot saved to")
                );
            })
            .await;
    }

    #[tokio::test]
    async fn test_ui_viewer_list_apps_error() {
        use crate::macos::applescript::{MOCK_RUNNER, ScriptRunner};
        use std::sync::Arc;
        use std::time::Duration;

        struct ErrorMock;
        impl ScriptRunner for ErrorMock {
            fn run_applescript(&self, _script: &str) -> anyhow::Result<String> {
                Err(anyhow::anyhow!(
                    "osascript error: System Events got an error: Access is not allowed"
                ))
            }
            fn run_applescript_with_timeout(
                &self,
                _script: &str,
                _timeout: Duration,
            ) -> anyhow::Result<String> {
                unimplemented!()
            }
            fn run_jxa(&self, _script: &str) -> anyhow::Result<String> {
                unimplemented!()
            }
        }

        let mock = Arc::new(ErrorMock);
        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_list_apps();
                let args = HashMap::new();

                let result = handler(args).await.unwrap();
                assert_eq!(result.is_error, Some(true));
                let content = result.content[0].as_text().unwrap().text.as_str();
                assert!(content.contains("Access is not allowed"));
            })
            .await;
    }
}
