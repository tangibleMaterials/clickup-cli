//! Edge cases: empty/whitespace input, unicode, emoji, special characters,
//! escapes, and deeply nested inline marks.

use crate::helpers::nl;
use serde_json::json;

#[test]
fn empty_input_yields_no_ops() {
    assert_ops!("", json!([]));
}

#[test]
fn whitespace_only_input_yields_no_ops() {
    assert_ops!("\n\n   \n\t\n", json!([]));
}

#[test]
fn unicode_text_is_preserved() {
    assert_ops!(
        "café — naïve — 日本語",
        json!([run!("café — naïve — 日本語"), nl()])
    );
}

#[test]
fn emoji_survive_alongside_marks() {
    assert_ops!(
        "party 🎉 with **bold** 🚀",
        json!([
            run!("party 🎉 with "),
            run!("bold", "bold"),
            run!(" 🚀"),
            nl()
        ])
    );
}

#[test]
fn special_characters_stay_literal() {
    assert_ops!(
        "Costs $5 & up (50% off!) a<b",
        json!([run!("Costs $5 & up (50% off!) a<b"), nl()])
    );
}

#[test]
fn escaped_markdown_characters_render_literally() {
    assert_ops!(
        "not \\*italic\\* here",
        json!([run!("not *italic* here"), nl()])
    );
}

#[test]
fn html_entities_are_decoded() {
    assert_ops!("A &amp; B", json!([run!("A & B"), nl()]));
}

#[test]
fn deeply_nested_marks_accumulate_in_canonical_order() {
    // Inline code inside bold+italic carries all three marks. Attribute keys are
    // compared order-independently, so nesting order does not matter.
    assert_ops!(
        "***`code`***",
        json!([run!("code", "bold", "italic", "code"), nl()])
    );
}
