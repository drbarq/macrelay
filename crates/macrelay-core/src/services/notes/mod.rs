use std::sync::Arc;

use rmcp::model::Tool;
use serde_json::json;

use crate::registry::{error_result, schema_from_json, text_result, ServiceRegistry, ToolHandler};

/// Register all Notes tools with the service registry.
pub fn register(registry: &mut ServiceRegistry) {
    registry.register(
        "notes_list_accounts",
        Tool::new(
            "notes_list_accounts",
            "List all Notes accounts (e.g. iCloud, On My Mac, Gmail).",
            schema_from_json(json!({
                "type": "object",
                "properties": {},
            })),
        ),
        handler_list_accounts(),
    );

    registry.register(
        "notes_list_folders",
        Tool::new(
            "notes_list_folders",
            "List all folders across all Notes accounts.",
            schema_from_json(json!({
                "type": "object",
                "properties": {},
            })),
        ),
        handler_list_folders(),
    );

    registry.register(
        "notes_search_notes",
        Tool::new(
            "notes_search_notes",
            "Search notes by text query. Returns matching note names with their folder and account.",
            schema_from_json(json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Text to search for in note names."
                    },
                    "folder": {
                        "type": "string",
                        "description": "Optional folder name to restrict the search to."
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of notes to return. Default 50."
                    },
                    "offset": {
                        "type": "integer",
                        "description": "Number of matching notes to skip. Default 0."
                    }
                },
                "required": ["query"]
            })),
        ),
        handler_search_notes(),
    );

    registry.register(
        "notes_read_note",
        Tool::new(
            "notes_read_note",
            "Read the full content of a note by its name. Returns the HTML body of the note.",
            schema_from_json(json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "The name (title) of the note to read."
                    }
                },
                "required": ["name"]
            })),
        ),
        handler_read_note(),
    );

    registry.register(
        "notes_write_note",
        Tool::new(
            "notes_write_note",
            "Create a new note with a title and HTML body. Optionally specify a folder.",
            schema_from_json(json!({
                "type": "object",
                "properties": {
                    "title": {
                        "type": "string",
                        "description": "The title of the new note."
                    },
                    "body": {
                        "type": "string",
                        "description": "The HTML body content of the note."
                    },
                    "folder": {
                        "type": "string",
                        "description": "Folder to create the note in. Defaults to the default 'Notes' folder."
                    }
                },
                "required": ["title", "body"]
            })),
        ),
        handler_write_note(),
    );

    registry.register(
        "notes_delete_note",
        Tool::new(
            "notes_delete_note",
            "Delete a note by name. The note is moved to Recently Deleted.",
            schema_from_json(json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "The name (title) of the note to delete."
                    }
                },
                "required": ["name"]
            })),
        ),
        handler_delete_note(),
    );

    registry.register(
        "notes_restore_note",
        Tool::new(
            "notes_restore_note",
            "Restore a note from Recently Deleted by moving it back to a target folder.",
            schema_from_json(json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "The name (title) of the note to restore."
                    },
                    "folder": {
                        "type": "string",
                        "description": "Folder to restore the note to. Defaults to 'Notes'."
                    }
                },
                "required": ["name"]
            })),
        ),
        handler_restore_note(),
    );

    registry.register(
        "notes_open_note",
        Tool::new(
            "notes_open_note",
            "Open Notes.app and display a specific note by name.",
            schema_from_json(json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "The name (title) of the note to open."
                    }
                },
                "required": ["name"]
            })),
        ),
        handler_open_note(),
    );
}

fn handler_list_accounts() -> ToolHandler {
    Arc::new(|_args| {
        Box::pin(async move {
            let script = r#"
                tell application "Notes"
                    set output to ""
                    repeat with a in accounts
                        set output to output & name of a & linefeed
                    end repeat
                    return output
                end tell
            "#;

            match crate::macos::applescript::run_applescript(script) {
                Ok(output) => {
                    let accounts: Vec<&str> =
                        output.lines().filter(|l| !l.trim().is_empty()).collect();
                    let result: Vec<serde_json::Value> = accounts
                        .iter()
                        .map(|name| json!({"name": name.trim()}))
                        .collect();
                    let json = serde_json::to_string_pretty(&result)?;
                    Ok(text_result(format!(
                        "Found {} account(s):\n\n{json}",
                        result.len()
                    )))
                }
                Err(e) => Ok(error_result(format!("Failed to list accounts: {e}"))),
            }
        })
    })
}

fn handler_list_folders() -> ToolHandler {
    Arc::new(|_args| {
        Box::pin(async move {
            let script = r#"
                tell application "Notes"
                    set output to ""
                    repeat with a in accounts
                        set acctName to name of a
                        repeat with f in folders of a
                            set output to output & (name of f) & "||" & acctName & linefeed
                        end repeat
                    end repeat
                    return output
                end tell
            "#;

            match crate::macos::applescript::run_applescript(script) {
                Ok(output) => {
                    let mut results: Vec<serde_json::Value> = Vec::new();
                    for line in output.lines() {
                        let line = line.trim();
                        if line.is_empty() {
                            continue;
                        }
                        let parts: Vec<&str> = line.split("||").collect();
                        let folder_name = parts.first().unwrap_or(&"").trim();
                        let account_name = parts.get(1).unwrap_or(&"").trim();
                        results.push(json!({
                            "folder": folder_name,
                            "account": account_name,
                        }));
                    }
                    let json = serde_json::to_string_pretty(&results)?;
                    Ok(text_result(format!(
                        "Found {} folder(s):\n\n{json}",
                        results.len()
                    )))
                }
                Err(e) => Ok(error_result(format!("Failed to list folders: {e}"))),
            }
        })
    })
}

fn handler_search_notes() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            let query = args
                .get("query")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("query is required"))?;

            let folder_filter = args.get("folder").and_then(|v| v.as_str()).unwrap_or("");
            let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(50);
            let offset = args.get("offset").and_then(|v| v.as_u64()).unwrap_or(0);

            let escaped_query = query.replace('"', "\\\"");

            // Build the folder iteration clause
            let folder_clause = if folder_filter.is_empty() {
                "repeat with f in folders of a".to_string()
            } else {
                let escaped_folder = folder_filter.replace('"', "\\\"");
                format!(r#"repeat with f in {{folder "{escaped_folder}" of a}}"#)
            };

            let script = format!(
                r#"
                set searchQuery to "{escaped_query}"
                set maxResults to {limit}
                set skipCount to {offset}
                set matchCount to 0
                set skipped to 0

                tell application "Notes"
                    set output to ""
                    repeat with a in accounts
                        set acctName to name of a
                        {folder_clause}
                            set folderName to name of f
                            repeat with n in notes of f
                                set noteName to name of n
                                if noteName contains searchQuery then
                                    if skipped < skipCount then
                                        set skipped to skipped + 1
                                    else if matchCount < maxResults then
                                        set modDate to modification date of n
                                        set output to output & noteName & "||" & folderName & "||" & acctName & "||" & (modDate as text) & linefeed
                                        set matchCount to matchCount + 1
                                    end if
                                end if
                                if matchCount >= maxResults then exit repeat
                            end repeat
                            if matchCount >= maxResults then exit repeat
                        end repeat
                        if matchCount >= maxResults then exit repeat
                    end repeat
                    return output
                end tell
                "#
            );

            match crate::macos::applescript::run_applescript(&script) {
                Ok(output) => {
                    let mut results: Vec<serde_json::Value> = Vec::new();
                    for line in output.lines() {
                        let line = line.trim();
                        if line.is_empty() {
                            continue;
                        }
                        let parts: Vec<&str> = line.split("||").collect();
                        results.push(json!({
                            "name": parts.first().unwrap_or(&"").trim(),
                            "folder": parts.get(1).unwrap_or(&"").trim(),
                            "account": parts.get(2).unwrap_or(&"").trim(),
                            "modified": parts.get(3).unwrap_or(&"").trim(),
                        }));
                    }
                    let json = serde_json::to_string_pretty(&results)?;
                    Ok(text_result(format!(
                        "Found {} note(s):\n\n{json}",
                        results.len()
                    )))
                }
                Err(e) => Ok(error_result(format!("Failed to search notes: {e}"))),
            }
        })
    })
}

fn handler_read_note() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            let name = args
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("name is required"))?;

            let escaped_name = name.replace('"', "\\\"");

            let script = format!(
                r#"
                tell application "Notes"
                    set matchedNotes to (every note whose name is "{escaped_name}")
                    if (count of matchedNotes) > 0 then
                        set theNote to item 1 of matchedNotes
                        set noteName to name of theNote
                        set noteBody to body of theNote
                        set noteFolder to name of container of theNote
                        set modDate to modification date of theNote
                        set creDate to creation date of theNote
                        return "NAME:" & noteName & linefeed & "FOLDER:" & noteFolder & linefeed & "CREATED:" & (creDate as text) & linefeed & "MODIFIED:" & (modDate as text) & linefeed & "BODY:" & linefeed & noteBody
                    else
                        return "ERROR:No note found with name: {escaped_name}"
                    end if
                end tell
                "#
            );

            match crate::macos::applescript::run_applescript(&script) {
                Ok(output) => {
                    if output.starts_with("ERROR:") {
                        Ok(error_result(output.trim_start_matches("ERROR:").to_string()))
                    } else {
                        Ok(text_result(output))
                    }
                }
                Err(e) => Ok(error_result(format!("Failed to read note: {e}"))),
            }
        })
    })
}

fn handler_write_note() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            let title = args
                .get("title")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("title is required"))?;

            let body = args
                .get("body")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("body is required"))?;

            let folder = args
                .get("folder")
                .and_then(|v| v.as_str())
                .unwrap_or("Notes");

            let escaped_title = title.replace('"', "\\\"");
            let escaped_body = body.replace('"', "\\\"");
            let escaped_folder = folder.replace('"', "\\\"");

            let script = format!(
                r#"
                tell application "Notes"
                    set targetFolder to missing value
                    repeat with a in accounts
                        repeat with f in folders of a
                            if name of f is "{escaped_folder}" then
                                set targetFolder to f
                                exit repeat
                            end if
                        end repeat
                        if targetFolder is not missing value then exit repeat
                    end repeat

                    if targetFolder is missing value then
                        return "ERROR:Folder not found: {escaped_folder}"
                    end if

                    make new note at targetFolder with properties {{name:"{escaped_title}", body:"{escaped_body}"}}
                    return "Note created: {escaped_title} in folder {escaped_folder}"
                end tell
                "#
            );

            match crate::macos::applescript::run_applescript(&script) {
                Ok(output) => {
                    if output.starts_with("ERROR:") {
                        Ok(error_result(output.trim_start_matches("ERROR:").to_string()))
                    } else {
                        Ok(text_result(output))
                    }
                }
                Err(e) => Ok(error_result(format!("Failed to create note: {e}"))),
            }
        })
    })
}

fn handler_delete_note() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            let name = args
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("name is required"))?;

            let escaped_name = name.replace('"', "\\\"");

            let script = format!(
                r#"
                tell application "Notes"
                    set matchedNotes to (every note whose name is "{escaped_name}")
                    if (count of matchedNotes) > 0 then
                        delete item 1 of matchedNotes
                        return "Deleted note: {escaped_name} (moved to Recently Deleted)"
                    else
                        return "ERROR:No note found with name: {escaped_name}"
                    end if
                end tell
                "#
            );

            match crate::macos::applescript::run_applescript(&script) {
                Ok(output) => {
                    if output.starts_with("ERROR:") {
                        Ok(error_result(output.trim_start_matches("ERROR:").to_string()))
                    } else {
                        Ok(text_result(output))
                    }
                }
                Err(e) => Ok(error_result(format!("Failed to delete note: {e}"))),
            }
        })
    })
}

fn handler_restore_note() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            let name = args
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("name is required"))?;

            let folder = args
                .get("folder")
                .and_then(|v| v.as_str())
                .unwrap_or("Notes");

            let escaped_name = name.replace('"', "\\\"");
            let escaped_folder = folder.replace('"', "\\\"");

            let script = format!(
                r#"
                tell application "Notes"
                    -- Find the target folder to restore to
                    set targetFolder to missing value
                    repeat with a in accounts
                        repeat with f in folders of a
                            if name of f is "{escaped_folder}" then
                                set targetFolder to f
                                exit repeat
                            end if
                        end repeat
                        if targetFolder is not missing value then exit repeat
                    end repeat

                    if targetFolder is missing value then
                        return "ERROR:Target folder not found: {escaped_folder}"
                    end if

                    -- Search Recently Deleted folders for the note
                    set noteFound to false
                    repeat with a in accounts
                        repeat with f in folders of a
                            if name of f is "Recently Deleted" then
                                repeat with n in notes of f
                                    if name of n is "{escaped_name}" then
                                        move n to targetFolder
                                        set noteFound to true
                                        exit repeat
                                    end if
                                end repeat
                            end if
                        end repeat
                        if noteFound then exit repeat
                    end repeat

                    if noteFound then
                        return "Restored note: {escaped_name} to folder {escaped_folder}"
                    else
                        return "ERROR:No note named '{escaped_name}' found in Recently Deleted"
                    end if
                end tell
                "#
            );

            match crate::macos::applescript::run_applescript(&script) {
                Ok(output) => {
                    if output.starts_with("ERROR:") {
                        Ok(error_result(output.trim_start_matches("ERROR:").to_string()))
                    } else {
                        Ok(text_result(output))
                    }
                }
                Err(e) => Ok(error_result(format!("Failed to restore note: {e}"))),
            }
        })
    })
}

fn handler_open_note() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            let name = args
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("name is required"))?;

            let escaped_name = name.replace('"', "\\\"");

            let script = format!(
                r#"
                tell application "Notes"
                    set matchedNotes to (every note whose name is "{escaped_name}")
                    if (count of matchedNotes) > 0 then
                        set theNote to item 1 of matchedNotes
                        show theNote
                        activate
                        return "Opened note: {escaped_name}"
                    else
                        return "ERROR:No note found with name: {escaped_name}"
                    end if
                end tell
                "#
            );

            match crate::macos::applescript::run_applescript(&script) {
                Ok(output) => {
                    if output.starts_with("ERROR:") {
                        Ok(error_result(output.trim_start_matches("ERROR:").to_string()))
                    } else {
                        Ok(text_result(output))
                    }
                }
                Err(e) => Ok(error_result(format!("Failed to open note: {e}"))),
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
        assert_eq!(tools.len(), 8, "Expected exactly 8 notes tools");

        let names: Vec<_> = tools.iter().map(|t| t.name.as_ref()).collect();
        assert!(names.contains(&"notes_list_accounts"));
        assert!(names.contains(&"notes_list_folders"));
        assert!(names.contains(&"notes_search_notes"));
        assert!(names.contains(&"notes_read_note"));
        assert!(names.contains(&"notes_write_note"));
        assert!(names.contains(&"notes_delete_note"));
        assert!(names.contains(&"notes_restore_note"));
        assert!(names.contains(&"notes_open_note"));
    }
}
