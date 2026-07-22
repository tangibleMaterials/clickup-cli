//! Shared helpers for the markdown → comment ops test suite.
//!
//! `to_comment_ops` returns a `Vec<serde_json::Value>` (a flat Quill-delta-style
//! op stream); the helpers here wrap it as a JSON array so tests can compare
//! against `serde_json::json!([...])` literals, and the macros keep individual
//! test bodies terse and spec-like.
//!
//! Two kinds of op make up every expectation:
//!
//! - a TEXT op — `{ "text": "…" }`, plus an `attributes` map of inline marks
//!   (`bold` / `italic` / `code`). Build these with `run!`.
//! - a NEWLINE op — `{ "text": "\n" }` that terminates a line and carries its
//!   block formatting in `attributes`. Build these with the `nl_*` helpers.

#![allow(dead_code, unused_macros)]

use clickup_cli::markdown::to_comment_ops;
use serde_json::{json, Value};

/// Convert `md` and wrap the resulting ops as a JSON array `Value`, ready to
/// compare against a `json!([...])` expectation.
pub fn ops(md: &str) -> Value {
    Value::Array(to_comment_ops(md))
}

/// A plain paragraph-terminating newline op: `{ "text": "\n" }`.
pub fn nl() -> Value {
    json!({ "text": "\n" })
}

/// A heading-terminating newline op: `{ "text": "\n", "attributes": { "header": level } }`.
pub fn nl_header(level: u64) -> Value {
    json!({ "text": "\n", "attributes": { "header": level } })
}

/// A bullet-list-item-terminating newline op. `indent` is 0 for a top-level
/// item; a positive value marks a nested item.
pub fn nl_bullet(indent: u64) -> Value {
    if indent == 0 {
        json!({ "text": "\n", "attributes": { "list": "bullet" } })
    } else {
        json!({ "text": "\n", "attributes": { "list": "bullet", "indent": indent } })
    }
}

/// An ordered-list-item-terminating newline op. `indent` is 0 for a top-level
/// item; a positive value marks a nested item.
pub fn nl_ordered(indent: u64) -> Value {
    if indent == 0 {
        json!({ "text": "\n", "attributes": { "list": "ordered" } })
    } else {
        json!({ "text": "\n", "attributes": { "list": "ordered", "indent": indent } })
    }
}

/// A block-quote-line-terminating newline op.
pub fn nl_quote() -> Value {
    json!({ "text": "\n", "attributes": { "blockquote": true } })
}

/// A code-block-line-terminating newline op.
pub fn nl_code() -> Value {
    json!({ "text": "\n", "attributes": { "code-block": true } })
}

/// Assert that converting the markdown on the left produces the op array on the
/// right, printing the offending input on failure.
macro_rules! assert_ops {
    ($md:expr, $expected:expr $(,)?) => {
        assert_eq!(
            $crate::helpers::ops($md),
            $expected,
            "\n--- markdown input ---\n{}\n----------------------",
            $md
        );
    };
}

/// Build a text op, optionally with inline marks:
///
/// ```ignore
/// run!("plain");
/// run!("bold", "bold");
/// run!("both", "bold", "italic");
/// ```
macro_rules! run {
    ($text:expr) => {
        serde_json::json!({ "text": $text })
    };
    ($text:expr, $($mark:expr),+ $(,)?) => {{
        let mut attrs = serde_json::Map::new();
        $( attrs.insert($mark.to_string(), serde_json::Value::Bool(true)); )+
        serde_json::json!({ "text": $text, "attributes": attrs })
    }};
}
