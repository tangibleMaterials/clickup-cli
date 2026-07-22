//! Block quote conversion: `>` lines become a `blockquote` block whose text is
//! flattened into a single `content` array.

use serde_json::json;

#[test]
fn single_line_blockquote_maps_to_blockquote() {
    assert_blocks!(
        "> a quoted line",
        json!([{ "type": "blockquote", "content": [{ "type": "text", "text": "a quoted line" }] }])
    );
}

#[test]
fn soft_wrapped_blockquote_joins_with_a_space() {
    // Consecutive `>` lines with no blank between them are one soft-wrapped
    // paragraph, so they join with a space.
    assert_blocks!(
        "> quoted\n> more",
        json!([{ "type": "blockquote", "content": [{ "type": "text", "text": "quoted more" }] }])
    );
}

#[test]
fn multi_paragraph_blockquote_joins_with_a_newline() {
    assert_blocks!(
        "> para one\n>\n> para two",
        json!([{
            "type": "blockquote",
            "content": [{ "type": "text", "text": "para one\npara two" }]
        }])
    );
}

#[test]
fn blockquote_preserves_inline_formatting() {
    assert_blocks!(
        "> a **bold** quote",
        json!([{
            "type": "blockquote",
            "content": [
                make_content_run!("a "),
                make_content_run!("bold", "bold"),
                make_content_run!(" quote"),
            ]
        }])
    );
}

#[test]
fn blockquote_after_paragraph_is_a_separate_block() {
    assert_blocks!(
        "intro line\n\n> quoted line",
        json!([
            { "type": "p", "content": [{ "type": "text", "text": "intro line" }] },
            { "type": "blockquote", "content": [{ "type": "text", "text": "quoted line" }] },
        ])
    );
}
