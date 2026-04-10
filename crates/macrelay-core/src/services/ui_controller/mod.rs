use std::sync::Arc;

use rmcp::model::Tool;
use serde_json::json;

use crate::registry::{error_result, schema_from_json, text_result, ServiceRegistry, ToolHandler};

/// Map common key names to macOS virtual key codes for use with `key code`.
fn key_name_to_code(name: &str) -> Option<u8> {
    match name.to_lowercase().as_str() {
        "return" | "enter" => Some(36),
        "tab" => Some(48),
        "space" => Some(49),
        "delete" | "backspace" => Some(51),
        "escape" | "esc" => Some(53),
        "forward_delete" => Some(117),
        "up" => Some(126),
        "down" => Some(125),
        "left" => Some(123),
        "right" => Some(124),
        "home" => Some(115),
        "end" => Some(119),
        "page_up" | "pageup" => Some(116),
        "page_down" | "pagedown" => Some(121),
        "f1" => Some(122),
        "f2" => Some(120),
        "f3" => Some(99),
        "f4" => Some(118),
        "f5" => Some(96),
        "f6" => Some(97),
        "f7" => Some(98),
        "f8" => Some(100),
        "f9" => Some(101),
        "f10" => Some(109),
        "f11" => Some(103),
        "f12" => Some(111),
        _ => None,
    }
}

/// Register all UI controller tools with the service registry.
pub fn register(registry: &mut ServiceRegistry) {
    // 1. Click at coordinates or on an element
    registry.register(
        "ui_controller_click",
        Tool::new(
            "ui_controller_click",
            "Click at screen coordinates or on a named UI element within an application. Provide either x/y coordinates, or app_name + element_name to click a specific button/element.",
            schema_from_json(json!({
                "type": "object",
                "properties": {
                    "x": {
                        "type": "integer",
                        "description": "X screen coordinate to click at."
                    },
                    "y": {
                        "type": "integer",
                        "description": "Y screen coordinate to click at."
                    },
                    "app_name": {
                        "type": "string",
                        "description": "Name of the application process containing the element to click."
                    },
                    "element_name": {
                        "type": "string",
                        "description": "Name of the UI element (button, checkbox, etc.) to click within the app."
                    },
                    "button": {
                        "type": "string",
                        "description": "Mouse button to use: 'left' (default) or 'right'.",
                        "enum": ["left", "right"]
                    },
                    "click_count": {
                        "type": "integer",
                        "description": "Number of clicks. Default 1. Use 2 for double-click."
                    }
                }
            })),
        ),
        handler_click(),
    );

    // 2. Type text into the focused field
    registry.register(
        "ui_controller_type_text",
        Tool::new(
            "ui_controller_type_text",
            "Type text into the currently focused input field. Optionally activate an application first.",
            schema_from_json(json!({
                "type": "object",
                "properties": {
                    "text": {
                        "type": "string",
                        "description": "The text to type."
                    },
                    "app_name": {
                        "type": "string",
                        "description": "Optional application to activate before typing."
                    }
                },
                "required": ["text"]
            })),
        ),
        handler_type_text(),
    );

    // 3. Press key combination
    registry.register(
        "ui_controller_press_key",
        Tool::new(
            "ui_controller_press_key",
            "Press a key or key combination. Supports special keys (return, tab, escape, arrow keys, function keys) and modifier combinations (command, shift, option, control).",
            schema_from_json(json!({
                "type": "object",
                "properties": {
                    "key": {
                        "type": "string",
                        "description": "Key to press: 'return', 'tab', 'escape', 'space', 'delete', 'up', 'down', 'left', 'right', 'f1'-'f12', or a single letter/number."
                    },
                    "modifiers": {
                        "type": "array",
                        "items": {
                            "type": "string",
                            "enum": ["command", "shift", "option", "control"]
                        },
                        "description": "Modifier keys to hold while pressing the key."
                    },
                    "app_name": {
                        "type": "string",
                        "description": "Optional application to activate before pressing the key."
                    }
                },
                "required": ["key"]
            })),
        ),
        handler_press_key(),
    );

    // 4. Scroll in an app
    registry.register(
        "ui_controller_scroll",
        Tool::new(
            "ui_controller_scroll",
            "Scroll within the frontmost application or at specific coordinates.",
            schema_from_json(json!({
                "type": "object",
                "properties": {
                    "direction": {
                        "type": "string",
                        "description": "Scroll direction.",
                        "enum": ["up", "down", "left", "right"]
                    },
                    "amount": {
                        "type": "integer",
                        "description": "Number of scroll steps. Default 3."
                    },
                    "app_name": {
                        "type": "string",
                        "description": "Optional application to activate before scrolling."
                    }
                },
                "required": ["direction"]
            })),
        ),
        handler_scroll(),
    );

    // 5. Drag from one point to another
    registry.register(
        "ui_controller_drag",
        Tool::new(
            "ui_controller_drag",
            "Drag from one screen coordinate to another. Useful for moving items, resizing, or selecting regions.",
            schema_from_json(json!({
                "type": "object",
                "properties": {
                    "from_x": {
                        "type": "integer",
                        "description": "Starting X coordinate."
                    },
                    "from_y": {
                        "type": "integer",
                        "description": "Starting Y coordinate."
                    },
                    "to_x": {
                        "type": "integer",
                        "description": "Ending X coordinate."
                    },
                    "to_y": {
                        "type": "integer",
                        "description": "Ending Y coordinate."
                    }
                },
                "required": ["from_x", "from_y", "to_x", "to_y"]
            })),
        ),
        handler_drag(),
    );

    // 6. Select a menu item
    registry.register(
        "ui_controller_select_menu",
        Tool::new(
            "ui_controller_select_menu",
            "Select a menu item from an application's menu bar. Navigate nested menus by providing the full menu path.",
            schema_from_json(json!({
                "type": "object",
                "properties": {
                    "app_name": {
                        "type": "string",
                        "description": "Name of the application whose menu to access."
                    },
                    "menu_path": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Ordered path of menu items, e.g. [\"File\", \"Save As...\"] or [\"Edit\", \"Find\", \"Find...\"]."
                    }
                },
                "required": ["app_name", "menu_path"]
            })),
        ),
        handler_select_menu(),
    );

    // 7. Manage windows
    registry.register(
        "ui_controller_manage_window",
        Tool::new(
            "ui_controller_manage_window",
            "Perform window management operations: list, close, minimize, fullscreen, focus, move, or resize windows of an application.",
            schema_from_json(json!({
                "type": "object",
                "properties": {
                    "app_name": {
                        "type": "string",
                        "description": "Name of the application."
                    },
                    "action": {
                        "type": "string",
                        "description": "Window action to perform.",
                        "enum": ["list", "close", "minimize", "fullscreen", "focus", "move", "resize"]
                    },
                    "x": {
                        "type": "integer",
                        "description": "X position for 'move' action."
                    },
                    "y": {
                        "type": "integer",
                        "description": "Y position for 'move' action."
                    },
                    "width": {
                        "type": "integer",
                        "description": "Width for 'resize' action."
                    },
                    "height": {
                        "type": "integer",
                        "description": "Height for 'resize' action."
                    }
                },
                "required": ["app_name", "action"]
            })),
        ),
        handler_manage_window(),
    );

    // 8. Manage applications (open/close)
    registry.register(
        "ui_controller_manage_app",
        Tool::new(
            "ui_controller_manage_app",
            "Open, close, or force-quit a macOS application.",
            schema_from_json(json!({
                "type": "object",
                "properties": {
                    "app_name": {
                        "type": "string",
                        "description": "Name of the application."
                    },
                    "action": {
                        "type": "string",
                        "description": "Action to perform.",
                        "enum": ["open", "close"]
                    },
                    "force": {
                        "type": "boolean",
                        "description": "If true and action is 'close', force-quit the application. Default false."
                    }
                },
                "required": ["app_name", "action"]
            })),
        ),
        handler_manage_app(),
    );

    // 9. Interact with file dialogs
    registry.register(
        "ui_controller_file_dialog",
        Tool::new(
            "ui_controller_file_dialog",
            "Interact with open/save file dialogs. Navigate to a path, set a filename, confirm, or cancel the dialog.",
            schema_from_json(json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "description": "Dialog action to perform.",
                        "enum": ["navigate", "set_filename", "confirm", "cancel"]
                    },
                    "path": {
                        "type": "string",
                        "description": "Directory path to navigate to (used with 'navigate' action)."
                    },
                    "filename": {
                        "type": "string",
                        "description": "Filename to type into the save field (used with 'set_filename' action)."
                    }
                },
                "required": ["action"]
            })),
        ),
        handler_file_dialog(),
    );

    // 10. Control Dock items
    registry.register(
        "ui_controller_dock",
        Tool::new(
            "ui_controller_dock",
            "Click an application icon in the macOS Dock.",
            schema_from_json(json!({
                "type": "object",
                "properties": {
                    "app_name": {
                        "type": "string",
                        "description": "Name of the application in the Dock to click."
                    },
                    "action": {
                        "type": "string",
                        "description": "Action to perform. Default 'click'.",
                        "enum": ["click"]
                    }
                },
                "required": ["app_name"]
            })),
        ),
        handler_dock(),
    );
}

// ---------------------------------------------------------------------------
// Handler implementations
// ---------------------------------------------------------------------------

fn handler_click() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            let button = args
                .get("button")
                .and_then(|v| v.as_str())
                .unwrap_or("left");
            let click_count = args
                .get("click_count")
                .and_then(|v| v.as_u64())
                .unwrap_or(1);

            let has_coords = args.get("x").is_some() && args.get("y").is_some();
            let has_element =
                args.get("app_name").is_some() && args.get("element_name").is_some();

            if !has_coords && !has_element {
                return Ok(error_result(
                    "Provide either x/y coordinates or app_name + element_name.",
                ));
            }

            let script = if has_element {
                let app_name = args["app_name"].as_str().unwrap();
                let element_name = args["element_name"].as_str().unwrap();
                let escaped_app = app_name.replace('"', "\\\"");
                let escaped_el = element_name.replace('"', "\\\"");

                if button == "right" {
                    // Simulate right-click via AXShowMenu accessibility action
                    format!(
                        r#"
                        tell application "System Events"
                            tell process "{escaped_app}"
                                set frontmost to true
                                perform action "AXShowMenu" of button "{escaped_el}" of window 1
                            end tell
                        end tell
                        return "Right-clicked element '{escaped_el}' in '{escaped_app}'"
                        "#
                    )
                } else if click_count > 1 {
                    format!(
                        r#"
                        tell application "System Events"
                            tell process "{escaped_app}"
                                set frontmost to true
                                repeat {click_count} times
                                    click button "{escaped_el}" of window 1
                                end repeat
                            end tell
                        end tell
                        return "Clicked element '{escaped_el}' in '{escaped_app}' {click_count} time(s)"
                        "#
                    )
                } else {
                    format!(
                        r#"
                        tell application "System Events"
                            tell process "{escaped_app}"
                                set frontmost to true
                                click button "{escaped_el}" of window 1
                            end tell
                        end tell
                        return "Clicked element '{escaped_el}' in '{escaped_app}'"
                        "#
                    )
                }
            } else {
                let x = args["x"].as_i64().unwrap();
                let y = args["y"].as_i64().unwrap();

                if button == "right" {
                    format!(
                        r#"
                        do shell script "/usr/local/bin/cliclick rc:{x},{y} 2>/dev/null || true"
                        return "Right-clicked at ({x}, {y})"
                        "#
                    )
                } else if click_count == 2 {
                    format!(
                        r#"
                        tell application "System Events"
                            click at {{{x}, {y}}}
                            delay 0.05
                            click at {{{x}, {y}}}
                        end tell
                        return "Double-clicked at ({x}, {y})"
                        "#
                    )
                } else if click_count > 2 {
                    format!(
                        r#"
                        tell application "System Events"
                            repeat {click_count} times
                                click at {{{x}, {y}}}
                                delay 0.05
                            end repeat
                        end tell
                        return "Clicked at ({x}, {y}) {click_count} time(s)"
                        "#
                    )
                } else {
                    format!(
                        r#"
                        tell application "System Events"
                            click at {{{x}, {y}}}
                        end tell
                        return "Clicked at ({x}, {y})"
                        "#
                    )
                }
            };

            match crate::macos::applescript::run_applescript(&script) {
                Ok(result) => Ok(text_result(result)),
                Err(e) => Ok(error_result(format!("Click failed: {e}"))),
            }
        })
    })
}

fn handler_type_text() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            let text = args
                .get("text")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("text is required"))?;

            let escaped_text = text.replace('\\', "\\\\").replace('"', "\\\"");

            let activate_block =
                if let Some(app_name) = args.get("app_name").and_then(|v| v.as_str()) {
                    let escaped_app = app_name.replace('"', "\\\"");
                    format!(
                        r#"
                tell application "{escaped_app}" to activate
                delay 0.3
                "#
                    )
                } else {
                    String::new()
                };

            let script = format!(
                r#"
                {activate_block}
                tell application "System Events"
                    keystroke "{escaped_text}"
                end tell
                return "Typed text successfully"
                "#
            );

            match crate::macos::applescript::run_applescript(&script) {
                Ok(result) => Ok(text_result(result)),
                Err(e) => Ok(error_result(format!("Type text failed: {e}"))),
            }
        })
    })
}

fn handler_press_key() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            let key = args
                .get("key")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("key is required"))?;

            let modifiers: Vec<String> = args
                .get("modifiers")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|m| m.as_str().map(|s| format!("{s} down")))
                        .collect()
                })
                .unwrap_or_default();

            let using_clause = if modifiers.is_empty() {
                String::new()
            } else {
                format!(" using {{{}}}", modifiers.join(", "))
            };

            let activate_block =
                if let Some(app_name) = args.get("app_name").and_then(|v| v.as_str()) {
                    let escaped_app = app_name.replace('"', "\\\"");
                    format!(
                        r#"
                tell application "{escaped_app}" to activate
                delay 0.3
                "#
                    )
                } else {
                    String::new()
                };

            // Use key code for special keys, keystroke for characters
            let key_action = if let Some(code) = key_name_to_code(key) {
                format!("key code {code}{using_clause}")
            } else {
                // Single character or short string: use keystroke
                let escaped_key = key.replace('\\', "\\\\").replace('"', "\\\"");
                format!("keystroke \"{escaped_key}\"{using_clause}")
            };

            let script = format!(
                r#"
                {activate_block}
                tell application "System Events"
                    {key_action}
                end tell
                return "Pressed key: {key}"
                "#
            );

            match crate::macos::applescript::run_applescript(&script) {
                Ok(result) => Ok(text_result(result)),
                Err(e) => Ok(error_result(format!("Press key failed: {e}"))),
            }
        })
    })
}

fn handler_scroll() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            let direction = args
                .get("direction")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("direction is required"))?;

            let amount = args
                .get("amount")
                .and_then(|v| v.as_i64())
                .unwrap_or(3);

            let activate_block =
                if let Some(app_name) = args.get("app_name").and_then(|v| v.as_str()) {
                    let escaped_app = app_name.replace('"', "\\\"");
                    format!(
                        r#"
                tell application "{escaped_app}" to activate
                delay 0.3
                "#
                    )
                } else {
                    String::new()
                };

            // Map direction to arrow key codes and repeat
            let key_code = match direction {
                "up" => 126,
                "down" => 125,
                "left" => 123,
                "right" => 124,
                _ => {
                    return Ok(error_result(format!(
                        "Invalid direction: {direction}. Use up, down, left, or right."
                    )));
                }
            };

            let script = format!(
                r#"
                {activate_block}
                tell application "System Events"
                    repeat {amount} times
                        key code {key_code}
                        delay 0.05
                    end repeat
                end tell
                return "Scrolled {direction} {amount} steps"
                "#
            );

            match crate::macos::applescript::run_applescript(&script) {
                Ok(result) => Ok(text_result(result)),
                Err(e) => Ok(error_result(format!("Scroll failed: {e}"))),
            }
        })
    })
}

fn handler_drag() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            let from_x = args
                .get("from_x")
                .and_then(|v| v.as_i64())
                .ok_or_else(|| anyhow::anyhow!("from_x is required"))?;
            let from_y = args
                .get("from_y")
                .and_then(|v| v.as_i64())
                .ok_or_else(|| anyhow::anyhow!("from_y is required"))?;
            let to_x = args
                .get("to_x")
                .and_then(|v| v.as_i64())
                .ok_or_else(|| anyhow::anyhow!("to_x is required"))?;
            let to_y = args
                .get("to_y")
                .and_then(|v| v.as_i64())
                .ok_or_else(|| anyhow::anyhow!("to_y is required"))?;

            // Use cliclick for drag operations, with a Python/Quartz fallback
            let script = format!(
                r#"
                do shell script "/usr/local/bin/cliclick dd:{from_x},{from_y} du:{to_x},{to_y} 2>/dev/null"
                return "Dragged from ({from_x}, {from_y}) to ({to_x}, {to_y})"
                "#
            );

            match crate::macos::applescript::run_applescript(&script) {
                Ok(result) => Ok(text_result(result)),
                Err(e) => {
                    // Fallback: use CGEvent-based approach via Python/Quartz
                    let fallback = format!(
                        r#"
                        do shell script "python3 -c '
import Quartz, time
start = ({from_x}, {from_y})
end = ({to_x}, {to_y})
ev = Quartz.CGEventCreateMouseEvent(None, Quartz.kCGEventLeftMouseDown, start, Quartz.kCGMouseButtonLeft)
Quartz.CGEventPost(Quartz.kCGHIDEventTap, ev)
time.sleep(0.1)
ev = Quartz.CGEventCreateMouseEvent(None, Quartz.kCGEventLeftMouseDragged, end, Quartz.kCGMouseButtonLeft)
Quartz.CGEventPost(Quartz.kCGHIDEventTap, ev)
time.sleep(0.1)
ev = Quartz.CGEventCreateMouseEvent(None, Quartz.kCGEventLeftMouseUp, end, Quartz.kCGMouseButtonLeft)
Quartz.CGEventPost(Quartz.kCGHIDEventTap, ev)
'"
                        return "Dragged from ({from_x}, {from_y}) to ({to_x}, {to_y})"
                        "#
                    );
                    match crate::macos::applescript::run_applescript(&fallback) {
                        Ok(result) => Ok(text_result(result)),
                        Err(e2) => Ok(error_result(format!(
                            "Drag failed. cliclick error: {e}, fallback error: {e2}"
                        ))),
                    }
                }
            }
        })
    })
}

fn handler_select_menu() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            let app_name = args
                .get("app_name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("app_name is required"))?;

            let menu_path = args
                .get("menu_path")
                .and_then(|v| v.as_array())
                .ok_or_else(|| anyhow::anyhow!("menu_path is required and must be an array"))?;

            if menu_path.is_empty() {
                return Ok(error_result("menu_path must contain at least one item."));
            }

            let escaped_app = app_name.replace('"', "\\\"");

            // Build the nested menu AppleScript expression.
            // ["File", "Save As..."] =>
            //   click menu item "Save As..." of menu "File" of menu bar 1
            // ["Edit", "Find", "Find..."] =>
            //   click menu item "Find..." of menu "Find" of menu item "Find" of menu "Edit" of menu bar 1
            let items: Vec<String> = menu_path
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.replace('"', "\\\"")))
                .collect();

            let click_expr = if items.len() == 1 {
                format!(r#"click menu "{}" of menu bar 1"#, items[0])
            } else {
                // Last item is the menu item to click
                let last = &items[items.len() - 1];
                let first = &items[0];

                let mut chain = format!(r#"click menu item "{last}""#);

                // Middle items form submenu chain (from second-to-last back to second)
                for i in (1..items.len() - 1).rev() {
                    chain = format!(
                        r#"{chain} of menu "{}" of menu item "{}""#,
                        items[i], items[i]
                    );
                }

                chain = format!(r#"{chain} of menu "{first}" of menu bar 1"#);
                chain
            };

            let menu_display = items.join(" > ");
            let script = format!(
                r#"
                tell application "{escaped_app}" to activate
                delay 0.3
                tell application "System Events"
                    tell process "{escaped_app}"
                        {click_expr}
                    end tell
                end tell
                return "Selected menu: {menu_display}"
                "#
            );

            match crate::macos::applescript::run_applescript(&script) {
                Ok(result) => Ok(text_result(result)),
                Err(e) => Ok(error_result(format!("Select menu failed: {e}"))),
            }
        })
    })
}

fn handler_manage_window() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            let app_name = args
                .get("app_name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("app_name is required"))?;

            let action = args
                .get("action")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("action is required"))?;

            let escaped_app = app_name.replace('"', "\\\"");

            let script = match action {
                "list" => {
                    format!(
                        r#"
                        set output to ""
                        tell application "System Events"
                            tell process "{escaped_app}"
                                set winCount to count of windows
                                if winCount is 0 then
                                    return "No windows found for {escaped_app}"
                                end if
                                repeat with i from 1 to winCount
                                    set w to window i
                                    set wName to name of w
                                    set wPos to position of w
                                    set wSize to size of w
                                    set output to output & "Window " & i & ": " & wName & " at (" & (item 1 of wPos) & ", " & (item 2 of wPos) & ") size (" & (item 1 of wSize) & "x" & (item 2 of wSize) & ")" & linefeed
                                end repeat
                            end tell
                        end tell
                        return output
                        "#
                    )
                }
                "close" => {
                    format!(
                        r#"
                        tell application "System Events"
                            tell process "{escaped_app}"
                                if (count of windows) > 0 then
                                    click button 1 of window 1
                                end if
                            end tell
                        end tell
                        return "Closed front window of {escaped_app}"
                        "#
                    )
                }
                "minimize" => {
                    format!(
                        r#"
                        tell application "System Events"
                            tell process "{escaped_app}"
                                if (count of windows) > 0 then
                                    set value of attribute "AXMinimized" of window 1 to true
                                end if
                            end tell
                        end tell
                        return "Minimized front window of {escaped_app}"
                        "#
                    )
                }
                "fullscreen" => {
                    format!(
                        r#"
                        tell application "System Events"
                            tell process "{escaped_app}"
                                if (count of windows) > 0 then
                                    set value of attribute "AXFullScreen" of window 1 to true
                                end if
                            end tell
                        end tell
                        return "Toggled fullscreen for {escaped_app}"
                        "#
                    )
                }
                "focus" => {
                    format!(
                        r#"
                        tell application "{escaped_app}" to activate
                        return "Focused {escaped_app}"
                        "#
                    )
                }
                "move" => {
                    let x = args
                        .get("x")
                        .and_then(|v| v.as_i64())
                        .ok_or_else(|| anyhow::anyhow!("x is required for move action"))?;
                    let y = args
                        .get("y")
                        .and_then(|v| v.as_i64())
                        .ok_or_else(|| anyhow::anyhow!("y is required for move action"))?;
                    format!(
                        r#"
                        tell application "System Events"
                            tell process "{escaped_app}"
                                if (count of windows) > 0 then
                                    set position of window 1 to {{{x}, {y}}}
                                end if
                            end tell
                        end tell
                        return "Moved {escaped_app} window to ({x}, {y})"
                        "#
                    )
                }
                "resize" => {
                    let width = args
                        .get("width")
                        .and_then(|v| v.as_i64())
                        .ok_or_else(|| {
                            anyhow::anyhow!("width is required for resize action")
                        })?;
                    let height = args
                        .get("height")
                        .and_then(|v| v.as_i64())
                        .ok_or_else(|| {
                            anyhow::anyhow!("height is required for resize action")
                        })?;
                    format!(
                        r#"
                        tell application "System Events"
                            tell process "{escaped_app}"
                                if (count of windows) > 0 then
                                    set size of window 1 to {{{width}, {height}}}
                                end if
                            end tell
                        end tell
                        return "Resized {escaped_app} window to {width}x{height}"
                        "#
                    )
                }
                _ => {
                    return Ok(error_result(format!(
                        "Unknown window action: {action}. Use list, close, minimize, fullscreen, focus, move, or resize."
                    )));
                }
            };

            match crate::macos::applescript::run_applescript(&script) {
                Ok(result) => Ok(text_result(result)),
                Err(e) => Ok(error_result(format!("Window management failed: {e}"))),
            }
        })
    })
}

fn handler_manage_app() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            let app_name = args
                .get("app_name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("app_name is required"))?;

            let action = args
                .get("action")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("action is required"))?;

            let force = args
                .get("force")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            let escaped_app = app_name.replace('"', "\\\"");
            let escaped_app_shell = app_name.replace('\'', "'\\''");

            let script = match action {
                "open" => {
                    format!(
                        r#"
                        tell application "{escaped_app}" to activate
                        return "Opened {escaped_app}"
                        "#
                    )
                }
                "close" => {
                    if force {
                        format!(
                            r#"
                            do shell script "killall '{escaped_app_shell}' 2>/dev/null || true"
                            return "Force-quit {escaped_app}"
                            "#
                        )
                    } else {
                        format!(
                            r#"
                            tell application "{escaped_app}" to quit
                            return "Closed {escaped_app}"
                            "#
                        )
                    }
                }
                _ => {
                    return Ok(error_result(format!(
                        "Unknown app action: {action}. Use 'open' or 'close'."
                    )));
                }
            };

            match crate::macos::applescript::run_applescript(&script) {
                Ok(result) => Ok(text_result(result)),
                Err(e) => Ok(error_result(format!("App management failed: {e}"))),
            }
        })
    })
}

fn handler_file_dialog() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            let action = args
                .get("action")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("action is required"))?;

            let script = match action {
                "navigate" => {
                    let path = args
                        .get("path")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| {
                            anyhow::anyhow!("path is required for navigate action")
                        })?;
                    let escaped_path = path.replace('\\', "\\\\").replace('"', "\\\"");

                    // Cmd+Shift+G opens "Go to Folder", then type the path and press Return
                    format!(
                        r#"
                        tell application "System Events"
                            keystroke "g" using {{command down, shift down}}
                            delay 0.5
                            keystroke "{escaped_path}"
                            delay 0.2
                            key code 36
                        end tell
                        delay 0.3
                        return "Navigated to: {escaped_path}"
                        "#
                    )
                }
                "set_filename" => {
                    let filename = args
                        .get("filename")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| {
                            anyhow::anyhow!("filename is required for set_filename action")
                        })?;
                    let escaped_filename =
                        filename.replace('\\', "\\\\").replace('"', "\\\"");

                    // Select all in filename field and type the new name
                    format!(
                        r#"
                        tell application "System Events"
                            keystroke "a" using {{command down}}
                            delay 0.1
                            keystroke "{escaped_filename}"
                        end tell
                        return "Set filename to: {escaped_filename}"
                        "#
                    )
                }
                "confirm" => {
                    r#"
                    tell application "System Events"
                        key code 36
                    end tell
                    return "Confirmed file dialog"
                    "#
                    .to_string()
                }
                "cancel" => {
                    r#"
                    tell application "System Events"
                        key code 53
                    end tell
                    return "Cancelled file dialog"
                    "#
                    .to_string()
                }
                _ => {
                    return Ok(error_result(format!(
                        "Unknown file dialog action: {action}. Use navigate, set_filename, confirm, or cancel."
                    )));
                }
            };

            match crate::macos::applescript::run_applescript(&script) {
                Ok(result) => Ok(text_result(result)),
                Err(e) => Ok(error_result(format!("File dialog interaction failed: {e}"))),
            }
        })
    })
}

fn handler_dock() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            let app_name = args
                .get("app_name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("app_name is required"))?;

            let escaped_app = app_name.replace('"', "\\\"");

            let script = format!(
                r#"
                tell application "System Events"
                    tell process "Dock"
                        click UI element "{escaped_app}" of list 1
                    end tell
                end tell
                return "Clicked '{escaped_app}' in Dock"
                "#
            );

            match crate::macos::applescript::run_applescript(&script) {
                Ok(result) => Ok(text_result(result)),
                Err(e) => Ok(error_result(format!("Dock interaction failed: {e}"))),
            }
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_schemas_valid() {
        let mut registry = ServiceRegistry::new();
        register(&mut registry);
        let tools = registry.list_tools();
        assert_eq!(tools.len(), 10, "Expected exactly 10 ui_controller tools");

        let names: Vec<_> = tools.iter().map(|t| t.name.as_ref()).collect();
        assert!(names.contains(&"ui_controller_click"));
        assert!(names.contains(&"ui_controller_type_text"));
        assert!(names.contains(&"ui_controller_press_key"));
        assert!(names.contains(&"ui_controller_scroll"));
        assert!(names.contains(&"ui_controller_drag"));
        assert!(names.contains(&"ui_controller_select_menu"));
        assert!(names.contains(&"ui_controller_manage_window"));
        assert!(names.contains(&"ui_controller_manage_app"));
        assert!(names.contains(&"ui_controller_file_dialog"));
        assert!(names.contains(&"ui_controller_dock"));
    }

    #[test]
    fn test_key_name_to_code_known_keys() {
        assert_eq!(key_name_to_code("return"), Some(36));
        assert_eq!(key_name_to_code("Return"), Some(36));
        assert_eq!(key_name_to_code("enter"), Some(36));
        assert_eq!(key_name_to_code("tab"), Some(48));
        assert_eq!(key_name_to_code("escape"), Some(53));
        assert_eq!(key_name_to_code("esc"), Some(53));
        assert_eq!(key_name_to_code("space"), Some(49));
        assert_eq!(key_name_to_code("delete"), Some(51));
        assert_eq!(key_name_to_code("backspace"), Some(51));
        assert_eq!(key_name_to_code("up"), Some(126));
        assert_eq!(key_name_to_code("down"), Some(125));
        assert_eq!(key_name_to_code("left"), Some(123));
        assert_eq!(key_name_to_code("right"), Some(124));
        assert_eq!(key_name_to_code("f1"), Some(122));
        assert_eq!(key_name_to_code("f12"), Some(111));
    }

    #[test]
    fn test_key_name_to_code_unknown_key() {
        assert_eq!(key_name_to_code("a"), None);
        assert_eq!(key_name_to_code("z"), None);
        assert_eq!(key_name_to_code("unknown"), None);
    }
}
