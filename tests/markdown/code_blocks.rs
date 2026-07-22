//! Fenced and indented code blocks: literal content (no inline parsing) and an
//! optional `attrs.language` derived from the fence info string.

use serde_json::json;

#[test]
fn fenced_code_preserves_language_attr() {
    assert_blocks!(
        "```ruby\nputs 1\nputs 2\n```",
        json!([{
            "type": "code",
            "text": "puts 1\nputs 2",
            "attrs": { "language": "ruby" }
        }])
    );
}

#[test]
fn fenced_code_without_language_omits_attrs() {
    assert_blocks!(
        "```\nplain code\n```",
        json!([{ "type": "code", "text": "plain code" }])
    );
}

#[test]
fn language_is_first_token_of_info_string() {
    // Info strings like "rust,ignore" or "js highlight" carry more than the
    // language; only the leading token is used.
    assert_blocks!(
        "```rust,ignore\nfn main() {}\n```",
        json!([{
            "type": "code",
            "text": "fn main() {}",
            "attrs": { "language": "rust" }
        }])
    );
}

#[test]
fn code_content_is_literal_and_not_mark_parsed() {
    // Markdown syntax inside a fence must survive verbatim as `text`.
    assert_blocks!(
        "```\n**not bold** and `inline`\n```",
        json!([{ "type": "code", "text": "**not bold** and `inline`" }])
    );
}

#[test]
fn multiline_code_preserves_interior_blank_lines() {
    assert_blocks!(
        "```python\ndef a():\n    pass\n\ndef b():\n    pass\n```",
        json!([{
            "type": "code",
            "text": "def a():\n    pass\n\ndef b():\n    pass",
            "attrs": { "language": "python" }
        }])
    );
}

#[test]
fn indented_code_block_maps_to_code_without_language() {
    assert_blocks!(
        "    let x = 1;\n    let y = 2;",
        json!([{ "type": "code", "text": "let x = 1;\nlet y = 2;" }])
    );
}
