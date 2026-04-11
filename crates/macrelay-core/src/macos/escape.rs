//! String escape helpers for embedding untrusted input into scripts.
//!
//! AppleScript and JXA are interpreted languages that we construct as strings
//! and pass to `osascript`. If we naively interpolate a user-provided value
//! that contains a backslash, quote, or newline, we can break the script or
//! turn it into an injection vector.
//!
//! These helpers exist so every call site uses the same correct escape rules.

/// Escape a string for embedding inside an AppleScript double-quoted string
/// literal.
///
/// AppleScript string literals are delimited by `"` and recognize two escape
/// sequences inside: `\\` for a backslash and `\"` for a double quote.
/// The backslash MUST be escaped first, otherwise a payload like `foo\"bar`
/// would be double-processed.
///
/// Example:
/// ```ignore
/// assert_eq!(escape_applescript_string(r#"he said "hi""#),
///            r#"he said \"hi\""#);
/// ```
pub fn escape_applescript_string(input: &str) -> String {
    input.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Escape a string for embedding inside a JavaScript (JXA) string literal.
///
/// Handles the characters that would otherwise terminate or corrupt a JS
/// string literal: backslash, double quote, single quote, and the three
/// common control characters (newline, carriage return, tab). Backslash is
/// always escaped first.
pub fn escape_jxa_string(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\'', "\\'")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

/// Escape a string so it is safe to embed between single quotes in a
/// `/bin/sh` command line.
///
/// Single-quoted shell strings cannot contain a single quote at all. The
/// standard trick is to close the quoted run, insert an escaped single quote
/// via `'\''`, and reopen the quoted run.
///
/// Example: `foo'bar` becomes `foo'\''bar`, and the full command is
/// `shortcuts run 'foo'\''bar'` which the shell parses as the literal
/// `foo'bar`.
pub fn escape_shell_single_quoted(input: &str) -> String {
    input.replace('\'', "'\\''")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applescript_plain_ascii_passes_through() {
        assert_eq!(escape_applescript_string("hello world"), "hello world");
        assert_eq!(escape_applescript_string(""), "");
        assert_eq!(escape_applescript_string("New Meeting"), "New Meeting");
    }

    #[test]
    fn applescript_escapes_double_quote() {
        assert_eq!(
            escape_applescript_string(r#"he said "hi""#),
            r#"he said \"hi\""#
        );
    }

    #[test]
    fn applescript_escapes_backslash() {
        assert_eq!(
            escape_applescript_string(r"C:\path\file"),
            r"C:\\path\\file"
        );
    }

    #[test]
    fn applescript_escapes_backslash_before_quote() {
        // Input: foo\"bar (backslash, then quote, then bar)
        // Broken: .replace('"', "\\\"") would leave \ alone and produce foo\\"bar,
        //         which in AppleScript would be \ then " (end of string) then bar — broken.
        // Correct: escape \ first → foo\\"bar, then escape " → foo\\\"bar.
        //          In AppleScript that parses as: \ then " then bar — the intended value.
        assert_eq!(escape_applescript_string(r#"foo\"bar"#), r#"foo\\\"bar"#);
    }

    #[test]
    fn applescript_escapes_trailing_backslash() {
        // Input: foo\ — a trailing backslash.
        // Broken: passes through as foo\, and "foo\" in AppleScript is an
        //         unterminated string literal because \" is an escaped quote.
        // Correct: foo\\ — in AppleScript, "foo\\" parses to foo\.
        assert_eq!(escape_applescript_string(r"foo\"), r"foo\\");
    }

    #[test]
    fn applescript_escapes_mixed_payload() {
        let input = r#"a\b"c\"d"#;
        let expected = r#"a\\b\"c\\\"d"#;
        assert_eq!(escape_applescript_string(input), expected);
    }

    #[test]
    fn applescript_passes_through_unicode() {
        assert_eq!(escape_applescript_string("café ☕"), "café ☕");
        assert_eq!(escape_applescript_string("日本語"), "日本語");
    }

    #[test]
    fn jxa_escapes_all_special_chars() {
        let input = "a\\b\"c'd\ne\rf\tg";
        let expected = r#"a\\b\"c\'d\ne\rf\tg"#;
        assert_eq!(escape_jxa_string(input), expected);
    }

    #[test]
    fn jxa_backslash_escaped_first() {
        // Input: \n as two characters (backslash-n), not a newline.
        // After escape: each `\` becomes `\\`, so we get \\n.
        // If we escaped \n (newline) first, a literal backslash-n would get
        // mistakenly doubled.
        assert_eq!(escape_jxa_string(r"\n"), r"\\n");
    }

    #[test]
    fn jxa_plain_ascii_passes_through() {
        assert_eq!(escape_jxa_string("hello world"), "hello world");
    }

    #[test]
    fn shell_single_quoted_plain_passes_through() {
        assert_eq!(escape_shell_single_quoted("hello world"), "hello world");
    }

    #[test]
    fn shell_single_quoted_escapes_apostrophe() {
        // Input: foo'bar → foo'\''bar
        // Wrapped: 'foo'\''bar' which the shell parses as: foo + ' + bar.
        assert_eq!(escape_shell_single_quoted("foo'bar"), r"foo'\''bar");
    }

    #[test]
    fn shell_single_quoted_multiple_apostrophes() {
        assert_eq!(
            escape_shell_single_quoted("it's 'quoted'"),
            r"it'\''s '\''quoted'\''"
        );
    }
}
