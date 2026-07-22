//! Minimal block-level markdown → ClickUp doc block conversion.
//!
//! ClickUp's v2 comment API accepts two mutually exclusive body shapes:
//!
//! - `{ "comment_text": "..." }` — plain text, stored verbatim (markdown is
//!   NOT rendered, it appears as literal `#`, `**`, etc. in the UI).
//! - `{ "comment": [ ...blocks... ] }` — an ordered array of "doc blocks" that
//!   ClickUp renders as rich content (headings, lists, code, quotes, …).
//!
//! This module turns a markdown string into that doc block array so callers can
//! post rich comments. It is intentionally a small, dependency-free, line-based
//! parser covering the common block types — not a full CommonMark implementation.
//!
//! Inline formatting (`**bold**`, `*italic*`, `` `code` ``) is left untouched in
//! the emitted `text` strings; ClickUp renders inline markdown inside block text.
//!
//! Supported blocks:
//! - `h1` / `h2` / `h3` — `#`, `##`, `###` prefixes
//! - `code` — fenced ```` ``` ```` blocks, with an optional `language` attr
//! - `bullet_list` (`list_item` children) — `- ` / `* ` lines
//! - `ordered_list` (`list_item` children) — `1. ` lines
//! - `blockquote` — `> ` lines (consecutive lines merged)
//! - `p` — everything else (consecutive lines merged into one paragraph)

use serde_json::{json, Value};

/// Convert a markdown string into an array of ClickUp doc blocks suitable for
/// the `comment` field of the v2 comment API.
pub fn to_doc_blocks(markdown: &str) -> Vec<Value> {
    let lines: Vec<&str> = markdown.lines().collect();
    let mut blocks: Vec<Value> = Vec::new();
    let mut paragraph: Vec<String> = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i].trim_end();

        // Blank line — terminates the current paragraph (if any).
        if line.trim().is_empty() {
            flush_paragraph(&mut blocks, &mut paragraph);
            i += 1;
            continue;
        }

        // Fenced code block: ```lang ... ```
        if let Some(language) = fence_language(line) {
            flush_paragraph(&mut blocks, &mut paragraph);
            i += 1; // consume the opening fence
            let mut code_lines: Vec<&str> = Vec::new();
            while i < lines.len() && !is_fence(lines[i]) {
                code_lines.push(lines[i]);
                i += 1;
            }
            if i < lines.len() {
                i += 1; // consume the closing fence
            }
            let mut block = json!({ "type": "code", "text": code_lines.join("\n") });
            if !language.is_empty() {
                block["attrs"] = json!({ "language": language });
            }
            blocks.push(block);
            continue;
        }

        // Headings: #, ##, ### (deeper levels fall through to a paragraph).
        if let Some((level, text)) = heading(line) {
            flush_paragraph(&mut blocks, &mut paragraph);
            let ty = match level {
                1 => "h1",
                2 => "h2",
                _ => "h3",
            };
            blocks.push(json!({ "type": ty, "text": text }));
            i += 1;
            continue;
        }

        // Blockquote: one or more consecutive `>` lines merged into one block.
        if let Some(first) = blockquote_text(line) {
            flush_paragraph(&mut blocks, &mut paragraph);
            let mut quote = vec![first];
            i += 1;
            while i < lines.len() {
                match blockquote_text(lines[i].trim_end()) {
                    Some(t) => {
                        quote.push(t);
                        i += 1;
                    }
                    None => break,
                }
            }
            blocks.push(json!({ "type": "blockquote", "text": quote.join("\n") }));
            continue;
        }

        // Bullet list: consecutive `- ` / `* ` lines.
        if let Some(first) = bullet_item(line) {
            flush_paragraph(&mut blocks, &mut paragraph);
            let mut items = vec![list_item(first)];
            i += 1;
            while i < lines.len() {
                match bullet_item(lines[i].trim_end()) {
                    Some(t) => {
                        items.push(list_item(t));
                        i += 1;
                    }
                    None => break,
                }
            }
            blocks.push(json!({ "type": "bullet_list", "children": items }));
            continue;
        }

        // Ordered list: consecutive `1. ` lines.
        if let Some(first) = ordered_item(line) {
            flush_paragraph(&mut blocks, &mut paragraph);
            let mut items = vec![list_item(first)];
            i += 1;
            while i < lines.len() {
                match ordered_item(lines[i].trim_end()) {
                    Some(t) => {
                        items.push(list_item(t));
                        i += 1;
                    }
                    None => break,
                }
            }
            blocks.push(json!({ "type": "ordered_list", "children": items }));
            continue;
        }

        // Anything else: accumulate into the current paragraph.
        paragraph.push(line.trim().to_string());
        i += 1;
    }

    flush_paragraph(&mut blocks, &mut paragraph);
    blocks
}

/// Emit the accumulated paragraph lines as a single `p` block, then clear them.
fn flush_paragraph(blocks: &mut Vec<Value>, paragraph: &mut Vec<String>) {
    if !paragraph.is_empty() {
        blocks.push(json!({ "type": "p", "text": paragraph.join("\n") }));
        paragraph.clear();
    }
}

fn list_item(text: String) -> Value {
    json!({ "type": "list_item", "text": text })
}

/// True if the (trimmed-left) line opens or closes a fenced code block.
fn is_fence(line: &str) -> bool {
    line.trim_start().starts_with("```")
}

/// If `line` opens a fenced code block, return the (possibly empty) language
/// tag that follows the opening ```` ``` ````.
fn fence_language(line: &str) -> Option<String> {
    line.trim_start()
        .strip_prefix("```")
        .map(|rest| rest.trim().to_string())
}

/// Parse an ATX heading, returning `(level, text)` for levels 1–3 only.
fn heading(line: &str) -> Option<(usize, String)> {
    let t = line.trim_start();
    let hashes = t.chars().take_while(|&c| c == '#').count();
    if (1..=3).contains(&hashes) {
        let rest = &t[hashes..];
        // A real heading requires a space after the hashes (or nothing at all).
        if rest.is_empty() || rest.starts_with(' ') {
            return Some((hashes, rest.trim_start().to_string()));
        }
    }
    None
}

/// If `line` is a blockquote line (`>`), return its text with the marker removed.
fn blockquote_text(line: &str) -> Option<String> {
    line.trim_start()
        .strip_prefix('>')
        .map(|rest| rest.strip_prefix(' ').unwrap_or(rest).to_string())
}

/// If `line` is a bullet list item (`- ` or `* `), return its text.
fn bullet_item(line: &str) -> Option<String> {
    let t = line.trim_start();
    for marker in ["- ", "* "] {
        if let Some(rest) = t.strip_prefix(marker) {
            return Some(rest.trim_start().to_string());
        }
    }
    None
}

/// If `line` is an ordered list item (`1. `, `2. `, …), return its text.
fn ordered_item(line: &str) -> Option<String> {
    let t = line.trim_start();
    let digits = t.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits == 0 {
        return None;
    }
    t[digits..]
        .strip_prefix(". ")
        .map(|rest| rest.trim_start().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headings_map_to_h1_h2_h3() {
        assert_eq!(
            to_doc_blocks("# One\n## Two\n### Three"),
            vec![
                json!({ "type": "h1", "text": "One" }),
                json!({ "type": "h2", "text": "Two" }),
                json!({ "type": "h3", "text": "Three" }),
            ]
        );
    }

    #[test]
    fn deeper_headings_and_no_space_are_paragraphs() {
        // #### has no ClickUp equivalent; #tag has no space — both are text.
        assert_eq!(
            to_doc_blocks("#### Four\n#notaheading"),
            vec![json!({ "type": "p", "text": "#### Four\n#notaheading" })]
        );
    }

    #[test]
    fn consecutive_lines_form_one_paragraph() {
        assert_eq!(
            to_doc_blocks("line one\nline two\n\nsecond para"),
            vec![
                json!({ "type": "p", "text": "line one\nline two" }),
                json!({ "type": "p", "text": "second para" }),
            ]
        );
    }

    #[test]
    fn bullet_list_with_both_markers() {
        assert_eq!(
            to_doc_blocks("- one\n* two"),
            vec![json!({
                "type": "bullet_list",
                "children": [
                    { "type": "list_item", "text": "one" },
                    { "type": "list_item", "text": "two" },
                ]
            })]
        );
    }

    #[test]
    fn ordered_list() {
        assert_eq!(
            to_doc_blocks("1. first\n2. second\n10. tenth"),
            vec![json!({
                "type": "ordered_list",
                "children": [
                    { "type": "list_item", "text": "first" },
                    { "type": "list_item", "text": "second" },
                    { "type": "list_item", "text": "tenth" },
                ]
            })]
        );
    }

    #[test]
    fn fenced_code_with_language() {
        assert_eq!(
            to_doc_blocks("```ruby\nputs 1\nputs 2\n```"),
            vec![json!({
                "type": "code",
                "text": "puts 1\nputs 2",
                "attrs": { "language": "ruby" }
            })]
        );
    }

    #[test]
    fn fenced_code_without_language_omits_attrs() {
        assert_eq!(
            to_doc_blocks("```\nplain\n```"),
            vec![json!({ "type": "code", "text": "plain" })]
        );
    }

    #[test]
    fn unclosed_fence_consumes_to_end() {
        assert_eq!(
            to_doc_blocks("```\nno close"),
            vec![json!({ "type": "code", "text": "no close" })]
        );
    }

    #[test]
    fn blockquote_merges_consecutive_lines() {
        assert_eq!(
            to_doc_blocks("> quoted\n> more"),
            vec![json!({ "type": "blockquote", "text": "quoted\nmore" })]
        );
    }

    #[test]
    fn inline_formatting_is_preserved_verbatim() {
        assert_eq!(
            to_doc_blocks("some **bold** and `code` here"),
            vec![json!({ "type": "p", "text": "some **bold** and `code` here" })]
        );
    }

    #[test]
    fn mixed_document_preserves_block_order() {
        let md =
            "# Plan\n\nIntro para.\n\n- Step 1\n- Step 2\n\n1. First\n\n> note\n\n```js\nok()\n```";
        assert_eq!(
            to_doc_blocks(md),
            vec![
                json!({ "type": "h1", "text": "Plan" }),
                json!({ "type": "p", "text": "Intro para." }),
                json!({
                    "type": "bullet_list",
                    "children": [
                        { "type": "list_item", "text": "Step 1" },
                        { "type": "list_item", "text": "Step 2" },
                    ]
                }),
                json!({
                    "type": "ordered_list",
                    "children": [{ "type": "list_item", "text": "First" }]
                }),
                json!({ "type": "blockquote", "text": "note" }),
                json!({ "type": "code", "text": "ok()", "attrs": { "language": "js" } }),
            ]
        );
    }

    #[test]
    fn empty_input_yields_no_blocks() {
        assert!(to_doc_blocks("").is_empty());
        assert!(to_doc_blocks("\n\n").is_empty());
    }
}
