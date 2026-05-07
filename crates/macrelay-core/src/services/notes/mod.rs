use std::sync::Arc;

use rmcp::model::Tool;
use serde_json::json;

use crate::macos::escape::escape_applescript_string;
use crate::registry::{ServiceRegistry, ToolHandler, error_result, schema_from_json, text_result};

/// Register all Notes tools with the service registry.
pub fn register(registry: &mut ServiceRegistry) {
    registry.register(
        "productivity_notes_list_accounts",
        Tool::new(
            "productivity_notes_list_accounts",
            "[READ] List all Notes accounts (e.g. iCloud, On My Mac, Gmail).",
            schema_from_json(json!({
                "type": "object",
                "properties": {},
            })),
        ),
        handler_list_accounts(),
    );

    registry.register(
        "productivity_notes_list_folders",
        Tool::new(
            "productivity_notes_list_folders",
            "[READ] List all folders across all Notes accounts.",
            schema_from_json(json!({
                "type": "object",
                "properties": {},
            })),
        ),
        handler_list_folders(),
    );

    registry.register(
        "productivity_notes_search_notes",
        Tool::new(
            "productivity_notes_search_notes",
            "[READ] Search notes by text query. Returns matching note names with their folder and account.",
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
        "productivity_notes_read_note",
        Tool::new(
            "productivity_notes_read_note",
            "[READ] Read the full content of a note by its name. Returns the HTML body of the note.",
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
        "productivity_notes_write_note",
        Tool::new(
            "productivity_notes_write_note",
            "[CREATE] Create a new note with a title and HTML body. Optionally specify a folder and account.",
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
                        "description": "Folder to create the note in. Defaults to 'Notes'."
                    },
                    "account": {
                        "type": "string",
                        "description": "Account to create the note in (e.g. 'iCloud', 'On My Mac'). If omitted, prefers iCloud. Use notes_list_accounts to see available accounts."
                    }
                },
                "required": ["title", "body"]
            })),
        ),
        handler_write_note(),
    );

    registry.register(
        "productivity_notes_update_note",
        Tool::new(
            "productivity_notes_update_note",
            "[UPDATE] Update an existing note's body and/or name. Finds the note by iterating all accounts and folders.",
            schema_from_json(json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "The current name (title) of the note to update."
                    },
                    "body": {
                        "type": "string",
                        "description": "New HTML body content for the note."
                    },
                    "new_name": {
                        "type": "string",
                        "description": "New name/title for the note."
                    }
                },
                "required": ["name"]
            })),
        ),
        handler_update_note(),
    );

    registry.register(
        "productivity_notes_delete_note",
        Tool::new(
            "productivity_notes_delete_note",
            "[DELETE] Delete a note by name. The note is moved to Recently Deleted.",
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
        "productivity_notes_restore_note",
        Tool::new(
            "productivity_notes_restore_note",
            "[UPDATE] Restore a note from Recently Deleted by moving it back to a target folder and account.",
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
                    },
                    "account": {
                        "type": "string",
                        "description": "Account to restore the note to (e.g. 'iCloud', 'On My Mac'). If omitted, prefers iCloud. Use notes_list_accounts to see available accounts."
                    }
                },
                "required": ["name"]
            })),
        ),
        handler_restore_note(),
    );

    registry.register(
        "productivity_notes_open_note",
        Tool::new(
            "productivity_notes_open_note",
            "[READ] Open Notes.app and display a specific note by name.",
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
            let query = match args.get("query").and_then(|v| v.as_str()) {
                Some(q) => q,
                None => return Ok(error_result("query is required")),
            };

            let folder_filter = args.get("folder").and_then(|v| v.as_str()).unwrap_or("");
            let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(50);
            let offset = args.get("offset").and_then(|v| v.as_u64()).unwrap_or(0);

            let escaped_query = escape_applescript_string(query);

            // Build the folder iteration clause
            let folder_clause = if folder_filter.is_empty() {
                "repeat with f in folders of a".to_string()
            } else {
                let escaped_folder = escape_applescript_string(folder_filter);
                format!(r#"repeat with f in {{folder "{escaped_folder}" of a}}"#)
            };

            // Use Notes' built-in `whose name contains` filter rather than
            // iterating every note manually. On a 1500-note library, the
            // manual scan takes ~55s (over the default 30s timeout); the
            // `whose` form returns the same results in ~8s. We still iterate
            // accounts/folders so we can attach folder + account names to
            // each result, but the per-note property loop is gone.
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
                            set hits to (notes of f whose name contains searchQuery)
                            repeat with n in hits
                                if skipped < skipCount then
                                    set skipped to skipped + 1
                                else if matchCount < maxResults then
                                    set modDate to modification date of n
                                    set output to output & (name of n) & "||" & folderName & "||" & acctName & "||" & (modDate as text) & linefeed
                                    set matchCount to matchCount + 1
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

            // Use EXTENDED_TIMEOUT (60s) as a safety margin: the `whose`
            // filter is fast, but very large libraries (10k+ notes) may
            // still need more than 30s on a cold Notes process.
            match crate::macos::applescript::run_applescript_with_timeout(
                &script,
                crate::macos::applescript::EXTENDED_TIMEOUT,
            ) {
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
            let name = match args.get("name").and_then(|v| v.as_str()) {
                Some(n) => n,
                None => return Ok(error_result("name is required")),
            };

            let escaped_name = escape_applescript_string(name);

            let script = format!(
                r#"
                tell application "Notes"
                    repeat with a in accounts
                        set acctName to name of a
                        repeat with f in folders of a
                            set folderName to name of f
                            set matches to (every note of f whose name is "{escaped_name}")
                            if (count of matches) > 0 then
                                set theNote to item 1 of matches
                                set noteBody to body of theNote
                                set modDate to modification date of theNote
                                set creDate to creation date of theNote
                                return "NAME:" & (name of theNote) & "||FOLDER:" & folderName & "||ACCOUNT:" & acctName & "||MODIFIED:" & (modDate as text) & "||CREATED:" & (creDate as text) & "||BODY:" & noteBody
                            end if
                        end repeat
                    end repeat
                    return "ERROR:No note found with name: {escaped_name}"
                end tell
                "#
            );

            match crate::macos::applescript::run_applescript(&script) {
                Ok(output) => {
                    if output.starts_with("ERROR:") {
                        Ok(error_result(
                            output.trim_start_matches("ERROR:").to_string(),
                        ))
                    } else {
                        // Parse the ||-delimited output into structured JSON.
                        // Use splitn(7, "||") because BODY is last and may contain "||".
                        let parts: Vec<&str> = output.splitn(7, "||").collect();
                        let get_field = |prefix: &str, part: Option<&&str>| -> String {
                            part.map(|p| p.trim_start_matches(prefix).to_string())
                                .unwrap_or_default()
                        };
                        let note = json!({
                            "name": get_field("NAME:", parts.first()),
                            "folder": get_field("FOLDER:", parts.get(1)),
                            "account": get_field("ACCOUNT:", parts.get(2)),
                            "modified": get_field("MODIFIED:", parts.get(3)),
                            "created": get_field("CREATED:", parts.get(4)),
                            "body": parts.get(5).map(|p| p.trim_start_matches("BODY:")).unwrap_or(""),
                        });
                        let json = serde_json::to_string_pretty(&note)?;
                        Ok(text_result(json))
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
            let title = match args.get("title").and_then(|v| v.as_str()) {
                Some(t) => t,
                None => return Ok(error_result("title is required")),
            };
            let body = match args.get("body").and_then(|v| v.as_str()) {
                Some(b) => b,
                None => return Ok(error_result("body is required")),
            };

            let folder = args
                .get("folder")
                .and_then(|v| v.as_str())
                .unwrap_or("Notes");

            let account = args.get("account").and_then(|v| v.as_str());

            let escaped_title = escape_applescript_string(title);
            let escaped_body = escape_applescript_string(body);
            let escaped_folder = escape_applescript_string(folder);

            let script = if let Some(acct) = account {
                // Account explicitly specified - scope to that account only
                let escaped_account = escape_applescript_string(acct);
                format!(
                    r#"
                    tell application "Notes"
                        set targetFolder to missing value
                        try
                            set targetAcct to account "{escaped_account}"
                            repeat with f in folders of targetAcct
                                if name of f is "{escaped_folder}" then
                                    set targetFolder to f
                                    exit repeat
                                end if
                            end repeat
                        on error
                            return "ERROR:Account not found: {escaped_account}. Use notes_list_accounts to see available accounts."
                        end try

                        if targetFolder is missing value then
                            return "ERROR:Folder '{escaped_folder}' not found in account '{escaped_account}'. Use notes_list_folders to see available folders."
                        end if

                        make new note at targetFolder with properties {{name:"{escaped_title}", body:"{escaped_body}"}}
                        return "Note created: {escaped_title} in folder {escaped_folder} (account: {escaped_account})"
                    end tell
                    "#
                )
            } else {
                // No account specified - find all accounts with this folder, prefer iCloud
                format!(
                    r#"
                    tell application "Notes"
                        set matchingAccounts to {{}}
                        set targetFolder to missing value
                        set iCloudFolder to missing value

                        repeat with a in accounts
                            set acctName to name of a
                            repeat with f in folders of a
                                if name of f is "{escaped_folder}" then
                                    set end of matchingAccounts to acctName
                                    if targetFolder is missing value then
                                        set targetFolder to f
                                        set targetAcctName to acctName
                                    end if
                                    if acctName contains "iCloud" then
                                        set iCloudFolder to f
                                        set iCloudAcctName to acctName
                                    end if
                                    exit repeat
                                end if
                            end repeat
                        end repeat

                        if targetFolder is missing value then
                            return "ERROR:Folder not found: {escaped_folder}. Use notes_list_folders to see available folders."
                        end if

                        -- Prefer iCloud if available
                        if iCloudFolder is not missing value then
                            set targetFolder to iCloudFolder
                            set targetAcctName to iCloudAcctName
                        end if

                        make new note at targetFolder with properties {{name:"{escaped_title}", body:"{escaped_body}"}}

                        set resultMsg to "Note created: {escaped_title} in folder {escaped_folder} (account: " & targetAcctName & ")"
                        if (count of matchingAccounts) > 1 then
                            set acctList to ""
                            repeat with i from 1 to count of matchingAccounts
                                if i > 1 then set acctList to acctList & ", "
                                set acctList to acctList & item i of matchingAccounts
                            end repeat
                            set resultMsg to resultMsg & ". Note: folder '{escaped_folder}' exists in multiple accounts: " & acctList & ". Use the account parameter to target a specific one."
                        end if
                        return resultMsg
                    end tell
                    "#
                )
            };

            match crate::macos::applescript::run_applescript(&script) {
                Ok(output) => {
                    if output.starts_with("ERROR:") {
                        Ok(error_result(
                            output.trim_start_matches("ERROR:").to_string(),
                        ))
                    } else {
                        Ok(text_result(output))
                    }
                }
                Err(e) => Ok(error_result(format!("Failed to create note: {e}"))),
            }
        })
    })
}

fn handler_update_note() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            let name = match args.get("name").and_then(|v| v.as_str()) {
                Some(n) => n,
                None => return Ok(error_result("name is required")),
            };

            let new_body = args.get("body").and_then(|v| v.as_str());
            let new_name = args.get("new_name").and_then(|v| v.as_str());

            if new_body.is_none() && new_name.is_none() {
                return Ok(error_result(
                    "At least one of 'body' or 'new_name' must be provided",
                ));
            }

            let escaped_name = escape_applescript_string(name);

            // Build the update commands conditionally
            let mut update_commands = String::new();
            let mut updated_parts: Vec<&str> = Vec::new();

            if let Some(body) = new_body {
                let escaped_body = escape_applescript_string(body);
                update_commands.push_str(&format!("set body of n to \"{escaped_body}\"\n"));
                updated_parts.push("body");
            }
            if let Some(rname) = new_name {
                let escaped_new_name = escape_applescript_string(rname);
                update_commands.push_str(&format!("set name of n to \"{escaped_new_name}\"\n"));
                updated_parts.push("name");
            }

            let updated_desc = updated_parts.join(" and ");

            // Iterate accounts > folders, then use `whose name is` *scoped to
            // each folder* to find the target note. This keeps `folderName`
            // and `acctName` available for the result message (the flat
            // `every note whose name is` form loses parent context — see
            // `delete_note` which doesn't need that context, vs. this one
            // which does). Folder-scoped `whose` is O(1) lookup per folder
            // instead of O(N) manual property reads, so update_note stops
            // scaling with library size.
            let script = format!(
                r#"
                tell application "Notes"
                    set noteFound to false
                    set resultMsg to ""
                    repeat with a in accounts
                        set acctName to name of a
                        repeat with f in folders of a
                            set folderName to name of f
                            set matches to (every note of f whose name is "{escaped_name}")
                            if (count of matches) > 0 then
                                set n to item 1 of matches
                                {update_commands}
                                set noteFound to true
                                set resultMsg to "Updated {updated_desc} of note: {escaped_name} (folder: " & folderName & ", account: " & acctName & ")"
                                exit repeat
                            end if
                        end repeat
                        if noteFound then exit repeat
                    end repeat

                    if noteFound then
                        return resultMsg
                    else
                        return "ERROR:No note found with name: {escaped_name}"
                    end if
                end tell
                "#
            );

            match crate::macos::applescript::run_applescript(&script) {
                Ok(output) => {
                    if output.starts_with("ERROR:") {
                        Ok(error_result(
                            output.trim_start_matches("ERROR:").to_string(),
                        ))
                    } else {
                        Ok(text_result(output))
                    }
                }
                Err(e) => Ok(error_result(format!("Failed to update note: {e}"))),
            }
        })
    })
}

fn handler_delete_note() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            let name = match args.get("name").and_then(|v| v.as_str()) {
                Some(n) => n,
                None => return Ok(error_result("name is required")),
            };

            let escaped_name = escape_applescript_string(name);

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
                        Ok(error_result(
                            output.trim_start_matches("ERROR:").to_string(),
                        ))
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
            let name = match args.get("name").and_then(|v| v.as_str()) {
                Some(n) => n,
                None => return Ok(error_result("name is required")),
            };

            let folder = args
                .get("folder")
                .and_then(|v| v.as_str())
                .unwrap_or("Notes");

            let account = args.get("account").and_then(|v| v.as_str());

            let escaped_name = escape_applescript_string(name);
            let escaped_folder = escape_applescript_string(folder);

            let script = if let Some(acct) = account {
                // Account explicitly specified. Avoid all reference-chain iteration:
                // direct by-name lookups + `whose` filters keep references concrete.
                let escaped_account = escape_applescript_string(acct);
                format!(
                    r#"
                    tell application "Notes"
                        try
                            set targetAcct to account "{escaped_account}"
                        on error
                            return "ERROR:Account not found: {escaped_account}. Use notes_list_accounts to see available accounts."
                        end try

                        try
                            set targetFolderRef to folder "{escaped_folder}" of targetAcct
                            get name of targetFolderRef
                        on error
                            return "ERROR:Folder '{escaped_folder}' not found in account '{escaped_account}'. Use notes_list_folders to see available folders."
                        end try

                        -- Scan each account's Recently Deleted via by-name lookup + whose filter
                        set noteFound to false
                        repeat with a in accounts
                            set acctName to name of a
                            try
                                set rdFolder to folder "Recently Deleted" of a
                                set matches to (every note of rdFolder whose name is "{escaped_name}")
                                if (count of matches) > 0 then
                                    move (item 1 of matches) to folder "{escaped_folder}" of account "{escaped_account}"
                                    set noteFound to true
                                    exit repeat
                                end if
                            end try
                        end repeat

                        if noteFound then
                            return "Restored note: {escaped_name} to folder {escaped_folder} (account: {escaped_account})"
                        else
                            return "ERROR:No note named '{escaped_name}' found in Recently Deleted"
                        end if
                    end tell
                    "#
                )
            } else {
                // No account specified - prefer iCloud, report alternatives.
                // Track the target account by NAME string only; avoid iterating
                // folder/note reference chains.
                format!(
                    r#"
                    tell application "Notes"
                        set matchingAccounts to {{}}
                        set targetAcctName to missing value
                        set iCloudAcctName to missing value

                        -- Phase 1: discover which accounts contain the target folder (by name)
                        repeat with a in accounts
                            set acctName to name of a
                            try
                                set probeFolder to folder "{escaped_folder}" of a
                                get name of probeFolder
                                set end of matchingAccounts to acctName
                                if targetAcctName is missing value then
                                    set targetAcctName to acctName
                                end if
                                if acctName contains "iCloud" then
                                    set iCloudAcctName to acctName
                                end if
                            end try
                        end repeat

                        if targetAcctName is missing value then
                            return "ERROR:Target folder not found: {escaped_folder}. Use notes_list_folders to see available folders."
                        end if

                        if iCloudAcctName is not missing value then
                            set targetAcctName to iCloudAcctName
                        end if

                        -- Phase 2: scan each account's Recently Deleted via by-name lookup + whose filter
                        set noteFound to false
                        repeat with a in accounts
                            try
                                set rdFolder to folder "Recently Deleted" of a
                                set matches to (every note of rdFolder whose name is "{escaped_name}")
                                if (count of matches) > 0 then
                                    move (item 1 of matches) to folder "{escaped_folder}" of (first account whose name is targetAcctName)
                                    set noteFound to true
                                    exit repeat
                                end if
                            end try
                        end repeat

                        if noteFound then
                            set resultMsg to "Restored note: {escaped_name} to folder {escaped_folder} (account: " & targetAcctName & ")"
                            if (count of matchingAccounts) > 1 then
                                set acctList to ""
                                repeat with i from 1 to count of matchingAccounts
                                    if i > 1 then set acctList to acctList & ", "
                                    set acctList to acctList & item i of matchingAccounts
                                end repeat
                                set resultMsg to resultMsg & ". Note: folder '{escaped_folder}' exists in multiple accounts: " & acctList & ". Use the account parameter to target a specific one."
                            end if
                            return resultMsg
                        else
                            return "ERROR:No note named '{escaped_name}' found in Recently Deleted"
                        end if
                    end tell
                    "#
                )
            };

            match crate::macos::applescript::run_applescript(&script) {
                Ok(output) => {
                    if output.starts_with("ERROR:") {
                        Ok(error_result(
                            output.trim_start_matches("ERROR:").to_string(),
                        ))
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
            let name = match args.get("name").and_then(|v| v.as_str()) {
                Some(n) => n,
                None => return Ok(error_result("name is required")),
            };

            let escaped_name = escape_applescript_string(name);

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
                        Ok(error_result(
                            output.trim_start_matches("ERROR:").to_string(),
                        ))
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
    use crate::macos::applescript::{MOCK_RUNNER, ScriptRunner};
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::time::Duration;

    struct AssertingMock {
        expected_fragment: String,
        response: String,
    }

    impl ScriptRunner for AssertingMock {
        fn run_applescript(&self, script: &str) -> anyhow::Result<String> {
            assert!(
                script.contains(&self.expected_fragment),
                "script missing fragment {:?}: {}",
                self.expected_fragment,
                script
            );
            Ok(self.response.clone())
        }
        fn run_applescript_with_timeout(
            &self,
            script: &str,
            _timeout: Duration,
        ) -> anyhow::Result<String> {
            // search_notes calls the timeout variant directly so it can opt
            // into EXTENDED_TIMEOUT; route it through the same assert path.
            self.run_applescript(script)
        }
        fn run_jxa(&self, _script: &str) -> anyhow::Result<String> {
            unimplemented!()
        }
    }

    #[test]
    fn test_tool_schemas_valid() {
        let mut registry = ServiceRegistry::new();
        register(&mut registry);
        let tools = registry.list_tools();
        assert_eq!(tools.len(), 9, "Expected exactly 9 notes tools");

        let names: Vec<_> = tools.iter().map(|t| t.name.as_ref()).collect();
        assert!(names.contains(&"productivity_notes_list_accounts"));
        assert!(names.contains(&"productivity_notes_list_folders"));
        assert!(names.contains(&"productivity_notes_search_notes"));
        assert!(names.contains(&"productivity_notes_read_note"));
        assert!(names.contains(&"productivity_notes_write_note"));
        assert!(names.contains(&"productivity_notes_update_note"));
        assert!(names.contains(&"productivity_notes_delete_note"));
        assert!(names.contains(&"productivity_notes_restore_note"));
        assert!(names.contains(&"productivity_notes_open_note"));
    }

    #[test]
    fn test_write_and_restore_have_account_param() {
        let mut registry = ServiceRegistry::new();
        register(&mut registry);
        let tools = registry.list_tools();

        for tool_name in &[
            "productivity_notes_write_note",
            "productivity_notes_restore_note",
        ] {
            let tool = tools
                .iter()
                .find(|t| t.name.as_ref() == *tool_name)
                .unwrap();
            let props = tool
                .input_schema
                .get("properties")
                .and_then(|v| v.as_object());
            assert!(
                props.is_some_and(|p| p.contains_key("account")),
                "{tool_name} should have an 'account' parameter"
            );
        }
    }

    #[tokio::test]
    async fn test_mock_list_accounts() {
        let mock = Arc::new(AssertingMock {
            expected_fragment: "repeat with a in accounts".to_string(),
            response: "iCloud\nOn My Mac\n".to_string(),
        });

        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_list_accounts();
                let result = handler(HashMap::new()).await.unwrap();
                assert_eq!(result.is_error, Some(false));

                let content = result.content[0].as_text().unwrap().text.as_str();
                assert!(content.contains("Found 2 account(s)"));
                assert!(content.contains("iCloud"));
                assert!(content.contains("On My Mac"));
            })
            .await;
    }

    #[tokio::test]
    async fn test_mock_list_folders() {
        let mock = Arc::new(AssertingMock {
            expected_fragment: "repeat with f in folders of a".to_string(),
            response: "Notes||iCloud\nReceipts||iCloud\n".to_string(),
        });

        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_list_folders();
                let result = handler(HashMap::new()).await.unwrap();
                assert_eq!(result.is_error, Some(false));

                let content = result.content[0].as_text().unwrap().text.as_str();
                assert!(content.contains("Found 2 folder(s)"));
                assert!(content.contains("\"folder\": \"Notes\""));
                assert!(content.contains("\"account\": \"iCloud\""));
            })
            .await;
    }

    #[tokio::test]
    async fn test_mock_search_notes() {
        // Regression guard: the search MUST use the `whose name contains`
        // server-side filter, not a manual `repeat ... if name of n contains`
        // scan. The latter takes ~55s on a 1500-note library and trips the
        // 30s default timeout.
        let mock = Arc::new(AssertingMock {
            expected_fragment: "whose name contains searchQuery".to_string(),
            response: "My Note||Notes||iCloud||Monday, January 1, 2024 at 12:00:00 PM\n"
                .to_string(),
        });

        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_search_notes();
                let mut args = HashMap::new();
                args.insert("query".to_string(), json!("My"));

                let result = handler(args).await.unwrap();
                assert_eq!(result.is_error, Some(false));

                let content = result.content[0].as_text().unwrap().text.as_str();
                assert!(content.contains("Found 1 note(s)"));
                assert!(content.contains("\"name\": \"My Note\""));
                assert!(
                    content.contains("\"modified\": \"Monday, January 1, 2024 at 12:00:00 PM\"")
                );
            })
            .await;
    }

    #[tokio::test]
    async fn test_mock_read_note() {
        let mock = Arc::new(AssertingMock {
            expected_fragment: "every note of f whose name is \"My Note\"".to_string(),
            response:
                "NAME:My Note||FOLDER:Notes||ACCOUNT:iCloud||MODIFIED:Jan 1||CREATED:Jan 1||BODY:Hello"
                    .to_string(),
        });

        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_read_note();
                let mut args = HashMap::new();
                args.insert("name".to_string(), json!("My Note"));

                let result = handler(args).await.unwrap();
                assert_eq!(result.is_error, Some(false));

                let content = result.content[0].as_text().unwrap().text.as_str();
                assert!(content.contains("\"name\": \"My Note\""));
                assert!(content.contains("\"folder\": \"Notes\""));
                assert!(content.contains("\"account\": \"iCloud\""));
                assert!(content.contains("\"body\": \"Hello\""));
            })
            .await;
    }

    #[tokio::test]
    async fn test_mock_write_note() {
        let mock = Arc::new(AssertingMock {
            expected_fragment:
                "make new note at targetFolder with properties {name:\"New Note\", body:\"Hello\"}"
                    .to_string(),
            response: "Note created: New Note in folder Notes (account: iCloud)".to_string(),
        });

        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_write_note();
                let mut args = HashMap::new();
                args.insert("title".to_string(), json!("New Note"));
                args.insert("body".to_string(), json!("Hello"));

                let result = handler(args).await.unwrap();
                assert_eq!(result.is_error, Some(false));

                let content = result.content[0].as_text().unwrap().text.as_str();
                assert!(content.contains("Note created: New Note"));
            })
            .await;
    }

    #[tokio::test]
    async fn test_mock_write_note_escaping() {
        let mock = Arc::new(AssertingMock {
            expected_fragment: "name:\"Note with \\\"quotes\\\" and \\\\backslash\"".to_string(),
            response: "Note created: Note with \"quotes\" and \\backslash".to_string(),
        });

        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_write_note();
                let mut args = HashMap::new();
                args.insert(
                    "title".to_string(),
                    json!("Note with \"quotes\" and \\backslash"),
                );
                args.insert("body".to_string(), json!("Hello"));

                let result = handler(args).await.unwrap();
                assert_eq!(result.is_error, Some(false));
            })
            .await;
    }

    #[tokio::test]
    async fn test_mock_delete_note() {
        let mock = Arc::new(AssertingMock {
            expected_fragment: "delete item 1 of matchedNotes".to_string(),
            response: "Deleted note: My Note (moved to Recently Deleted)".to_string(),
        });

        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_delete_note();
                let mut args = HashMap::new();
                args.insert("name".to_string(), json!("My Note"));

                let result = handler(args).await.unwrap();
                assert_eq!(result.is_error, Some(false));

                let content = result.content[0].as_text().unwrap().text.as_str();
                assert!(content.contains("Deleted note: My Note"));
            })
            .await;
    }

    #[tokio::test]
    async fn test_mock_restore_note() {
        let mock = Arc::new(AssertingMock {
            expected_fragment: "move (item 1 of matches) to folder \"Notes\"".to_string(),
            response: "Restored note: My Note to folder Notes (account: iCloud)".to_string(),
        });

        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_restore_note();
                let mut args = HashMap::new();
                args.insert("name".to_string(), json!("My Note"));

                let result = handler(args).await.unwrap();
                assert_eq!(result.is_error, Some(false));

                let content = result.content[0].as_text().unwrap().text.as_str();
                assert!(content.contains("Restored note: My Note"));
            })
            .await;
    }

    #[tokio::test]
    async fn test_mock_open_note() {
        let mock = Arc::new(AssertingMock {
            expected_fragment: "show theNote".to_string(),
            response: "Opened note: My Note".to_string(),
        });

        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_open_note();
                let mut args = HashMap::new();
                args.insert("name".to_string(), json!("My Note"));

                let result = handler(args).await.unwrap();
                assert_eq!(result.is_error, Some(false));

                let content = result.content[0].as_text().unwrap().text.as_str();
                assert!(content.contains("Opened note: My Note"));
            })
            .await;
    }

    /// When osascript fails, write_note must return a graceful error result
    /// instead of panicking or propagating a raw anyhow error.
    #[tokio::test]
    async fn test_write_note_returns_error_result_on_osascript_failure() {
        struct ErrorMock;
        impl ScriptRunner for ErrorMock {
            fn run_applescript(&self, _script: &str) -> anyhow::Result<String> {
                Err(anyhow::anyhow!(
                    "osascript: Notes got an error: Not authorized to send Apple events"
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
                let handler = handler_write_note();
                let mut args = HashMap::new();
                args.insert("title".to_string(), json!("Test"));
                args.insert("body".to_string(), json!("Body"));

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
                    content.contains("Not authorized"),
                    "Expected underlying error to be surfaced, got: {}",
                    content
                );
            })
            .await;
    }

    #[tokio::test]
    async fn test_validation_write_note_requires_title() {
        let handler = handler_write_note();
        let mut args = HashMap::new();
        args.insert("body".to_string(), json!("Body"));

        let result = handler(args).await.expect("Handler should not panic");
        assert_eq!(result.is_error, Some(true));
        assert!(
            result.content[0]
                .as_text()
                .unwrap()
                .text
                .contains("title is required")
        );
    }

    #[tokio::test]
    async fn test_mock_update_note() {
        // Regression guard: like search_notes, update_note must use
        // folder-scoped `whose name is` rather than a manual property loop.
        // We assert on the lookup form, not just the body assignment.
        let mock = Arc::new(AssertingMock {
            expected_fragment: "every note of f whose name is \"My Note\"".to_string(),
            response: "Updated body of note: My Note (folder: Notes, account: iCloud)".to_string(),
        });

        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_update_note();
                let mut args = HashMap::new();
                args.insert("name".to_string(), json!("My Note"));
                args.insert("body".to_string(), json!("<p>Updated content</p>"));

                let result = handler(args).await.unwrap();
                assert_eq!(result.is_error, Some(false));

                let content = result.content[0].as_text().unwrap().text.as_str();
                assert!(content.contains("Updated"));
            })
            .await;
    }

    #[tokio::test]
    async fn test_validation_update_note_requires_name() {
        let handler = handler_update_note();
        let mut args = HashMap::new();
        args.insert("body".to_string(), json!("New body"));

        let result = handler(args).await.expect("Handler should not panic");
        assert_eq!(result.is_error, Some(true));
        assert!(
            result.content[0]
                .as_text()
                .unwrap()
                .text
                .contains("name is required")
        );
    }

    #[tokio::test]
    async fn test_validation_update_note_requires_change() {
        let handler = handler_update_note();
        let mut args = HashMap::new();
        args.insert("name".to_string(), json!("My Note"));
        // No body or new_name — nothing to update

        let result = handler(args).await.expect("Handler should not panic");
        assert_eq!(result.is_error, Some(true));
        assert!(result.content[0].as_text().unwrap().text.contains("body"));
    }

    #[tokio::test]
    async fn test_validation_read_note_requires_name() {
        let handler = handler_read_note();
        let args = HashMap::new();

        let result = handler(args).await.expect("Handler should not panic");
        assert_eq!(result.is_error, Some(true));
        assert!(
            result.content[0]
                .as_text()
                .unwrap()
                .text
                .contains("name is required")
        );
    }
}
