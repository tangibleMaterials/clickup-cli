//! List conversion: bullet lists (`-`/`*`), ordered lists (`1.`), inline
//! formatting inside items, and nested sub-lists.

use serde_json::json;

#[test]
fn dash_bullets_map_to_bullet_list() {
    assert_blocks!(
        "- one\n- two",
        json!([{
            "type": "bullet_list",
            "children": [
                { "type": "list_item", "content": [{ "type": "text", "text": "one" }] },
                { "type": "list_item", "content": [{ "type": "text", "text": "two" }] },
            ]
        }])
    );
}

#[test]
fn star_bullets_map_to_bullet_list() {
    assert_blocks!(
        "* alpha\n* beta",
        json!([{
            "type": "bullet_list",
            "children": [
                { "type": "list_item", "content": [{ "type": "text", "text": "alpha" }] },
                { "type": "list_item", "content": [{ "type": "text", "text": "beta" }] },
            ]
        }])
    );
}

#[test]
fn numbered_items_map_to_ordered_list() {
    assert_blocks!(
        "1. first\n2. second\n3. third",
        json!([{
            "type": "ordered_list",
            "children": [
                { "type": "list_item", "content": [{ "type": "text", "text": "first" }] },
                { "type": "list_item", "content": [{ "type": "text", "text": "second" }] },
                { "type": "list_item", "content": [{ "type": "text", "text": "third" }] },
            ]
        }])
    );
}

#[test]
fn list_item_inline_formatting_gets_marks() {
    assert_blocks!(
        "- plain\n- with **bold**\n- with `code`",
        json!([{
            "type": "bullet_list",
            "children": [
                { "type": "list_item", "content": [make_content_run!("plain")] },
                { "type": "list_item", "content": [
                    make_content_run!("with "),
                    make_content_run!("bold", "bold"),
                ] },
                { "type": "list_item", "content": [
                    make_content_run!("with "),
                    make_content_run!("code", "code"),
                ] },
            ]
        }])
    );
}

#[test]
fn nested_bullet_list_nests_under_parent_item_children() {
    assert_blocks!(
        "- a\n  - a1\n  - a2\n- b",
        json!([{
            "type": "bullet_list",
            "children": [
                {
                    "type": "list_item",
                    "content": [{ "type": "text", "text": "a" }],
                    "children": [{
                        "type": "bullet_list",
                        "children": [
                            { "type": "list_item", "content": [{ "type": "text", "text": "a1" }] },
                            { "type": "list_item", "content": [{ "type": "text", "text": "a2" }] },
                        ]
                    }]
                },
                { "type": "list_item", "content": [{ "type": "text", "text": "b" }] },
            ]
        }])
    );
}

#[test]
fn switching_bullet_marker_starts_a_new_list() {
    // CommonMark: changing the bullet character begins a fresh list, so this is
    // two `bullet_list` blocks rather than one merged list.
    assert_blocks!(
        "- one\n* two",
        json!([
            {
                "type": "bullet_list",
                "children": [
                    { "type": "list_item", "content": [{ "type": "text", "text": "one" }] }
                ]
            },
            {
                "type": "bullet_list",
                "children": [
                    { "type": "list_item", "content": [{ "type": "text", "text": "two" }] }
                ]
            },
        ])
    );
}
