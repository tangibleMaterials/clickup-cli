//! Block quote conversion: each `>` paragraph line emits its text ops followed
//! by a `{ "blockquote": true }` newline terminator.

use crate::helpers::{nl, nl_quote};
use serde_json::json;

#[test]
fn single_line_blockquote_terminates_with_blockquote_newline() {
    assert_ops!(
        "> a quoted line",
        json!([run!("a quoted line"), nl_quote()])
    );
}

#[test]
fn soft_wrapped_blockquote_joins_with_a_space() {
    // Consecutive `>` lines with no blank between them are one soft-wrapped
    // paragraph, so they join with a space.
    assert_ops!("> quoted\n> more", json!([run!("quoted more"), nl_quote()]));
}

#[test]
fn multi_paragraph_blockquote_is_two_quoted_lines() {
    assert_ops!(
        "> para one\n>\n> para two",
        json!([run!("para one"), nl_quote(), run!("para two"), nl_quote()])
    );
}

#[test]
fn blockquote_preserves_inline_formatting() {
    assert_ops!(
        "> a **bold** quote",
        json!([run!("a "), run!("bold", "bold"), run!(" quote"), nl_quote()])
    );
}

#[test]
fn blockquote_after_paragraph_is_a_separate_line() {
    assert_ops!(
        "intro line\n\n> quoted line",
        json!([run!("intro line"), nl(), run!("quoted line"), nl_quote()])
    );
}
