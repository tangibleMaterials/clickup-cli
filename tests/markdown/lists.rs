//! List conversion: bullet lists (`-`/`*`), ordered lists (`1.`), inline
//! formatting inside items, and nested sub-lists.
//!
//! Each item emits its text ops followed by a newline op carrying the list kind;
//! nested items add an `indent` and are emitted right after their parent line.

use crate::helpers::{nl_bullet, nl_ordered};
use serde_json::json;

#[test]
fn dash_bullets_map_to_bullet_list() {
    assert_ops!(
        "- one\n- two",
        json!([run!("one"), nl_bullet(0), run!("two"), nl_bullet(0)])
    );
}

#[test]
fn star_bullets_map_to_bullet_list() {
    assert_ops!(
        "* alpha\n* beta",
        json!([run!("alpha"), nl_bullet(0), run!("beta"), nl_bullet(0)])
    );
}

#[test]
fn numbered_items_map_to_ordered_list() {
    assert_ops!(
        "1. first\n2. second\n3. third",
        json!([
            run!("first"),
            nl_ordered(0),
            run!("second"),
            nl_ordered(0),
            run!("third"),
            nl_ordered(0),
        ])
    );
}

#[test]
fn list_item_inline_formatting_gets_marks() {
    assert_ops!(
        "- plain\n- with **bold**\n- with `code`",
        json!([
            run!("plain"),
            nl_bullet(0),
            run!("with "),
            run!("bold", "bold"),
            nl_bullet(0),
            run!("with "),
            run!("code", "code"),
            nl_bullet(0),
        ])
    );
}

#[test]
fn nested_bullet_list_nests_via_indent() {
    // The nested items are emitted immediately after their parent item's line
    // and carry `indent: 1`.
    assert_ops!(
        "- a\n  - a1\n  - a2\n- b",
        json!([
            run!("a"),
            nl_bullet(0),
            run!("a1"),
            nl_bullet(1),
            run!("a2"),
            nl_bullet(1),
            run!("b"),
            nl_bullet(0),
        ])
    );
}

#[test]
fn switching_bullet_marker_starts_a_new_list() {
    // CommonMark: changing the bullet character begins a fresh list. In the flat
    // op stream both lists read the same — two bullet items back to back.
    assert_ops!(
        "- one\n* two",
        json!([run!("one"), nl_bullet(0), run!("two"), nl_bullet(0)])
    );
}
