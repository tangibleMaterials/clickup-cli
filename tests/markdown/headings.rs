//! Heading conversion: ATX (`#`) and setext (`===`/`---`) headings terminate
//! their line with a `{ "header": 1|2|3 }` newline op; anything deeper clamps to
//! `header: 3`.

use crate::helpers::nl_header;
use serde_json::json;

#[test]
fn atx_headings_map_to_h1_h2_h3() {
    assert_ops!(
        "# One\n\n## Two\n\n### Three",
        json!([
            run!("One"),
            nl_header(1),
            run!("Two"),
            nl_header(2),
            run!("Three"),
            nl_header(3),
        ])
    );
}

#[test]
fn heading_level_four_clamps_to_h3() {
    assert_ops!("#### Four", json!([run!("Four"), nl_header(3)]));
}

#[test]
fn heading_level_six_clamps_to_h3() {
    assert_ops!("###### Six", json!([run!("Six"), nl_header(3)]));
}

#[test]
fn heading_with_inline_bold_gets_content_marks() {
    assert_ops!(
        "## Plan for **launch**",
        json!([run!("Plan for "), run!("launch", "bold"), nl_header(2)])
    );
}

#[test]
fn hash_without_space_is_paragraph_not_heading() {
    // `#tag` has no space after the hash, so CommonMark treats it as text.
    assert_ops!(
        "#notaheading",
        json!([run!("#notaheading"), crate::helpers::nl()])
    );
}

#[test]
fn setext_headings_map_to_h1_and_h2() {
    assert_ops!("Title\n=====", json!([run!("Title"), nl_header(1)]));
    assert_ops!(
        "Subtitle\n--------",
        json!([run!("Subtitle"), nl_header(2)])
    );
}
