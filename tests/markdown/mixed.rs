//! Realistic, full-document round trips exercising many block types together
//! and asserting op ordering is preserved end to end.

use crate::helpers::{nl, nl_bullet, nl_code, nl_header, nl_ordered, nl_quote};
use serde_json::json;

#[test]
fn heading_paragraph_list_sequence_preserves_order() {
    let md = "# Plan\n\nIntro **para**.\n\n- Step *1*\n- Step 2";
    assert_ops!(
        md,
        json!([
            run!("Plan"),
            nl_header(1),
            run!("Intro "),
            run!("para", "bold"),
            run!("."),
            nl(),
            run!("Step "),
            run!("1", "italic"),
            nl_bullet(0),
            run!("Step 2"),
            nl_bullet(0),
        ])
    );
}

#[test]
fn code_block_between_paragraphs_keeps_its_place() {
    let md = "before\n\n```js\nok()\n```\n\nafter";
    assert_ops!(
        md,
        json!([
            run!("before"),
            nl(),
            run!("ok()"),
            nl_code(),
            run!("after"),
            nl(),
        ])
    );
}

#[test]
fn every_block_type_in_one_document() {
    let md = "# H1\n\n## H2\n\n### H3\n\nA paragraph.\n\n- bullet\n\n1. ordered\n\n> quote\n\n```\ncode\n```";
    assert_ops!(
        md,
        json!([
            run!("H1"),
            nl_header(1),
            run!("H2"),
            nl_header(2),
            run!("H3"),
            nl_header(3),
            run!("A paragraph."),
            nl(),
            run!("bullet"),
            nl_bullet(0),
            run!("ordered"),
            nl_ordered(0),
            run!("quote"),
            nl_quote(),
            run!("code"),
            nl_code(),
        ])
    );
}

#[test]
fn adjacent_headings_stay_distinct_lines() {
    assert_ops!(
        "# First\n\n# Second",
        json!([run!("First"), nl_header(1), run!("Second"), nl_header(1)])
    );
}

#[test]
fn gary_style_implementation_plan_round_trip() {
    let md = concat!(
        "# Implementation Plan: Comment Markdown Support\n",
        "\n",
        "## Summary\n",
        "\n",
        "We will replace the hand-rolled parser with **comrak** and add a *robust* test suite.\n",
        "\n",
        "## Steps\n",
        "\n",
        "1. Add the `comrak` dependency\n",
        "2. Rewrite `to_comment_ops` to walk the AST\n",
        "3. Restructure tests by block type\n",
        "\n",
        "## Notes\n",
        "\n",
        "- Bold, italic, and `inline code` must all produce marks\n",
        "- Nested lists nest via `indent`\n",
        "\n",
        "> Ship it once every test passes.\n",
        "\n",
        "```rust\n",
        "let ops = to_comment_ops(md);\n",
        "assert!(!ops.is_empty());\n",
        "```\n",
    );

    assert_ops!(
        md,
        json!([
            run!("Implementation Plan: Comment Markdown Support"),
            nl_header(1),
            run!("Summary"),
            nl_header(2),
            run!("We will replace the hand-rolled parser with "),
            run!("comrak", "bold"),
            run!(" and add a "),
            run!("robust", "italic"),
            run!(" test suite."),
            nl(),
            run!("Steps"),
            nl_header(2),
            run!("Add the "),
            run!("comrak", "code"),
            run!(" dependency"),
            nl_ordered(0),
            run!("Rewrite "),
            run!("to_comment_ops", "code"),
            run!(" to walk the AST"),
            nl_ordered(0),
            run!("Restructure tests by block type"),
            nl_ordered(0),
            run!("Notes"),
            nl_header(2),
            run!("Bold, italic, and "),
            run!("inline code", "code"),
            run!(" must all produce marks"),
            nl_bullet(0),
            run!("Nested lists nest via "),
            run!("indent", "code"),
            nl_bullet(0),
            run!("Ship it once every test passes."),
            nl_quote(),
            run!("let ops = to_comment_ops(md);"),
            nl_code(),
            run!("assert!(!ops.is_empty());"),
            nl_code(),
        ])
    );
}
