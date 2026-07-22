//! Paragraph conversion and inline formatting: bold, italic, inline code,
//! combined marks, the underscore emphasis variants, and break handling.
//!
//! A paragraph emits its text ops followed by a plain `{ "text": "\n" }`
//! terminator (`nl()`).

use crate::helpers::nl;
use serde_json::json;

#[test]
fn plain_paragraph_is_single_text_run() {
    assert_ops!("just plain text", json!([run!("just plain text"), nl()]));
}

#[test]
fn bold_text_gets_bold_mark() {
    assert_ops!(
        "some **bold** here",
        json!([run!("some "), run!("bold", "bold"), run!(" here"), nl()])
    );
}

#[test]
fn italic_text_gets_italic_mark() {
    assert_ops!(
        "an *italic* word",
        json!([run!("an "), run!("italic", "italic"), run!(" word"), nl()])
    );
}

#[test]
fn inline_code_gets_code_mark() {
    assert_ops!(
        "run `cargo test` now",
        json!([run!("run "), run!("cargo test", "code"), run!(" now"), nl()])
    );
}

#[test]
fn nested_bold_and_italic_gets_both_marks() {
    assert_ops!("***wow***", json!([run!("wow", "bold", "italic"), nl()]));
}

#[test]
fn italic_inside_bold_marks_only_the_inner_run() {
    assert_ops!(
        "**bold with *italic* inside**",
        json!([
            run!("bold with ", "bold"),
            run!("italic", "bold", "italic"),
            run!(" inside", "bold"),
            nl(),
        ])
    );
}

#[test]
fn underscore_emphasis_variants_match_asterisks() {
    assert_ops!(
        "__b__ and _i_",
        json!([run!("b", "bold"), run!(" and "), run!("i", "italic"), nl()])
    );
}

#[test]
fn soft_wrapped_lines_join_with_a_space() {
    assert_ops!(
        "line one\nline two",
        json!([run!("line one line two"), nl()])
    );
}

#[test]
fn hard_break_is_preserved_as_a_newline() {
    // Two trailing spaces before the newline is a CommonMark hard break.
    assert_ops!(
        "line one  \nline two",
        json!([run!("line one\nline two"), nl()])
    );
}

#[test]
fn blank_line_splits_into_separate_paragraphs() {
    assert_ops!(
        "first para\n\nsecond para",
        json!([run!("first para"), nl(), run!("second para"), nl()])
    );
}
