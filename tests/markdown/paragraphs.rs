//! Paragraph conversion and inline formatting: bold, italic, inline code,
//! combined marks, the underscore emphasis variants, and break handling.

use serde_json::json;

#[test]
fn plain_paragraph_is_single_text_run() {
    assert_blocks!(
        "just plain text",
        json!([{ "type": "p", "content": [{ "type": "text", "text": "just plain text" }] }])
    );
}

#[test]
fn bold_text_gets_bold_mark() {
    assert_blocks!(
        "some **bold** here",
        json!([{
            "type": "p",
            "content": [
                make_content_run!("some "),
                make_content_run!("bold", "bold"),
                make_content_run!(" here"),
            ]
        }])
    );
}

#[test]
fn italic_text_gets_italic_mark() {
    assert_blocks!(
        "an *italic* word",
        json!([{
            "type": "p",
            "content": [
                make_content_run!("an "),
                make_content_run!("italic", "italic"),
                make_content_run!(" word"),
            ]
        }])
    );
}

#[test]
fn inline_code_gets_code_mark() {
    assert_blocks!(
        "run `cargo test` now",
        json!([{
            "type": "p",
            "content": [
                make_content_run!("run "),
                make_content_run!("cargo test", "code"),
                make_content_run!(" now"),
            ]
        }])
    );
}

#[test]
fn nested_bold_and_italic_gets_both_marks() {
    assert_blocks!(
        "***wow***",
        json!([{
            "type": "p",
            "content": [make_content_run!("wow", "bold", "italic")]
        }])
    );
}

#[test]
fn italic_inside_bold_marks_only_the_inner_run() {
    assert_blocks!(
        "**bold with *italic* inside**",
        json!([{
            "type": "p",
            "content": [
                make_content_run!("bold with ", "bold"),
                make_content_run!("italic", "bold", "italic"),
                make_content_run!(" inside", "bold"),
            ]
        }])
    );
}

#[test]
fn underscore_emphasis_variants_match_asterisks() {
    assert_blocks!(
        "__b__ and _i_",
        json!([{
            "type": "p",
            "content": [
                make_content_run!("b", "bold"),
                make_content_run!(" and "),
                make_content_run!("i", "italic"),
            ]
        }])
    );
}

#[test]
fn soft_wrapped_lines_join_with_a_space() {
    assert_blocks!(
        "line one\nline two",
        json!([{ "type": "p", "content": [{ "type": "text", "text": "line one line two" }] }])
    );
}

#[test]
fn hard_break_is_preserved_as_a_newline() {
    // Two trailing spaces before the newline is a CommonMark hard break.
    assert_blocks!(
        "line one  \nline two",
        json!([{ "type": "p", "content": [{ "type": "text", "text": "line one\nline two" }] }])
    );
}

#[test]
fn blank_line_splits_into_separate_paragraphs() {
    assert_blocks!(
        "first para\n\nsecond para",
        json!([
            { "type": "p", "content": [{ "type": "text", "text": "first para" }] },
            { "type": "p", "content": [{ "type": "text", "text": "second para" }] },
        ])
    );
}
