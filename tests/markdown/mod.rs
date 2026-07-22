//! Shared helpers for the markdown → doc block test suite.
//!
//! `to_doc_blocks` returns a `Vec<serde_json::Value>`; the helpers here wrap it
//! as a JSON array so tests can compare against `serde_json::json!([...])`
//! literals, and the macros keep individual test bodies terse and spec-like.

#![allow(dead_code, unused_macros)]

use clickup_cli::markdown::to_doc_blocks;
use serde_json::Value;

/// Convert `md` and wrap the resulting blocks as a JSON array `Value`, ready to
/// compare against a `json!([...])` expectation.
pub fn blocks(md: &str) -> Value {
    Value::Array(to_doc_blocks(md))
}

/// Build a single text run: `{ "type": "text", "text": text }`, plus a `marks`
/// array when any marks are supplied. Mirrors what the converter emits.
pub fn content_run(text: &str, marks: &[&str]) -> Value {
    if marks.is_empty() {
        serde_json::json!({ "type": "text", "text": text })
    } else {
        let marks: Vec<Value> = marks
            .iter()
            .map(|m| serde_json::json!({ "type": m }))
            .collect();
        serde_json::json!({ "type": "text", "text": text, "marks": marks })
    }
}

/// Assert that converting the markdown on the left produces the block array on
/// the right, printing the offending input on failure.
macro_rules! assert_blocks {
    ($md:expr, $expected:expr $(,)?) => {
        assert_eq!(
            $crate::helpers::blocks($md),
            $expected,
            "\n--- markdown input ---\n{}\n----------------------",
            $md
        );
    };
}

/// Build a `{ "type": "text", "text": ... }` run, optionally with marks:
///
/// ```ignore
/// make_content_run!("plain");
/// make_content_run!("bold", "bold");
/// make_content_run!("both", "bold", "italic");
/// ```
macro_rules! make_content_run {
    ($text:expr) => {
        serde_json::json!({ "type": "text", "text": $text })
    };
    ($text:expr, $($mark:expr),+ $(,)?) => {{
        let marks: Vec<serde_json::Value> =
            vec![$(serde_json::json!({ "type": $mark })),+];
        serde_json::json!({ "type": "text", "text": $text, "marks": marks })
    }};
}
