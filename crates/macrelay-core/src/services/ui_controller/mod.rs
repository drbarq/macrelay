use std::sync::Arc;

use rmcp::model::Tool;
use serde_json::json;

use crate::macos::escape::{escape_applescript_string, escape_shell_single_quoted};
use crate::registry::{ServiceRegistry, ToolHandler, error_result, schema_from_json, text_result};

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

            let has_coords = args.contains_key("x") && args.contains_key("y");
            let has_element = args.contains_key("app_name") && args.contains_key("element_name");

            if !has_coords && !has_element {
                return Ok(error_result(
                    "Provide either x/y coordinates or app_name + element_name.",
                ));
            }

            let script = if has_element {
                let app_name = args["app_name"].as_str().unwrap();
                let element_name = args["element_name"].as_str().unwrap();
                let escaped_app = escape_applescript_string(app_name);
                let escaped_el = escape_applescript_string(element_name);

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
            let text = match args.get("text").and_then(|v| v.as_str()) {
                Some(t) => t,
                None => return Ok(error_result("text is required")),
            };

            let escaped_text = escape_applescript_string(text);

            let activate_block =
                if let Some(app_name) = args.get("app_name").and_then(|v| v.as_str()) {
                    let escaped_app = escape_applescript_string(app_name);
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
            let key = match args.get("key").and_then(|v| v.as_str()) {
                Some(k) => k,
                None => return Ok(error_result("key is required")),
            };

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
                    let escaped_app = escape_applescript_string(app_name);
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
                let escaped_key = escape_applescript_string(key);
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
            let direction = match args.get("direction").and_then(|v| v.as_str()) {
                Some(d) => d,
                None => return Ok(error_result("direction is required")),
            };

            let amount = args.get("amount").and_then(|v| v.as_i64()).unwrap_or(3);

            let activate_block =
                if let Some(app_name) = args.get("app_name").and_then(|v| v.as_str()) {
                    let escaped_app = escape_applescript_string(app_name);
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
            let from_x = match args.get("from_x").and_then(|v| v.as_i64()) {
                Some(x) => x,
                None => return Ok(error_result("from_x is required")),
            };
            let from_y = match args.get("from_y").and_then(|v| v.as_i64()) {
                Some(y) => y,
                None => return Ok(error_result("from_y is required")),
            };
            let to_x = match args.get("to_x").and_then(|v| v.as_i64()) {
                Some(x) => x,
                None => return Ok(error_result("to_x is required")),
            };
            let to_y = match args.get("to_y").and_then(|v| v.as_i64()) {
                Some(y) => y,
                None => return Ok(error_result("to_y is required")),
            };

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
            let app_name = match args.get("app_name").and_then(|v| v.as_str()) {
                Some(a) => a,
                None => return Ok(error_result("app_name is required")),
            };

            let menu_path = match args.get("menu_path").and_then(|v| v.as_array()) {
                Some(mp) => mp,
                None => return Ok(error_result("menu_path is required and must be an array")),
            };

            if menu_path.is_empty() {
                return Ok(error_result("menu_path must contain at least one item."));
            }

            let escaped_app = escape_applescript_string(app_name);

            // Build the nested menu AppleScript expression.
            // ["File", "Save As..."] =>
            //   click menu item "Save As..." of menu "File" of menu bar 1
            // ["Edit", "Find", "Find..."] =>
            //   click menu item "Find..." of menu "Find" of menu item "Find" of menu "Edit" of menu bar 1
            let items: Vec<String> = menu_path
                .iter()
                .filter_map(|v| v.as_str().map(escape_applescript_string))
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
            let app_name = match args.get("app_name").and_then(|v| v.as_str()) {
                Some(a) => a,
                None => return Ok(error_result("app_name is required")),
            };

            let action = match args.get("action").and_then(|v| v.as_str()) {
                Some(a) => a,
                None => return Ok(error_result("action is required")),
            };

            let escaped_app = escape_applescript_string(app_name);

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
                    let x = match args.get("x").and_then(|v| v.as_i64()) {
                        Some(x) => x,
                        None => return Ok(error_result("x is required for move action")),
                    };
                    let y = match args.get("y").and_then(|v| v.as_i64()) {
                        Some(y) => y,
                        None => return Ok(error_result("y is required for move action")),
                    };
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
                    let width = match args.get("width").and_then(|v| v.as_i64()) {
                        Some(w) => w,
                        None => return Ok(error_result("width is required for resize action")),
                    };
                    let height = match args.get("height").and_then(|v| v.as_i64()) {
                        Some(h) => h,
                        None => return Ok(error_result("height is required for resize action")),
                    };
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
            let app_name = match args.get("app_name").and_then(|v| v.as_str()) {
                Some(a) => a,
                None => return Ok(error_result("app_name is required")),
            };

            let action = match args.get("action").and_then(|v| v.as_str()) {
                Some(a) => a,
                None => return Ok(error_result("action is required")),
            };

            let force = args.get("force").and_then(|v| v.as_bool()).unwrap_or(false);

            let escaped_app = escape_applescript_string(app_name);
            let escaped_app_shell = escape_shell_single_quoted(app_name);

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
            let action = match args.get("action").and_then(|v| v.as_str()) {
                Some(a) => a,
                None => return Ok(error_result("action is required")),
            };

            let script = match action {
                "navigate" => {
                    let path = match args.get("path").and_then(|v| v.as_str()) {
                        Some(p) => p,
                        None => return Ok(error_result("path is required for navigate action")),
                    };
                    let escaped_path = escape_applescript_string(path);

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
                    let filename = match args.get("filename").and_then(|v| v.as_str()) {
                        Some(f) => f,
                        None => {
                            return Ok(error_result(
                                "filename is required for set_filename action",
                            ));
                        }
                    };
                    let escaped_filename = escape_applescript_string(filename);

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
                "confirm" => r#"
                    tell application "System Events"
                        key code 36
                    end tell
                    return "Confirmed file dialog"
                    "#
                .to_string(),
                "cancel" => r#"
                    tell application "System Events"
                        key code 53
                    end tell
                    return "Cancelled file dialog"
                    "#
                .to_string(),
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
            let app_name = match args.get("app_name").and_then(|v| v.as_str()) {
                Some(a) => a,
                None => return Ok(error_result("app_name is required")),
            };

            let escaped_app = escape_applescript_string(app_name);

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

    struct AssertingMock {
        expectations: Mutex<Vec<(String, Result<String, String>)>>,
    }

    impl AssertingMock {
        fn new() -> Self {
            Self {
                expectations: Mutex::new(Vec::new()),
            }
        }

        fn expect(self, fragment: &str, response: Result<&str, &str>) -> Self {
            self.expectations.lock().unwrap().push((
                fragment.to_string(),
                response.map(|s| s.to_string()).map_err(|s| s.to_string()),
            ));
            self
        }
    }

    impl ScriptRunner for AssertingMock {
        fn run_applescript(&self, script: &str) -> anyhow::Result<String> {
            let mut expectations = self.expectations.lock().unwrap();
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
            response.map_err(|e| anyhow::anyhow!(e))
        }

        fn run_applescript_with_timeout(
            &self,
            script: &str,
            _timeout: Duration,
        ) -> anyhow::Result<String> {
            self.run_applescript(script)
        }

        fn run_jxa(&self, script: &str) -> anyhow::Result<String> {
            self.run_applescript(script)
        }
    }

    #[tokio::test]
    async fn test_ui_controller_click_coords() {
        let mock = Arc::new(
            AssertingMock::new().expect("click at {100, 200}", Ok("Clicked at (100, 200)")),
        );

        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_click();
                let mut args = HashMap::new();
                args.insert("x".to_string(), json!(100));
                args.insert("y".to_string(), json!(200));

                let result = handler(args).await.unwrap();
                assert_eq!(result.is_error, Some(false));
                assert!(
                    result.content[0]
                        .as_text()
                        .unwrap()
                        .text
                        .contains("Clicked at (100, 200)")
                );
            })
            .await;
    }

    #[tokio::test]
    async fn test_ui_controller_click_element() {
        let mock = Arc::new(AssertingMock::new().expect(
            "click button \"Submit\" of window 1",
            Ok("Clicked element 'Submit' in 'Safari'"),
        ));

        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_click();
                let mut args = HashMap::new();
                args.insert("app_name".to_string(), json!("Safari"));
                args.insert("element_name".to_string(), json!("Submit"));

                let result = handler(args).await.unwrap();
                assert_eq!(result.is_error, Some(false));
                assert!(
                    result.content[0]
                        .as_text()
                        .unwrap()
                        .text
                        .contains("Clicked element 'Submit' in 'Safari'")
                );
            })
            .await;
    }

    #[tokio::test]
    async fn test_ui_controller_type_text() {
        let mock = Arc::new(
            AssertingMock::new().expect("keystroke \"Hello World\"", Ok("Typed text successfully")),
        );

        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_type_text();
                let mut args = HashMap::new();
                args.insert("text".to_string(), json!("Hello World"));
                args.insert("app_name".to_string(), json!("Notes"));

                let result = handler(args).await.unwrap();
                assert_eq!(result.is_error, Some(false));
                assert!(
                    result.content[0]
                        .as_text()
                        .unwrap()
                        .text
                        .contains("Typed text successfully")
                );
            })
            .await;
    }

    #[tokio::test]
    async fn test_ui_controller_type_text_escaping() {
        let mock = Arc::new(AssertingMock::new().expect(
            "keystroke \"Text with \\\"quotes\\\" and \\\\backslash\"",
            Ok("Typed text successfully"),
        ));

        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_type_text();
                let mut args = HashMap::new();
                args.insert(
                    "text".to_string(),
                    json!("Text with \"quotes\" and \\backslash"),
                );
                args.insert("app_name".to_string(), json!("Notes"));

                let result = handler(args).await.unwrap();
                assert_eq!(result.is_error, Some(false));
            })
            .await;
    }

    #[tokio::test]
    async fn test_ui_controller_press_key() {
        let mock = Arc::new(AssertingMock::new().expect(
            "key code 36 using {command down}",
            Ok("Pressed key: return"),
        ));

        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_press_key();
                let mut args = HashMap::new();
                args.insert("key".to_string(), json!("return"));
                args.insert("modifiers".to_string(), json!(["command"]));

                let result = handler(args).await.unwrap();
                assert_eq!(result.is_error, Some(false));
                assert!(
                    result.content[0]
                        .as_text()
                        .unwrap()
                        .text
                        .contains("Pressed key: return")
                );
            })
            .await;
    }

    #[tokio::test]
    async fn test_ui_controller_scroll() {
        let mock = Arc::new(AssertingMock::new().expect(
            "repeat 5 times\n                        key code 125",
            Ok("Scrolled down 5 steps"),
        ));

        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_scroll();
                let mut args = HashMap::new();
                args.insert("direction".to_string(), json!("down"));
                args.insert("amount".to_string(), json!(5));

                let result = handler(args).await.unwrap();
                assert_eq!(result.is_error, Some(false));
                assert!(
                    result.content[0]
                        .as_text()
                        .unwrap()
                        .text
                        .contains("Scrolled down 5 steps")
                );
            })
            .await;
    }

    #[tokio::test]
    async fn test_ui_controller_drag() {
        let mock = Arc::new(AssertingMock::new().expect(
            "cliclick dd:100,100 du:500,500",
            Ok("Dragged from (100, 100) to (500, 500)"),
        ));

        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_drag();
                let mut args = HashMap::new();
                args.insert("from_x".to_string(), json!(100));
                args.insert("from_y".to_string(), json!(100));
                args.insert("to_x".to_string(), json!(500));
                args.insert("to_y".to_string(), json!(500));

                let result = handler(args).await.unwrap();
                assert_eq!(result.is_error, Some(false));
                assert!(
                    result.content[0]
                        .as_text()
                        .unwrap()
                        .text
                        .contains("Dragged from (100, 100) to (500, 500)")
                );
            })
            .await;
    }

    #[tokio::test]
    async fn test_ui_controller_select_menu() {
        let mock = Arc::new(AssertingMock::new().expect(
            "click menu item \"Save As...\" of menu \"File\" of menu bar 1",
            Ok("Selected menu: File > Save As..."),
        ));

        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_select_menu();
                let mut args = HashMap::new();
                args.insert("app_name".to_string(), json!("Safari"));
                args.insert("menu_path".to_string(), json!(["File", "Save As..."]));

                let result = handler(args).await.unwrap();
                assert_eq!(result.is_error, Some(false));
                assert!(
                    result.content[0]
                        .as_text()
                        .unwrap()
                        .text
                        .contains("Selected menu: File > Save As...")
                );
            })
            .await;
    }

    #[tokio::test]
    async fn test_ui_controller_manage_window() {
        let mock = Arc::new(AssertingMock::new().expect(
            "set value of attribute \"AXMinimized\" of window 1 to true",
            Ok("Minimized front window of Safari"),
        ));

        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_manage_window();
                let mut args = HashMap::new();
                args.insert("app_name".to_string(), json!("Safari"));
                args.insert("action".to_string(), json!("minimize"));

                let result = handler(args).await.unwrap();
                assert_eq!(result.is_error, Some(false));
                assert!(
                    result.content[0]
                        .as_text()
                        .unwrap()
                        .text
                        .contains("Minimized front window of Safari")
                );
            })
            .await;
    }

    #[tokio::test]
    async fn test_ui_controller_manage_app() {
        let mock = Arc::new(AssertingMock::new().expect(
            "tell application \"Safari\" to activate",
            Ok("Opened Safari"),
        ));

        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_manage_app();
                let mut args = HashMap::new();
                args.insert("app_name".to_string(), json!("Safari"));
                args.insert("action".to_string(), json!("open"));

                let result = handler(args).await.unwrap();
                assert_eq!(result.is_error, Some(false));
                assert!(
                    result.content[0]
                        .as_text()
                        .unwrap()
                        .text
                        .contains("Opened Safari")
                );
            })
            .await;
    }

    #[tokio::test]
    async fn test_ui_controller_file_dialog() {
        let mock = Arc::new(
            AssertingMock::new()
                .expect("keystroke \"/Users/test\"", Ok("Navigated to: /Users/test")),
        );

        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_file_dialog();
                let mut args = HashMap::new();
                args.insert("action".to_string(), json!("navigate"));
                args.insert("path".to_string(), json!("/Users/test"));

                let result = handler(args).await.unwrap();
                assert_eq!(result.is_error, Some(false));
                assert!(
                    result.content[0]
                        .as_text()
                        .unwrap()
                        .text
                        .contains("Navigated to: /Users/test")
                );
            })
            .await;
    }

    #[tokio::test]
    async fn test_ui_controller_dock() {
        let mock = Arc::new(AssertingMock::new().expect(
            "click UI element \"Safari\" of list 1",
            Ok("Clicked 'Safari' in Dock"),
        ));

        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_dock();
                let mut args = HashMap::new();
                args.insert("app_name".to_string(), json!("Safari"));

                let result = handler(args).await.unwrap();
                assert_eq!(result.is_error, Some(false));
                assert!(
                    result.content[0]
                        .as_text()
                        .unwrap()
                        .text
                        .contains("Clicked 'Safari' in Dock")
                );
            })
            .await;
    }

    /// When osascript fails (e.g. Accessibility permission denied), ui_controller
    /// handlers must return a graceful error result instead of panicking or
    /// propagating a raw anyhow error.
    #[tokio::test]
    async fn test_click_returns_error_result_on_osascript_failure() {
        // Use a minimal bespoke mock that always returns Err, rather than the
        // queue-based AssertingMock which expects a fragment match.
        struct ErrorMock;
        impl ScriptRunner for ErrorMock {
            fn run_applescript(&self, _script: &str) -> anyhow::Result<String> {
                Err(anyhow::anyhow!(
                    "osascript: System Events got an error: osascript is not allowed assistive access"
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
                let handler = handler_click();
                let mut args = HashMap::new();
                args.insert("x".to_string(), json!(100));
                args.insert("y".to_string(), json!(200));

                let result = handler(args)
                    .await
                    .expect("Handler should not panic on osascript error");
                assert_eq!(result.is_error, Some(true));

                let content = result.content[0].as_text().unwrap().text.as_str();
                assert!(
                    content.to_lowercase().contains("fail")
                        || content.to_lowercase().contains("error"),
                    "Expected a human-readable error, got: {}",
                    content
                );
                assert!(
                    content.contains("assistive access"),
                    "Expected underlying error to be surfaced, got: {}",
                    content
                );
            })
            .await;
    }

    #[tokio::test]
    async fn test_validation_click_requires_params() {
        let handler = handler_click();
        let args = HashMap::new();

        let result = handler(args).await.expect("Handler should not panic");
        assert_eq!(result.is_error, Some(true));
        assert!(
            result.content[0]
                .as_text()
                .unwrap()
                .text
                .contains("Provide either x/y coordinates or app_name + element_name")
        );
    }

    #[tokio::test]
    async fn test_validation_type_text_requires_text() {
        let handler = handler_type_text();
        let args = HashMap::new();

        let result = handler(args).await.expect("Handler should not panic");
        assert_eq!(result.is_error, Some(true));
        assert!(
            result.content[0]
                .as_text()
                .unwrap()
                .text
                .contains("text is required")
        );
    }
}
