//! Heading conversion: ATX (`#`) and setext (`===`/`---`) headings map to
//! ClickUp's `h1`/`h2`/`h3`; anything deeper clamps to `h3`.

use serde_json::json;

#[test]
fn atx_headings_map_to_h1_h2_h3() {
    assert_blocks!(
        "# One\n\n## Two\n\n### Three",
        json!([
            { "type": "h1", "content": [{ "type": "text", "text": "One" }] },
            { "type": "h2", "content": [{ "type": "text", "text": "Two" }] },
            { "type": "h3", "content": [{ "type": "text", "text": "Three" }] },
        ])
    );
}

#[test]
fn heading_level_four_clamps_to_h3() {
    assert_blocks!(
        "#### Four",
        json!([{ "type": "h3", "content": [{ "type": "text", "text": "Four" }] }])
    );
}

#[test]
fn heading_level_six_clamps_to_h3() {
    assert_blocks!(
        "###### Six",
        json!([{ "type": "h3", "content": [{ "type": "text", "text": "Six" }] }])
    );
}

#[test]
fn heading_with_inline_bold_gets_content_marks() {
    assert_blocks!(
        "## Plan for **launch**",
        json!([{
            "type": "h2",
            "content": [
                make_content_run!("Plan for "),
                make_content_run!("launch", "bold"),
            ]
        }])
    );
}

#[test]
fn hash_without_space_is_paragraph_not_heading() {
    // `#tag` has no space after the hash, so CommonMark treats it as text.
    assert_blocks!(
        "#notaheading",
        json!([{ "type": "p", "content": [{ "type": "text", "text": "#notaheading" }] }])
    );
}

#[test]
fn setext_headings_map_to_h1_and_h2() {
    assert_blocks!(
        "Title\n=====",
        json!([{ "type": "h1", "content": [{ "type": "text", "text": "Title" }] }])
    );
    assert_blocks!(
        "Subtitle\n--------",
        json!([{ "type": "h2", "content": [{ "type": "text", "text": "Subtitle" }] }])
    );
}
