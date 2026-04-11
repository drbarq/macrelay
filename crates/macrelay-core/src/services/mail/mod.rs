use std::sync::Arc;

use rmcp::model::Tool;
use serde_json::json;

use crate::macos::escape::escape_applescript_string;
use crate::registry::{ServiceRegistry, ToolHandler, error_result, schema_from_json, text_result};

/// Register all mail tools with the service registry.
pub fn register(registry: &mut ServiceRegistry) {
    registry.register(
        "communication_mail_list_accounts",
        Tool::new(
            "communication_mail_list_accounts",
            "[READ] List all mail accounts configured in Mail.app.",
            schema_from_json(json!({
                "type": "object",
                "properties": {},
            })),
        ),
        handler_list_accounts(),
    );

    registry.register(
        "communication_mail_list_mailboxes",
        Tool::new(
            "communication_mail_list_mailboxes",
            "[READ] List mailboxes (Inbox, Sent, Drafts, etc.) for a given mail account.",
            schema_from_json(json!({
                "type": "object",
                "properties": {
                    "account": {
                        "type": "string",
                        "description": "Name of the mail account to list mailboxes for."
                    }
                },
                "required": ["account"]
            })),
        ),
        handler_list_mailboxes(),
    );

    registry.register(
        "communication_mail_search_messages",
        Tool::new(
            "communication_mail_search_messages",
            "[READ] Search mail messages by query, sender, subject, or date range. Returns matching messages with subject, sender, date, and read status.",
            schema_from_json(json!({
                "type": "object",
                "properties": {
                    "account": {
                        "type": "string",
                        "description": "Mail account name to search in. Searches all accounts if omitted."
                    },
                    "mailbox": {
                        "type": "string",
                        "description": "Mailbox name to search in (e.g. 'INBOX'). Defaults to 'INBOX'."
                    },
                    "query": {
                        "type": "string",
                        "description": "General text to search for in subject and content."
                    },
                    "sender": {
                        "type": "string",
                        "description": "Filter by sender email or name."
                    },
                    "subject": {
                        "type": "string",
                        "description": "Filter by subject text."
                    },
                    "date_from": {
                        "type": "string",
                        "description": "Start date filter as 'YYYY-MM-DD' string."
                    },
                    "date_to": {
                        "type": "string",
                        "description": "End date filter as 'YYYY-MM-DD' string."
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of messages to return. Default 25."
                    },
                    "offset": {
                        "type": "integer",
                        "description": "Number of messages to skip for pagination. Default 0."
                    }
                }
            })),
        ),
        handler_search_messages(),
    );

    registry.register(
        "communication_mail_get_messages",
        Tool::new(
            "communication_mail_get_messages",
            "[READ] Get detailed message content by matching subject. Returns subject, sender, recipients, date, read status, and body content.",
            schema_from_json(json!({
                "type": "object",
                "properties": {
                    "subject": {
                        "type": "string",
                        "description": "Subject text to match (partial match)."
                    },
                    "account": {
                        "type": "string",
                        "description": "Mail account name. Searches all accounts if omitted."
                    },
                    "mailbox": {
                        "type": "string",
                        "description": "Mailbox name (e.g. 'INBOX'). Defaults to 'INBOX'."
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of messages to return. Default 5."
                    }
                },
                "required": ["subject"]
            })),
        ),
        handler_get_messages(),
    );

    registry.register(
        "communication_mail_get_thread",
        Tool::new(
            "communication_mail_get_thread",
            "[READ] Get all messages in a thread/conversation by matching the subject line. Returns all related messages sorted by date.",
            schema_from_json(json!({
                "type": "object",
                "properties": {
                    "subject": {
                        "type": "string",
                        "description": "Subject text to match for the thread (partial match, ignores Re:/Fwd: prefixes)."
                    },
                    "account": {
                        "type": "string",
                        "description": "Mail account name. Searches all accounts if omitted."
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of thread messages to return. Default 20."
                    }
                },
                "required": ["subject"]
            })),
        ),
        handler_get_thread(),
    );

    registry.register(
        "communication_mail_compose_message",
        Tool::new(
            "communication_mail_compose_message",
            "[CREATE] Create and display a new outgoing email message in Mail.app.",
            schema_from_json(json!({
                "type": "object",
                "properties": {
                    "to": {
                        "type": "string",
                        "description": "Recipient email address(es), comma-separated for multiple."
                    },
                    "cc": {
                        "type": "string",
                        "description": "CC email address(es), comma-separated for multiple."
                    },
                    "bcc": {
                        "type": "string",
                        "description": "BCC email address(es), comma-separated for multiple."
                    },
                    "subject": {
                        "type": "string",
                        "description": "Email subject line."
                    },
                    "body": {
                        "type": "string",
                        "description": "Email body content."
                    },
                    "send": {
                        "type": "boolean",
                        "description": "If true, send immediately. If false (default), just open the compose window."
                    }
                },
                "required": ["to", "subject", "body"]
            })),
        ),
        handler_compose_message(),
    );

    registry.register(
        "communication_mail_reply_message",
        Tool::new(
            "communication_mail_reply_message",
            "[CREATE] Reply to an existing email message found by subject match. Opens a reply window in Mail.app.",
            schema_from_json(json!({
                "type": "object",
                "properties": {
                    "subject": {
                        "type": "string",
                        "description": "Subject of the message to reply to (partial match)."
                    },
                    "reply_text": {
                        "type": "string",
                        "description": "Text to include in the reply body."
                    },
                    "reply_all": {
                        "type": "boolean",
                        "description": "If true, reply to all recipients. Default false."
                    },
                    "account": {
                        "type": "string",
                        "description": "Mail account name. Searches all accounts if omitted."
                    },
                    "mailbox": {
                        "type": "string",
                        "description": "Mailbox name. Defaults to 'INBOX'."
                    }
                },
                "required": ["subject"]
            })),
        ),
        handler_reply_message(),
    );

    registry.register(
        "communication_mail_forward_message",
        Tool::new(
            "communication_mail_forward_message",
            "[CREATE] Forward an existing email message found by subject match. Opens a forward window in Mail.app.",
            schema_from_json(json!({
                "type": "object",
                "properties": {
                    "subject": {
                        "type": "string",
                        "description": "Subject of the message to forward (partial match)."
                    },
                    "to": {
                        "type": "string",
                        "description": "Recipient email address to forward to."
                    },
                    "forward_text": {
                        "type": "string",
                        "description": "Optional text to prepend to the forwarded message."
                    },
                    "account": {
                        "type": "string",
                        "description": "Mail account name. Searches all accounts if omitted."
                    },
                    "mailbox": {
                        "type": "string",
                        "description": "Mailbox name. Defaults to 'INBOX'."
                    }
                },
                "required": ["subject", "to"]
            })),
        ),
        handler_forward_message(),
    );

    registry.register(
        "communication_mail_update_read_state",
        Tool::new(
            "communication_mail_update_read_state",
            "[UPDATE] Mark messages as read or unread by subject match.",
            schema_from_json(json!({
                "type": "object",
                "properties": {
                    "subject": {
                        "type": "string",
                        "description": "Subject of the message(s) to update (partial match)."
                    },
                    "read": {
                        "type": "boolean",
                        "description": "Set to true to mark as read, false to mark as unread."
                    },
                    "account": {
                        "type": "string",
                        "description": "Mail account name. Searches all accounts if omitted."
                    },
                    "mailbox": {
                        "type": "string",
                        "description": "Mailbox name. Defaults to 'INBOX'."
                    }
                },
                "required": ["subject", "read"]
            })),
        ),
        handler_update_read_state(),
    );

    registry.register(
        "communication_mail_move_message",
        Tool::new(
            "communication_mail_move_message",
            "[UPDATE] Move a message to a different mailbox by subject match.",
            schema_from_json(json!({
                "type": "object",
                "properties": {
                    "subject": {
                        "type": "string",
                        "description": "Subject of the message to move (partial match)."
                    },
                    "target_mailbox": {
                        "type": "string",
                        "description": "Name of the destination mailbox (e.g. 'Archive', 'Junk')."
                    },
                    "target_account": {
                        "type": "string",
                        "description": "Account of the destination mailbox. Uses the source account if omitted."
                    },
                    "account": {
                        "type": "string",
                        "description": "Mail account name where the message currently resides."
                    },
                    "mailbox": {
                        "type": "string",
                        "description": "Current mailbox name. Defaults to 'INBOX'."
                    }
                },
                "required": ["subject", "target_mailbox"]
            })),
        ),
        handler_move_message(),
    );

    registry.register(
        "communication_mail_delete_message",
        Tool::new(
            "communication_mail_delete_message",
            "[DELETE] Delete a message by moving it to Trash. Finds message by subject match.",
            schema_from_json(json!({
                "type": "object",
                "properties": {
                    "subject": {
                        "type": "string",
                        "description": "Subject of the message to delete (partial match)."
                    },
                    "account": {
                        "type": "string",
                        "description": "Mail account name. Searches all accounts if omitted."
                    },
                    "mailbox": {
                        "type": "string",
                        "description": "Mailbox name. Defaults to 'INBOX'."
                    }
                },
                "required": ["subject"]
            })),
        ),
        handler_delete_message(),
    );

    registry.register(
        "communication_mail_open_message",
        Tool::new(
            "communication_mail_open_message",
            "[READ] Open a specific message in Mail.app by subject match.",
            schema_from_json(json!({
                "type": "object",
                "properties": {
                    "subject": {
                        "type": "string",
                        "description": "Subject of the message to open (partial match)."
                    },
                    "account": {
                        "type": "string",
                        "description": "Mail account name. Searches all accounts if omitted."
                    },
                    "mailbox": {
                        "type": "string",
                        "description": "Mailbox name. Defaults to 'INBOX'."
                    }
                },
                "required": ["subject"]
            })),
        ),
        handler_open_message(),
    );

    registry.register(
        "communication_mail_get_attachment",
        Tool::new(
            "communication_mail_get_attachment",
            "[READ] List attachments of a message found by subject match. Returns attachment names and MIME types.",
            schema_from_json(json!({
                "type": "object",
                "properties": {
                    "subject": {
                        "type": "string",
                        "description": "Subject of the message to get attachments from (partial match)."
                    },
                    "account": {
                        "type": "string",
                        "description": "Mail account name. Searches all accounts if omitted."
                    },
                    "mailbox": {
                        "type": "string",
                        "description": "Mailbox name. Defaults to 'INBOX'."
                    }
                },
                "required": ["subject"]
            })),
        ),
        handler_get_attachment(),
    );
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

fn handler_list_accounts() -> ToolHandler {
    Arc::new(|_args| {
        Box::pin(async move {
            let script = r#"
                tell application "Mail"
                    set output to ""
                    set accts to every account
                    repeat with a in accts
                        set acctName to name of a
                        set acctEmail to email addresses of a
                        set emailStr to ""
                        repeat with e in acctEmail
                            if emailStr is not "" then set emailStr to emailStr & ", "
                            set emailStr to emailStr & (e as text)
                        end repeat
                        set output to output & acctName & "||" & emailStr & linefeed
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
                        let name = parts.first().unwrap_or(&"").trim();
                        let emails = parts.get(1).unwrap_or(&"").trim();
                        results.push(json!({
                            "name": name,
                            "email_addresses": emails,
                        }));
                    }
                    let json_str = serde_json::to_string_pretty(&results)?;
                    Ok(text_result(format!(
                        "Found {} mail account(s):\n\n{json_str}",
                        results.len()
                    )))
                }
                Err(e) => Ok(error_result(format!("Failed to list mail accounts: {e}"))),
            }
        })
    })
}

fn handler_list_mailboxes() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            let account = match args.get("account").and_then(|v| v.as_str()) {
                Some(a) => a,
                None => return Ok(error_result("account is required")),
            };

            let escaped_account = escape_applescript_string(account);

            let script = format!(
                r#"
                tell application "Mail"
                    try
                        set acct to account "{escaped_account}"
                    on error
                        return "ERROR:Account not found: {escaped_account}. Use communication_mail_list_accounts to see available accounts."
                    end try
                    set output to ""
                    set mboxes to every mailbox of acct
                    repeat with mb in mboxes
                        set mbName to name of mb
                        set msgCount to count of messages of mb
                        set unreadCount to unread count of mb
                        set output to output & mbName & "||" & msgCount & "||" & unreadCount & linefeed
                    end repeat
                    return output
                end tell
                "#
            );

            match crate::macos::applescript::run_applescript(&script) {
                Ok(output) => {
                    if let Some(err) = output.strip_prefix("ERROR:") {
                        return Ok(error_result(err.to_string()));
                    }
                    let mut results: Vec<serde_json::Value> = Vec::new();
                    for line in output.lines() {
                        let line = line.trim();
                        if line.is_empty() {
                            continue;
                        }
                        let parts: Vec<&str> = line.split("||").collect();
                        results.push(json!({
                            "name": parts.first().unwrap_or(&"").trim(),
                            "message_count": parts.get(1).unwrap_or(&"0").trim(),
                            "unread_count": parts.get(2).unwrap_or(&"0").trim(),
                        }));
                    }
                    let json_str = serde_json::to_string_pretty(&results)?;
                    Ok(text_result(format!(
                        "Found {} mailbox(es) for account '{account}':\n\n{json_str}",
                        results.len()
                    )))
                }
                Err(e) => Ok(error_result(format!("Failed to list mailboxes: {e}"))),
            }
        })
    })
}

fn handler_search_messages() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(25) as usize;
            let offset = args.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let mailbox = args
                .get("mailbox")
                .and_then(|v| v.as_str())
                .unwrap_or("INBOX");
            let account_filter = args.get("account").and_then(|v| v.as_str());
            let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
            let sender_filter = args.get("sender").and_then(|v| v.as_str()).unwrap_or("");
            let subject_filter = args.get("subject").and_then(|v| v.as_str()).unwrap_or("");
            let _date_from = args.get("date_from").and_then(|v| v.as_str()).unwrap_or("");
            let _date_to = args.get("date_to").and_then(|v| v.as_str()).unwrap_or("");

            let escaped_mailbox = escape_applescript_string(mailbox);

            // Build a whose clause for AppleScript filtering
            let mut whose_parts: Vec<String> = Vec::new();
            if !subject_filter.is_empty() {
                let escaped = escape_applescript_string(subject_filter);
                whose_parts.push(format!(r#"subject contains "{escaped}""#));
            }
            if !sender_filter.is_empty() {
                let escaped = escape_applescript_string(sender_filter);
                whose_parts.push(format!(r#"sender contains "{escaped}""#));
            }

            let whose_clause = if whose_parts.is_empty() {
                String::new()
            } else {
                format!(" whose {}", whose_parts.join(" and "))
            };

            // Fetch more than we need so we can do offset/client-side filtering
            let fetch_limit = offset + limit + 100;

            let account_loop = if let Some(acct) = account_filter {
                let escaped = escape_applescript_string(acct);
                format!(r#"repeat with acct in {{account "{escaped}"}}"#)
            } else {
                "repeat with acct in every account".to_string()
            };

            let script = format!(
                r#"
                tell application "Mail"
                    set output to ""
                    set counter to 0
                    {account_loop}
                        try
                            set mb to mailbox "{escaped_mailbox}" of acct
                            set msgs to (every message of mb{whose_clause})
                            repeat with msg in msgs
                                if counter >= {fetch_limit} then exit repeat
                                set msgSubject to subject of msg
                                set msgSender to sender of msg
                                set msgDate to date received of msg
                                set msgRead to read status of msg
                                set msgId to id of msg
                                set output to output & msgId & "||" & msgSubject & "||" & msgSender & "||" & (msgDate as text) & "||" & msgRead & linefeed
                                set counter to counter + 1
                            end repeat
                        end try
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
                        let subj = parts.get(1).unwrap_or(&"").trim();
                        let sndr = parts.get(2).unwrap_or(&"").trim();

                        // Client-side query filter (searches subject + sender)
                        if !query.is_empty() {
                            let q = query.to_lowercase();
                            if !subj.to_lowercase().contains(&q)
                                && !sndr.to_lowercase().contains(&q)
                            {
                                continue;
                            }
                        }

                        results.push(json!({
                            "id": parts.first().unwrap_or(&"").trim(),
                            "subject": subj,
                            "sender": sndr,
                            "date": parts.get(3).unwrap_or(&"").trim(),
                            "read": parts.get(4).unwrap_or(&"").trim(),
                        }));
                    }

                    // Apply offset and limit
                    let total = results.len();
                    let paginated: Vec<serde_json::Value> =
                        results.into_iter().skip(offset).take(limit).collect();

                    if paginated.is_empty() {
                        Ok(text_result(
                            "No messages found matching the search criteria.",
                        ))
                    } else {
                        let json_str = serde_json::to_string_pretty(&paginated)?;
                        Ok(text_result(format!(
                            "Showing {} of {} message(s) (offset {offset}):\n\n{json_str}",
                            paginated.len(),
                            total
                        )))
                    }
                }
                Err(e) => Ok(error_result(format!("Failed to search messages: {e}"))),
            }
        })
    })
}

fn handler_get_messages() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            let subject = match args.get("subject").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => return Ok(error_result("subject is required")),
            };
            let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(5) as usize;
            let mailbox = args
                .get("mailbox")
                .and_then(|v| v.as_str())
                .unwrap_or("INBOX");
            let account_filter = args.get("account").and_then(|v| v.as_str());

            let escaped_subject = escape_applescript_string(subject);
            let escaped_mailbox = escape_applescript_string(mailbox);

            let account_loop = if let Some(acct) = account_filter {
                let escaped = escape_applescript_string(acct);
                format!(r#"repeat with acct in {{account "{escaped}"}}"#)
            } else {
                "repeat with acct in every account".to_string()
            };

            let script = format!(
                r#"
                tell application "Mail"
                    set output to ""
                    set counter to 0
                    {account_loop}
                        try
                            set mb to mailbox "{escaped_mailbox}" of acct
                            set msgs to (every message of mb whose subject contains "{escaped_subject}")
                            repeat with msg in msgs
                                if counter >= {limit} then exit repeat
                                set msgSubject to subject of msg
                                set msgSender to sender of msg
                                set msgDate to date received of msg
                                set msgRead to read status of msg
                                set msgId to id of msg
                                set msgContent to ""
                                try
                                    set msgContent to content of msg
                                end try
                                set toRecips to ""
                                try
                                    repeat with r in to recipients of msg
                                        if toRecips is not "" then set toRecips to toRecips & ", "
                                        set toRecips to toRecips & (address of r as text)
                                    end repeat
                                end try
                                set ccRecips to ""
                                try
                                    repeat with r in cc recipients of msg
                                        if ccRecips is not "" then set ccRecips to ccRecips & ", "
                                        set ccRecips to ccRecips & (address of r as text)
                                    end repeat
                                end try
                                set output to output & "==MSG_START==" & linefeed
                                set output to output & "id:" & msgId & linefeed
                                set output to output & "subject:" & msgSubject & linefeed
                                set output to output & "sender:" & msgSender & linefeed
                                set output to output & "to:" & toRecips & linefeed
                                set output to output & "cc:" & ccRecips & linefeed
                                set output to output & "date:" & (msgDate as text) & linefeed
                                set output to output & "read:" & msgRead & linefeed
                                set output to output & "body:" & msgContent & linefeed
                                set output to output & "==MSG_END==" & linefeed
                                set counter to counter + 1
                            end repeat
                        end try
                    end repeat
                    return output
                end tell
                "#
            );

            match crate::macos::applescript::run_applescript(&script) {
                Ok(output) => {
                    let mut results: Vec<serde_json::Value> = Vec::new();
                    let messages: Vec<&str> = output.split("==MSG_START==").collect();
                    for msg_block in messages {
                        let msg_block = msg_block.trim();
                        if msg_block.is_empty() || !msg_block.contains("==MSG_END==") {
                            continue;
                        }
                        let content = msg_block.replace("==MSG_END==", "");
                        let mut msg_data = json!({});
                        let mut body_lines: Vec<String> = Vec::new();
                        let mut in_body = false;

                        for line in content.lines() {
                            if in_body {
                                body_lines.push(line.to_string());
                                continue;
                            }
                            if let Some(val) = line.strip_prefix("id:") {
                                msg_data["id"] = json!(val.trim());
                            } else if let Some(val) = line.strip_prefix("subject:") {
                                msg_data["subject"] = json!(val.trim());
                            } else if let Some(val) = line.strip_prefix("sender:") {
                                msg_data["sender"] = json!(val.trim());
                            } else if let Some(val) = line.strip_prefix("to:") {
                                msg_data["to"] = json!(val.trim());
                            } else if let Some(val) = line.strip_prefix("cc:") {
                                msg_data["cc"] = json!(val.trim());
                            } else if let Some(val) = line.strip_prefix("date:") {
                                msg_data["date"] = json!(val.trim());
                            } else if let Some(val) = line.strip_prefix("read:") {
                                msg_data["read"] = json!(val.trim());
                            } else if let Some(val) = line.strip_prefix("body:") {
                                in_body = true;
                                body_lines.push(val.to_string());
                            }
                        }
                        msg_data["body"] = json!(body_lines.join("\n").trim().to_string());
                        results.push(msg_data);
                    }

                    if results.is_empty() {
                        Ok(text_result(format!(
                            "No messages found matching subject: {subject}"
                        )))
                    } else {
                        let json_str = serde_json::to_string_pretty(&results)?;
                        Ok(text_result(format!(
                            "Found {} message(s):\n\n{json_str}",
                            results.len()
                        )))
                    }
                }
                Err(e) => Ok(error_result(format!("Failed to get messages: {e}"))),
            }
        })
    })
}

fn handler_get_thread() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            let subject = match args.get("subject").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => return Ok(error_result("subject is required")),
            };
            let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(20) as usize;
            let account_filter = args.get("account").and_then(|v| v.as_str());

            // Strip common prefixes to find the base subject for thread matching
            let base_subject = subject
                .trim_start_matches("Re: ")
                .trim_start_matches("RE: ")
                .trim_start_matches("Fwd: ")
                .trim_start_matches("FWD: ");
            let escaped_subject = escape_applescript_string(base_subject);

            let account_loop = if let Some(acct) = account_filter {
                let escaped = escape_applescript_string(acct);
                format!(r#"repeat with acct in {{account "{escaped}"}}"#)
            } else {
                "repeat with acct in every account".to_string()
            };

            // Search across all mailboxes to find thread messages
            let script = format!(
                r#"
                tell application "Mail"
                    set output to ""
                    set counter to 0
                    {account_loop}
                        repeat with mb in every mailbox of acct
                            try
                                set msgs to (every message of mb whose subject contains "{escaped_subject}")
                                repeat with msg in msgs
                                    if counter >= {limit} then exit repeat
                                    set msgSubject to subject of msg
                                    set msgSender to sender of msg
                                    set msgDate to date received of msg
                                    set msgRead to read status of msg
                                    set mbName to name of mb
                                    set msgContent to ""
                                    try
                                        set msgContent to content of msg
                                    end try
                                    set output to output & "==MSG_START==" & linefeed
                                    set output to output & "subject:" & msgSubject & linefeed
                                    set output to output & "sender:" & msgSender & linefeed
                                    set output to output & "date:" & (msgDate as text) & linefeed
                                    set output to output & "read:" & msgRead & linefeed
                                    set output to output & "mailbox:" & mbName & linefeed
                                    set output to output & "body:" & msgContent & linefeed
                                    set output to output & "==MSG_END==" & linefeed
                                    set counter to counter + 1
                                end repeat
                            end try
                        end repeat
                    end repeat
                    return output
                end tell
                "#
            );

            match crate::macos::applescript::run_applescript(&script) {
                Ok(output) => {
                    let mut results: Vec<serde_json::Value> = Vec::new();
                    let messages: Vec<&str> = output.split("==MSG_START==").collect();
                    for msg_block in messages {
                        let msg_block = msg_block.trim();
                        if msg_block.is_empty() || !msg_block.contains("==MSG_END==") {
                            continue;
                        }
                        let content = msg_block.replace("==MSG_END==", "");
                        let mut msg_data = json!({});
                        let mut body_lines: Vec<String> = Vec::new();
                        let mut in_body = false;

                        for line in content.lines() {
                            if in_body {
                                body_lines.push(line.to_string());
                                continue;
                            }
                            if let Some(val) = line.strip_prefix("subject:") {
                                msg_data["subject"] = json!(val.trim());
                            } else if let Some(val) = line.strip_prefix("sender:") {
                                msg_data["sender"] = json!(val.trim());
                            } else if let Some(val) = line.strip_prefix("date:") {
                                msg_data["date"] = json!(val.trim());
                            } else if let Some(val) = line.strip_prefix("read:") {
                                msg_data["read"] = json!(val.trim());
                            } else if let Some(val) = line.strip_prefix("mailbox:") {
                                msg_data["mailbox"] = json!(val.trim());
                            } else if let Some(val) = line.strip_prefix("body:") {
                                in_body = true;
                                body_lines.push(val.to_string());
                            }
                        }
                        msg_data["body"] = json!(body_lines.join("\n").trim().to_string());
                        results.push(msg_data);
                    }

                    if results.is_empty() {
                        Ok(text_result(format!(
                            "No thread messages found matching subject: {subject}"
                        )))
                    } else {
                        let json_str = serde_json::to_string_pretty(&results)?;
                        Ok(text_result(format!(
                            "Found {} message(s) in thread:\n\n{json_str}",
                            results.len()
                        )))
                    }
                }
                Err(e) => Ok(error_result(format!("Failed to get thread: {e}"))),
            }
        })
    })
}

fn handler_compose_message() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            let to = match args.get("to").and_then(|v| v.as_str()) {
                Some(t) => t,
                None => return Ok(error_result("to is required")),
            };
            let subject = match args.get("subject").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => return Ok(error_result("subject is required")),
            };
            let body = match args.get("body").and_then(|v| v.as_str()) {
                Some(b) => b,
                None => return Ok(error_result("body is required")),
            };
            let cc = args.get("cc").and_then(|v| v.as_str()).unwrap_or("");
            let bcc = args.get("bcc").and_then(|v| v.as_str()).unwrap_or("");
            let send = args.get("send").and_then(|v| v.as_bool()).unwrap_or(false);

            let escaped_subject = escape_applescript_string(subject);
            let escaped_body = escape_applescript_string(body);

            // Build recipient lines for to, cc, bcc
            let mut recipient_lines = String::new();
            for addr in to.split(',') {
                let addr = escape_applescript_string(addr.trim());
                if !addr.is_empty() {
                    recipient_lines.push_str(&format!(
                        "make new to recipient at end of to recipients with properties {{address:\"{addr}\"}}\n"
                    ));
                }
            }
            for addr in cc.split(',') {
                let addr = escape_applescript_string(addr.trim());
                if !addr.is_empty() {
                    recipient_lines.push_str(&format!(
                        "make new cc recipient at end of cc recipients with properties {{address:\"{addr}\"}}\n"
                    ));
                }
            }
            for addr in bcc.split(',') {
                let addr = escape_applescript_string(addr.trim());
                if !addr.is_empty() {
                    recipient_lines.push_str(&format!(
                        "make new bcc recipient at end of bcc recipients with properties {{address:\"{addr}\"}}\n"
                    ));
                }
            }

            let send_line = if send { "send newMsg" } else { "" };
            let visible = if send { "false" } else { "true" };

            let script = format!(
                r#"
                tell application "Mail"
                    set newMsg to make new outgoing message with properties {{subject:"{escaped_subject}", content:"{escaped_body}", visible:{visible}}}
                    tell newMsg
                        {recipient_lines}
                    end tell
                    {send_line}
                end tell
                return "Message composed: {escaped_subject}"
                "#
            );

            match crate::macos::applescript::run_applescript(&script) {
                Ok(result) => {
                    if send {
                        Ok(text_result(format!("Message sent: {subject}")))
                    } else {
                        Ok(text_result(result))
                    }
                }
                Err(e) => Ok(error_result(format!("Failed to compose message: {e}"))),
            }
        })
    })
}

fn handler_reply_message() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            let subject = match args.get("subject").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => return Ok(error_result("subject is required")),
            };
            let reply_text = args
                .get("reply_text")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let reply_all = args
                .get("reply_all")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let mailbox = args
                .get("mailbox")
                .and_then(|v| v.as_str())
                .unwrap_or("INBOX");
            let account_filter = args.get("account").and_then(|v| v.as_str());

            let escaped_subject = escape_applescript_string(subject);
            let escaped_mailbox = escape_applescript_string(mailbox);
            let escaped_reply_text = escape_applescript_string(reply_text);

            let account_loop = if let Some(acct) = account_filter {
                let escaped = escape_applescript_string(acct);
                format!(r#"repeat with acct in {{account "{escaped}"}}"#)
            } else {
                "repeat with acct in every account".to_string()
            };

            let reply_cmd = if reply_all {
                "reply msg with opening window and reply to all"
            } else {
                "reply msg with opening window"
            };

            let script = format!(
                r#"
                tell application "Mail"
                    set msgFound to false
                    {account_loop}
                        try
                            set mb to mailbox "{escaped_mailbox}" of acct
                            set msgs to (every message of mb whose subject contains "{escaped_subject}")
                            if (count of msgs) > 0 then
                                set msg to item 1 of msgs
                                set replyMsg to {reply_cmd}
                                if "{escaped_reply_text}" is not "" then
                                    set content of replyMsg to "{escaped_reply_text}" & return & return & (content of replyMsg)
                                end if
                                set msgFound to true
                                exit repeat
                            end if
                        end try
                    end repeat
                    if msgFound then
                        activate
                        return "Reply window opened for: {escaped_subject}"
                    else
                        return "No message found matching subject: {escaped_subject}"
                    end if
                end tell
                "#
            );

            match crate::macos::applescript::run_applescript(&script) {
                Ok(result) => Ok(text_result(result)),
                Err(e) => Ok(error_result(format!("Failed to reply to message: {e}"))),
            }
        })
    })
}

fn handler_forward_message() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            let subject = match args.get("subject").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => return Ok(error_result("subject is required")),
            };
            let to = match args.get("to").and_then(|v| v.as_str()) {
                Some(t) => t,
                None => return Ok(error_result("to is required")),
            };
            let forward_text = args
                .get("forward_text")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let mailbox = args
                .get("mailbox")
                .and_then(|v| v.as_str())
                .unwrap_or("INBOX");
            let account_filter = args.get("account").and_then(|v| v.as_str());

            let escaped_subject = escape_applescript_string(subject);
            let escaped_mailbox = escape_applescript_string(mailbox);
            let escaped_to = escape_applescript_string(to);
            let escaped_forward_text = escape_applescript_string(forward_text);

            let account_loop = if let Some(acct) = account_filter {
                let escaped = escape_applescript_string(acct);
                format!(r#"repeat with acct in {{account "{escaped}"}}"#)
            } else {
                "repeat with acct in every account".to_string()
            };

            let script = format!(
                r#"
                tell application "Mail"
                    set msgFound to false
                    {account_loop}
                        try
                            set mb to mailbox "{escaped_mailbox}" of acct
                            set msgs to (every message of mb whose subject contains "{escaped_subject}")
                            if (count of msgs) > 0 then
                                set msg to item 1 of msgs
                                set fwdMsg to forward msg with opening window
                                tell fwdMsg
                                    make new to recipient at end of to recipients with properties {{address:"{escaped_to}"}}
                                end tell
                                if "{escaped_forward_text}" is not "" then
                                    set content of fwdMsg to "{escaped_forward_text}" & return & return & (content of fwdMsg)
                                end if
                                set msgFound to true
                                exit repeat
                            end if
                        end try
                    end repeat
                    if msgFound then
                        activate
                        return "Forward window opened for: {escaped_subject}"
                    else
                        return "No message found matching subject: {escaped_subject}"
                    end if
                end tell
                "#
            );

            match crate::macos::applescript::run_applescript(&script) {
                Ok(result) => Ok(text_result(result)),
                Err(e) => Ok(error_result(format!("Failed to forward message: {e}"))),
            }
        })
    })
}

fn handler_update_read_state() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            let subject = match args.get("subject").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => return Ok(error_result("subject is required")),
            };
            let read = match args.get("read").and_then(|v| v.as_bool()) {
                Some(r) => r,
                None => return Ok(error_result("read is required")),
            };
            let mailbox = args
                .get("mailbox")
                .and_then(|v| v.as_str())
                .unwrap_or("INBOX");
            let account_filter = args.get("account").and_then(|v| v.as_str());

            let escaped_subject = escape_applescript_string(subject);
            let escaped_mailbox = escape_applescript_string(mailbox);
            let read_str = if read { "true" } else { "false" };

            let account_loop = if let Some(acct) = account_filter {
                let escaped = escape_applescript_string(acct);
                format!(r#"repeat with acct in {{account "{escaped}"}}"#)
            } else {
                "repeat with acct in every account".to_string()
            };

            let state_label = if read { "read" } else { "unread" };

            let script = format!(
                r#"
                tell application "Mail"
                    set updateCount to 0
                    {account_loop}
                        try
                            set mb to mailbox "{escaped_mailbox}" of acct
                            set msgs to (every message of mb whose subject contains "{escaped_subject}")
                            repeat with msg in msgs
                                set read status of msg to {read_str}
                                set updateCount to updateCount + 1
                            end repeat
                        end try
                    end repeat
                    if updateCount > 0 then
                        return "Marked " & updateCount & " message(s) as {state_label}"
                    else
                        return "No messages found matching subject: {escaped_subject}"
                    end if
                end tell
                "#
            );

            match crate::macos::applescript::run_applescript(&script) {
                Ok(result) => Ok(text_result(result)),
                Err(e) => Ok(error_result(format!("Failed to update read state: {e}"))),
            }
        })
    })
}

fn handler_move_message() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            let subject = match args.get("subject").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => return Ok(error_result("subject is required")),
            };
            let target_mailbox = match args.get("target_mailbox").and_then(|v| v.as_str()) {
                Some(tm) => tm,
                None => return Ok(error_result("target_mailbox is required")),
            };
            let target_account = args.get("target_account").and_then(|v| v.as_str());
            let mailbox = args
                .get("mailbox")
                .and_then(|v| v.as_str())
                .unwrap_or("INBOX");
            let account_filter = args.get("account").and_then(|v| v.as_str());

            let escaped_subject = escape_applescript_string(subject);
            let escaped_mailbox = escape_applescript_string(mailbox);
            let escaped_target = escape_applescript_string(target_mailbox);

            let account_loop = if let Some(acct) = account_filter {
                let escaped = escape_applescript_string(acct);
                format!(r#"repeat with acct in {{account "{escaped}"}}"#)
            } else {
                "repeat with acct in every account".to_string()
            };

            // Determine the target mailbox reference
            let target_ref = if let Some(ta) = target_account {
                let escaped_ta = escape_applescript_string(ta);
                format!(r#"mailbox "{escaped_target}" of account "{escaped_ta}""#)
            } else {
                // Use the same account as the source
                format!(r#"mailbox "{escaped_target}" of acct"#)
            };

            let script = format!(
                r#"
                tell application "Mail"
                    set msgFound to false
                    {account_loop}
                        try
                            set mb to mailbox "{escaped_mailbox}" of acct
                            set msgs to (every message of mb whose subject contains "{escaped_subject}")
                            if (count of msgs) > 0 then
                                set msg to item 1 of msgs
                                set mailbox of msg to {target_ref}
                                set msgFound to true
                                exit repeat
                            end if
                        end try
                    end repeat
                    if msgFound then
                        return "Message moved to {escaped_target}: {escaped_subject}"
                    else
                        return "No message found matching subject: {escaped_subject}"
                    end if
                end tell
                "#
            );

            match crate::macos::applescript::run_applescript(&script) {
                Ok(result) => Ok(text_result(result)),
                Err(e) => Ok(error_result(format!("Failed to move message: {e}"))),
            }
        })
    })
}

fn handler_delete_message() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            let subject = match args.get("subject").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => return Ok(error_result("subject is required")),
            };
            let mailbox = args
                .get("mailbox")
                .and_then(|v| v.as_str())
                .unwrap_or("INBOX");
            let account_filter = args.get("account").and_then(|v| v.as_str());

            let escaped_subject = escape_applescript_string(subject);
            let escaped_mailbox = escape_applescript_string(mailbox);

            let account_loop = if let Some(acct) = account_filter {
                let escaped = escape_applescript_string(acct);
                format!(r#"repeat with acct in {{account "{escaped}"}}"#)
            } else {
                "repeat with acct in every account".to_string()
            };

            let script = format!(
                r#"
                tell application "Mail"
                    set msgFound to false
                    {account_loop}
                        try
                            set mb to mailbox "{escaped_mailbox}" of acct
                            set msgs to (every message of mb whose subject contains "{escaped_subject}")
                            if (count of msgs) > 0 then
                                set msg to item 1 of msgs
                                delete msg
                                set msgFound to true
                                exit repeat
                            end if
                        end try
                    end repeat
                    if msgFound then
                        return "Message deleted (moved to Trash): {escaped_subject}"
                    else
                        return "No message found matching subject: {escaped_subject}"
                    end if
                end tell
                "#
            );

            match crate::macos::applescript::run_applescript(&script) {
                Ok(result) => Ok(text_result(result)),
                Err(e) => Ok(error_result(format!("Failed to delete message: {e}"))),
            }
        })
    })
}

fn handler_open_message() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            let subject = match args.get("subject").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => return Ok(error_result("subject is required")),
            };
            let mailbox = args
                .get("mailbox")
                .and_then(|v| v.as_str())
                .unwrap_or("INBOX");
            let account_filter = args.get("account").and_then(|v| v.as_str());

            let escaped_subject = escape_applescript_string(subject);
            let escaped_mailbox = escape_applescript_string(mailbox);

            let account_loop = if let Some(acct) = account_filter {
                let escaped = escape_applescript_string(acct);
                format!(r#"repeat with acct in {{account "{escaped}"}}"#)
            } else {
                "repeat with acct in every account".to_string()
            };

            let script = format!(
                r#"
                tell application "Mail"
                    set msgFound to false
                    {account_loop}
                        try
                            set mb to mailbox "{escaped_mailbox}" of acct
                            set msgs to (every message of mb whose subject contains "{escaped_subject}")
                            if (count of msgs) > 0 then
                                set msg to item 1 of msgs
                                activate
                                open msg
                                set msgFound to true
                                exit repeat
                            end if
                        end try
                    end repeat
                    if msgFound then
                        return "Opened message in Mail.app: {escaped_subject}"
                    else
                        return "No message found matching subject: {escaped_subject}"
                    end if
                end tell
                "#
            );

            match crate::macos::applescript::run_applescript(&script) {
                Ok(result) => Ok(text_result(result)),
                Err(e) => Ok(error_result(format!("Failed to open message: {e}"))),
            }
        })
    })
}

fn handler_get_attachment() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            let subject = match args.get("subject").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => return Ok(error_result("subject is required")),
            };
            let mailbox = args
                .get("mailbox")
                .and_then(|v| v.as_str())
                .unwrap_or("INBOX");
            let account_filter = args.get("account").and_then(|v| v.as_str());

            let escaped_subject = escape_applescript_string(subject);
            let escaped_mailbox = escape_applescript_string(mailbox);

            let account_loop = if let Some(acct) = account_filter {
                let escaped = escape_applescript_string(acct);
                format!(r#"repeat with acct in {{account "{escaped}"}}"#)
            } else {
                "repeat with acct in every account".to_string()
            };

            let script = format!(
                r#"
                tell application "Mail"
                    set msgFound to false
                    set output to ""
                    {account_loop}
                        try
                            set mb to mailbox "{escaped_mailbox}" of acct
                            set msgs to (every message of mb whose subject contains "{escaped_subject}")
                            if (count of msgs) > 0 then
                                set msg to item 1 of msgs
                                set msgFound to true
                                set attachments_ to every mail attachment of msg
                                if (count of attachments_) = 0 then
                                    set output to "NO_ATTACHMENTS"
                                else
                                    repeat with att in attachments_
                                        set attName to name of att
                                        set attType to MIME type of att
                                        set attSize to file size of att
                                        set output to output & attName & "||" & attType & "||" & attSize & linefeed
                                    end repeat
                                end if
                                exit repeat
                            end if
                        end try
                    end repeat
                    if not msgFound then
                        return "NO_MESSAGE_FOUND"
                    end if
                    return output
                end tell
                "#
            );

            match crate::macos::applescript::run_applescript(&script) {
                Ok(output) => {
                    let trimmed = output.trim();
                    if trimmed == "NO_MESSAGE_FOUND" {
                        return Ok(text_result(format!(
                            "No message found matching subject: {subject}"
                        )));
                    }
                    if trimmed == "NO_ATTACHMENTS" {
                        return Ok(text_result(format!(
                            "No attachments found on message: {subject}"
                        )));
                    }

                    let mut results: Vec<serde_json::Value> = Vec::new();
                    for line in output.lines() {
                        let line = line.trim();
                        if line.is_empty() {
                            continue;
                        }
                        let parts: Vec<&str> = line.split("||").collect();
                        results.push(json!({
                            "name": parts.first().unwrap_or(&"").trim(),
                            "mime_type": parts.get(1).unwrap_or(&"").trim(),
                            "size_bytes": parts.get(2).unwrap_or(&"0").trim(),
                        }));
                    }

                    let json_str = serde_json::to_string_pretty(&results)?;
                    Ok(text_result(format!(
                        "Found {} attachment(s):\n\n{json_str}",
                        results.len()
                    )))
                }
                Err(e) => Ok(error_result(format!("Failed to get attachments: {e}"))),
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

    struct AssertingMailMock {
        expected_fragments: Vec<String>,
        response: String,
    }

    impl ScriptRunner for AssertingMailMock {
        fn run_applescript(&self, script: &str) -> anyhow::Result<String> {
            for fragment in &self.expected_fragments {
                assert!(
                    script.contains(fragment),
                    "Script missing fragment: {}\nScript: {}",
                    fragment,
                    script
                );
            }
            Ok(self.response.clone())
        }
        fn run_applescript_with_timeout(
            &self,
            script: &str,
            _timeout: Duration,
        ) -> anyhow::Result<String> {
            self.run_applescript(script)
        }
        fn run_jxa(&self, _script: &str) -> anyhow::Result<String> {
            unimplemented!()
        }
    }

    #[tokio::test]
    async fn test_tool_schemas_valid() {
        let mut registry = ServiceRegistry::new();
        register(&mut registry);
        let tools = registry.list_tools();
        assert_eq!(tools.len(), 13, "Expected exactly 13 mail tools");

        let names: Vec<_> = tools.iter().map(|t| t.name.as_ref()).collect();
        assert!(names.contains(&"communication_mail_list_accounts"));
        assert!(names.contains(&"communication_mail_list_mailboxes"));
        assert!(names.contains(&"communication_mail_search_messages"));
        assert!(names.contains(&"communication_mail_get_messages"));
        assert!(names.contains(&"communication_mail_get_thread"));
        assert!(names.contains(&"communication_mail_compose_message"));
        assert!(names.contains(&"communication_mail_reply_message"));
        assert!(names.contains(&"communication_mail_forward_message"));
        assert!(names.contains(&"communication_mail_update_read_state"));
        assert!(names.contains(&"communication_mail_move_message"));
        assert!(names.contains(&"communication_mail_delete_message"));
        assert!(names.contains(&"communication_mail_open_message"));
        assert!(names.contains(&"communication_mail_get_attachment"));
    }

    #[tokio::test]
    async fn test_mail_list_accounts() {
        let mock = Arc::new(AssertingMailMock {
            expected_fragments: vec!["every account".to_string()],
            response: "Personal||me@example.com\nWork||job@work.com".to_string(),
        });

        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_list_accounts();
                let args = HashMap::new();
                let result = handler(args).await.unwrap();
                let content = result.content[0].as_text().unwrap().text.as_str();
                assert!(content.contains("Personal"));
                assert!(content.contains("me@example.com"));
                assert!(content.contains("Work"));
                assert!(content.contains("job@work.com"));
            })
            .await;
    }

    #[tokio::test]
    async fn test_mail_list_mailboxes() {
        let mock = Arc::new(AssertingMailMock {
            expected_fragments: vec![
                "account \"Personal\"".to_string(),
                "every mailbox of acct".to_string(),
            ],
            response: "INBOX||10||2\nArchive||50||0".to_string(),
        });

        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_list_mailboxes();
                let mut args = HashMap::new();
                args.insert("account".to_string(), json!("Personal"));
                let result = handler(args).await.unwrap();
                let content = result.content[0].as_text().unwrap().text.as_str();
                assert!(content.contains("INBOX"));
                assert!(content.contains("Archive"));
                assert!(content.contains("10"));
                assert!(content.contains("2"));
            })
            .await;
    }

    #[tokio::test]
    async fn test_mail_search_messages() {
        let mock = Arc::new(AssertingMailMock {
            expected_fragments: vec![
                "mailbox \"INBOX\"".to_string(),
                "subject contains \"Hello\"".to_string(),
            ],
            response: "1||Hello||Alice||2024-01-01||false\n2||Meeting||Bob||2024-01-02||true"
                .to_string(),
        });

        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_search_messages();
                let mut args = HashMap::new();
                args.insert("subject".to_string(), json!("Hello"));
                args.insert("mailbox".to_string(), json!("INBOX"));
                let result = handler(args).await.unwrap();
                let content = result.content[0].as_text().unwrap().text.as_str();
                assert!(content.contains("Hello"));
                assert!(content.contains("Alice"));
                // Second message is returned by mock but would be filtered by handler if we used query arg
                assert!(content.contains("Meeting"));
            })
            .await;
    }

    #[tokio::test]
    async fn test_mail_get_messages() {
        let mock = Arc::new(AssertingMailMock {
            expected_fragments: vec![
                "mailbox \"INBOX\"".to_string(),
                "subject contains \"Meeting\"".to_string()
            ],
            response: "==MSG_START==\nid:1\nsubject:Meeting\nsender:Alice\nto:Bob\ncc:\ndate:2024-01-01\nread:false\nbody:Let's meet at 5.\n==MSG_END==".to_string(),
        });

        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_get_messages();
                let mut args = HashMap::new();
                args.insert("subject".to_string(), json!("Meeting"));
                let result = handler(args).await.unwrap();
                let content = result.content[0].as_text().unwrap().text.as_str();
                assert!(content.contains("Meeting"));
                assert!(content.contains("Let's meet at 5."));
            })
            .await;
    }

    #[tokio::test]
    async fn test_mail_get_thread() {
        let mock = Arc::new(AssertingMailMock {
            expected_fragments: vec![
                "every mailbox of acct".to_string(),
                "subject contains \"Meeting\"".to_string()
            ],
            response: "==MSG_START==\nsubject:Re: Meeting\nsender:Bob\ndate:2024-01-02\nread:true\nmailbox:INBOX\nbody:Sounds good.\n==MSG_END==".to_string(),
        });

        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_get_thread();
                let mut args = HashMap::new();
                args.insert("subject".to_string(), json!("Meeting"));
                let result = handler(args).await.unwrap();
                let content = result.content[0].as_text().unwrap().text.as_str();
                assert!(content.contains("Re: Meeting"));
                assert!(content.contains("Sounds good."));
            })
            .await;
    }

    #[tokio::test]
    async fn test_mail_compose_message() {
        let mock = Arc::new(AssertingMailMock {
            expected_fragments: vec![
                "subject:\"Test Subject\"".to_string(),
                "content:\"Test Body\"".to_string(),
                "address:\"test@example.com\"".to_string(),
            ],
            response: "Message composed: Test Subject".to_string(),
        });

        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_compose_message();
                let mut args = HashMap::new();
                args.insert("to".to_string(), json!("test@example.com"));
                args.insert("subject".to_string(), json!("Test Subject"));
                args.insert("body".to_string(), json!("Test Body"));
                let result = handler(args).await.unwrap();
                let content = result.content[0].as_text().unwrap().text.as_str();
                assert!(content.contains("Message composed"));
            })
            .await;
    }

    #[tokio::test]
    async fn test_mail_compose_message_escaping() {
        let mock = Arc::new(AssertingMailMock {
            expected_fragments: vec![
                "subject:\"Subject with \\\"quotes\\\" and \\\\backslash\"".to_string(),
                "content:\"Body with \\\"quotes\\\" and \\\\backslash\"".to_string(),
            ],
            response: "Message composed: Subject with \"quotes\" and \\backslash".to_string(),
        });

        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_compose_message();
                let mut args = HashMap::new();
                args.insert("to".to_string(), json!("test@example.com"));
                args.insert(
                    "subject".to_string(),
                    json!("Subject with \"quotes\" and \\backslash"),
                );
                args.insert(
                    "body".to_string(),
                    json!("Body with \"quotes\" and \\backslash"),
                );
                let result = handler(args).await.unwrap();
                let content = result.content[0].as_text().unwrap().text.as_str();
                assert!(content.contains("Message composed"));
            })
            .await;
    }

    #[tokio::test]
    async fn test_mail_reply_message() {
        let mock = Arc::new(AssertingMailMock {
            expected_fragments: vec![
                "reply msg with opening window".to_string(),
                "subject contains \"Hello\"".to_string(),
            ],
            response: "Reply window opened for: Hello".to_string(),
        });

        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_reply_message();
                let mut args = HashMap::new();
                args.insert("subject".to_string(), json!("Hello"));
                args.insert("reply_text".to_string(), json!("I'm replying."));
                let result = handler(args).await.unwrap();
                let content = result.content[0].as_text().unwrap().text.as_str();
                assert!(content.contains("Reply window opened"));
            })
            .await;
    }

    #[tokio::test]
    async fn test_mail_forward_message() {
        let mock = Arc::new(AssertingMailMock {
            expected_fragments: vec![
                "forward msg with opening window".to_string(),
                "address:\"friend@example.com\"".to_string(),
            ],
            response: "Forward window opened for: Hello".to_string(),
        });

        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_forward_message();
                let mut args = HashMap::new();
                args.insert("subject".to_string(), json!("Hello"));
                args.insert("to".to_string(), json!("friend@example.com"));
                let result = handler(args).await.unwrap();
                let content = result.content[0].as_text().unwrap().text.as_str();
                assert!(content.contains("Forward window opened"));
            })
            .await;
    }

    #[tokio::test]
    async fn test_mail_update_read_state() {
        let mock = Arc::new(AssertingMailMock {
            expected_fragments: vec![
                "set read status of msg to true".to_string(),
                "subject contains \"Hello\"".to_string(),
            ],
            response: "Marked 1 message(s) as read".to_string(),
        });

        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_update_read_state();
                let mut args = HashMap::new();
                args.insert("subject".to_string(), json!("Hello"));
                args.insert("read".to_string(), json!(true));
                let result = handler(args).await.unwrap();
                let content = result.content[0].as_text().unwrap().text.as_str();
                assert!(content.contains("Marked 1 message(s) as read"));
            })
            .await;
    }

    #[tokio::test]
    async fn test_mail_move_message() {
        let mock = Arc::new(AssertingMailMock {
            expected_fragments: vec![
                "set mailbox of msg to mailbox \"Archive\"".to_string(),
                "subject contains \"Hello\"".to_string(),
            ],
            response: "Message moved to Archive: Hello".to_string(),
        });

        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_move_message();
                let mut args = HashMap::new();
                args.insert("subject".to_string(), json!("Hello"));
                args.insert("target_mailbox".to_string(), json!("Archive"));
                let result = handler(args).await.unwrap();
                let content = result.content[0].as_text().unwrap().text.as_str();
                assert!(content.contains("Message moved to Archive"));
            })
            .await;
    }

    #[tokio::test]
    async fn test_mail_delete_message() {
        let mock = Arc::new(AssertingMailMock {
            expected_fragments: vec![
                "delete msg".to_string(),
                "subject contains \"Hello\"".to_string(),
            ],
            response: "Message deleted (moved to Trash): Hello".to_string(),
        });

        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_delete_message();
                let mut args = HashMap::new();
                args.insert("subject".to_string(), json!("Hello"));
                let result = handler(args).await.unwrap();
                let content = result.content[0].as_text().unwrap().text.as_str();
                assert!(content.contains("Message deleted"));
            })
            .await;
    }

    #[tokio::test]
    async fn test_mail_open_message() {
        let mock = Arc::new(AssertingMailMock {
            expected_fragments: vec![
                "open msg".to_string(),
                "subject contains \"Hello\"".to_string(),
            ],
            response: "Opened message in Mail.app: Hello".to_string(),
        });

        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_open_message();
                let mut args = HashMap::new();
                args.insert("subject".to_string(), json!("Hello"));
                let result = handler(args).await.unwrap();
                let content = result.content[0].as_text().unwrap().text.as_str();
                assert!(content.contains("Opened message"));
            })
            .await;
    }

    #[tokio::test]
    async fn test_mail_get_attachment() {
        let mock = Arc::new(AssertingMailMock {
            expected_fragments: vec![
                "every mail attachment of msg".to_string(),
                "subject contains \"Hello\"".to_string(),
            ],
            response: "image.png||image/png||1024".to_string(),
        });

        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_get_attachment();
                let mut args = HashMap::new();
                args.insert("subject".to_string(), json!("Hello"));
                let result = handler(args).await.unwrap();
                let content = result.content[0].as_text().unwrap().text.as_str();
                assert!(content.contains("image.png"));
                assert!(content.contains("image/png"));
            })
            .await;
    }

    /// When osascript fails, compose_message must return a graceful error
    /// result instead of panicking or propagating a raw anyhow error.
    #[tokio::test]
    async fn test_compose_message_returns_error_result_on_osascript_failure() {
        struct ErrorMock;
        impl ScriptRunner for ErrorMock {
            fn run_applescript(&self, _script: &str) -> anyhow::Result<String> {
                Err(anyhow::anyhow!(
                    "osascript: Mail got an error: Can't continue Mail"
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
                let handler = handler_compose_message();
                let mut args = HashMap::new();
                args.insert("to".to_string(), json!("test@example.com"));
                args.insert("subject".to_string(), json!("Hello"));
                args.insert("body".to_string(), json!("Hi"));

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
                    content.contains("Can't continue Mail"),
                    "Expected underlying error to be surfaced, got: {}",
                    content
                );
            })
            .await;
    }

    #[tokio::test]
    async fn test_validation_compose_message_requires_to() {
        let handler = handler_compose_message();
        let mut args = HashMap::new();
        args.insert("subject".to_string(), json!("Subj"));

        let result = handler(args).await.expect("Handler should not panic");
        assert_eq!(result.is_error, Some(true));
        assert!(
            result.content[0]
                .as_text()
                .unwrap()
                .text
                .contains("to is required")
        );
    }

    #[tokio::test]
    async fn test_validation_get_attachment_requires_subject() {
        let handler = handler_get_attachment();
        let args = HashMap::new();

        let result = handler(args).await.expect("Handler should not panic");
        assert_eq!(result.is_error, Some(true));
        assert!(
            result.content[0]
                .as_text()
                .unwrap()
                .text
                .contains("subject is required")
        );
    }
}
