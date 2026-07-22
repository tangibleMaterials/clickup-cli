//! Fenced and indented code blocks: literal content (no inline parsing), one op
//! per source line, each line terminated by a `{ "code-block": true }` newline.
//! The fence language is not representable in the comment ops model and is
//! dropped.

use crate::helpers::nl_code;
use serde_json::json;

#[test]
fn fenced_code_emits_one_line_per_op() {
    assert_ops!(
        "```ruby\nputs 1\nputs 2\n```",
        json!([run!("puts 1"), nl_code(), run!("puts 2"), nl_code()])
    );
}

#[test]
fn fenced_code_without_language_is_the_same_shape() {
    assert_ops!(
        "```\nplain code\n```",
        json!([run!("plain code"), nl_code()])
    );
}

#[test]
fn language_info_string_is_ignored() {
    // Info strings like "rust,ignore" carry a language the comment ops model
    // cannot express; the content is still preserved verbatim.
    assert_ops!(
        "```rust,ignore\nfn main() {}\n```",
        json!([run!("fn main() {}"), nl_code()])
    );
}

#[test]
fn code_content_is_literal_and_not_mark_parsed() {
    // Markdown syntax inside a fence must survive verbatim as `text`.
    assert_ops!(
        "```\n**not bold** and `inline`\n```",
        json!([run!("**not bold** and `inline`"), nl_code()])
    );
}

#[test]
fn multiline_code_preserves_interior_blank_lines() {
    // A blank source line becomes a bare `code-block` newline (no text op).
    assert_ops!(
        "```python\ndef a():\n    pass\n\ndef b():\n    pass\n```",
        json!([
            run!("def a():"),
            nl_code(),
            run!("    pass"),
            nl_code(),
            nl_code(),
            run!("def b():"),
            nl_code(),
            run!("    pass"),
            nl_code(),
        ])
    );
}

#[test]
fn indented_code_block_is_code_too() {
    assert_ops!(
        "    let x = 1;\n    let y = 2;",
        json!([run!("let x = 1;"), nl_code(), run!("let y = 2;"), nl_code()])
    );
}
