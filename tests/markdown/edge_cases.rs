//! Edge cases: empty/whitespace input, unicode, emoji, special characters,
//! escapes, and deeply nested inline marks.

use serde_json::json;

#[test]
fn empty_input_yields_no_blocks() {
    assert_blocks!("", json!([]));
}

#[test]
fn whitespace_only_input_yields_no_blocks() {
    assert_blocks!("\n\n   \n\t\n", json!([]));
}

#[test]
fn unicode_text_is_preserved() {
    assert_blocks!(
        "café — naïve — 日本語",
        json!([{ "type": "p", "content": [{ "type": "text", "text": "café — naïve — 日本語" }] }])
    );
}

#[test]
fn emoji_survive_alongside_marks() {
    assert_blocks!(
        "party 🎉 with **bold** 🚀",
        json!([{
            "type": "p",
            "content": [
                make_content_run!("party 🎉 with "),
                make_content_run!("bold", "bold"),
                make_content_run!(" 🚀"),
            ]
        }])
    );
}

#[test]
fn special_characters_stay_literal() {
    assert_blocks!(
        "Costs $5 & up (50% off!) a<b",
        json!([{
            "type": "p",
            "content": [{ "type": "text", "text": "Costs $5 & up (50% off!) a<b" }]
        }])
    );
}

#[test]
fn escaped_markdown_characters_render_literally() {
    assert_blocks!(
        "not \\*italic\\* here",
        json!([{ "type": "p", "content": [{ "type": "text", "text": "not *italic* here" }] }])
    );
}

#[test]
fn html_entities_are_decoded() {
    assert_blocks!(
        "A &amp; B",
        json!([{ "type": "p", "content": [{ "type": "text", "text": "A & B" }] }])
    );
}

#[test]
fn deeply_nested_marks_accumulate_in_canonical_order() {
    // Inline code inside bold+italic carries all three marks, ordered
    // bold → italic → code regardless of nesting order.
    assert_blocks!(
        "***`code`***",
        json!([{
            "type": "p",
            "content": [make_content_run!("code", "bold", "italic", "code")]
        }])
    );
}
