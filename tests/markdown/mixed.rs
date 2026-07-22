//! Realistic, full-document round trips exercising many block types together
//! and asserting block ordering is preserved end to end.

use serde_json::json;

#[test]
fn heading_paragraph_list_sequence_preserves_order() {
    let md = "# Plan\n\nIntro **para**.\n\n- Step *1*\n- Step 2";
    assert_blocks!(
        md,
        json!([
            { "type": "h1", "content": [{ "type": "text", "text": "Plan" }] },
            {
                "type": "p",
                "content": [
                    make_content_run!("Intro "),
                    make_content_run!("para", "bold"),
                    make_content_run!("."),
                ]
            },
            {
                "type": "bullet_list",
                "children": [
                    { "type": "list_item", "content": [
                        make_content_run!("Step "),
                        make_content_run!("1", "italic"),
                    ] },
                    { "type": "list_item", "content": [make_content_run!("Step 2")] },
                ]
            },
        ])
    );
}

#[test]
fn code_block_between_paragraphs_keeps_its_place() {
    let md = "before\n\n```js\nok()\n```\n\nafter";
    assert_blocks!(
        md,
        json!([
            { "type": "p", "content": [{ "type": "text", "text": "before" }] },
            { "type": "code", "text": "ok()", "attrs": { "language": "js" } },
            { "type": "p", "content": [{ "type": "text", "text": "after" }] },
        ])
    );
}

#[test]
fn every_block_type_in_one_document() {
    let md = "# H1\n\n## H2\n\n### H3\n\nA paragraph.\n\n- bullet\n\n1. ordered\n\n> quote\n\n```\ncode\n```";
    assert_blocks!(
        md,
        json!([
            { "type": "h1", "content": [{ "type": "text", "text": "H1" }] },
            { "type": "h2", "content": [{ "type": "text", "text": "H2" }] },
            { "type": "h3", "content": [{ "type": "text", "text": "H3" }] },
            { "type": "p", "content": [{ "type": "text", "text": "A paragraph." }] },
            {
                "type": "bullet_list",
                "children": [{ "type": "list_item", "content": [{ "type": "text", "text": "bullet" }] }]
            },
            {
                "type": "ordered_list",
                "children": [{ "type": "list_item", "content": [{ "type": "text", "text": "ordered" }] }]
            },
            { "type": "blockquote", "content": [{ "type": "text", "text": "quote" }] },
            { "type": "code", "text": "code" },
        ])
    );
}

#[test]
fn adjacent_headings_stay_distinct_blocks() {
    assert_blocks!(
        "# First\n\n# Second",
        json!([
            { "type": "h1", "content": [{ "type": "text", "text": "First" }] },
            { "type": "h1", "content": [{ "type": "text", "text": "Second" }] },
        ])
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
        "2. Rewrite `to_doc_blocks` to walk the AST\n",
        "3. Restructure tests by block type\n",
        "\n",
        "## Notes\n",
        "\n",
        "- Bold, italic, and `inline code` must all produce marks\n",
        "- Nested lists nest under `children`\n",
        "\n",
        "> Ship it once every test passes.\n",
        "\n",
        "```rust\n",
        "let blocks = to_doc_blocks(md);\n",
        "assert!(!blocks.is_empty());\n",
        "```\n",
    );

    assert_blocks!(
        md,
        json!([
            {
                "type": "h1",
                "content": [{ "type": "text", "text": "Implementation Plan: Comment Markdown Support" }]
            },
            { "type": "h2", "content": [{ "type": "text", "text": "Summary" }] },
            {
                "type": "p",
                "content": [
                    make_content_run!("We will replace the hand-rolled parser with "),
                    make_content_run!("comrak", "bold"),
                    make_content_run!(" and add a "),
                    make_content_run!("robust", "italic"),
                    make_content_run!(" test suite."),
                ]
            },
            { "type": "h2", "content": [{ "type": "text", "text": "Steps" }] },
            {
                "type": "ordered_list",
                "children": [
                    { "type": "list_item", "content": [
                        make_content_run!("Add the "),
                        make_content_run!("comrak", "code"),
                        make_content_run!(" dependency"),
                    ] },
                    { "type": "list_item", "content": [
                        make_content_run!("Rewrite "),
                        make_content_run!("to_doc_blocks", "code"),
                        make_content_run!(" to walk the AST"),
                    ] },
                    { "type": "list_item", "content": [make_content_run!("Restructure tests by block type")] },
                ]
            },
            { "type": "h2", "content": [{ "type": "text", "text": "Notes" }] },
            {
                "type": "bullet_list",
                "children": [
                    { "type": "list_item", "content": [
                        make_content_run!("Bold, italic, and "),
                        make_content_run!("inline code", "code"),
                        make_content_run!(" must all produce marks"),
                    ] },
                    { "type": "list_item", "content": [
                        make_content_run!("Nested lists nest under "),
                        make_content_run!("children", "code"),
                    ] },
                ]
            },
            {
                "type": "blockquote",
                "content": [{ "type": "text", "text": "Ship it once every test passes." }]
            },
            {
                "type": "code",
                "text": "let blocks = to_doc_blocks(md);\nassert!(!blocks.is_empty());",
                "attrs": { "language": "rust" }
            },
        ])
    );
}
