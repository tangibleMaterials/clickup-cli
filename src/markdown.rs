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
//! Inline formatting (`**bold**`, `*italic*`, `` `code` ``) is parsed into a
//! `content` array of text runs with `marks`, because ClickUp does NOT render
//! raw inline markdown inside a block's `text` string — it would show the literal
//! `**`/`*`/`` ` `` characters. See [`inline_to_content`]. Blocks that carry
//! inline formatting (`p`, `h1`/`h2`/`h3`, `list_item`, `blockquote`) therefore
//! emit `content` instead of `text`; `code` blocks keep a literal `text` string.
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
            blocks.push(json!({ "type": ty, "content": inline_to_content(&text) }));
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
            blocks.push(json!({ "type": "blockquote", "content": inline_to_content(&quote.join("\n")) }));
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
        blocks.push(json!({ "type": "p", "content": inline_to_content(&paragraph.join("\n")) }));
        paragraph.clear();
    }
}

fn list_item(text: String) -> Value {
    json!({ "type": "list_item", "content": inline_to_content(&text) })
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

/// Parse a string of inline markdown into a ClickUp `content` array — an ordered
/// list of text runs, each an object `{ "type": "text", "text": "...", "marks": [...] }`.
///
/// Supported inline marks:
/// - `**bold**` / `__bold__` → `bold`
/// - `*italic*` / `_italic_` → `italic`
/// - `***bold+italic***` → `bold` + `italic`
/// - `` `code` `` → `code` (content is literal — no nested parsing)
///
/// Plain text between marks becomes a run with no `marks` key. A string with no
/// inline formatting at all still yields a single plain text run, keeping the API
/// shape consistent across every block.
pub fn inline_to_content(text: &str) -> Vec<Value> {
    let chars: Vec<char> = text.chars().collect();
    let mut runs: Vec<(String, Vec<&'static str>)> = Vec::new();
    parse_inline(&chars, &[], &mut runs);

    if runs.is_empty() {
        // Empty string (e.g. a bare heading marker) — emit one empty run so the
        // block always carries a `content` array rather than a missing field.
        return vec![json!({ "type": "text", "text": "" })];
    }

    runs.into_iter()
        .map(|(t, marks)| {
            if marks.is_empty() {
                json!({ "type": "text", "text": t })
            } else {
                let marks_json: Vec<Value> = marks.iter().map(|m| json!({ "type": m })).collect();
                json!({ "type": "text", "text": t, "marks": marks_json })
            }
        })
        .collect()
}

/// Recursive-descent scan of `chars`, appending `(text, marks)` runs to `out`.
/// `active` is the set of marks inherited from an enclosing span (so nested
/// spans accumulate their parents' marks).
fn parse_inline(chars: &[char], active: &[&'static str], out: &mut Vec<(String, Vec<&'static str>)>) {
    let mut i = 0;
    let mut buf = String::new();

    'outer: while i < chars.len() {
        // Inline code — highest precedence, literal content (no nested parsing).
        if chars[i] == '`' {
            if let Some(close) = find_char(chars, i + 1, '`') {
                flush_buf(out, &mut buf, active);
                let content: String = chars[i + 1..close].iter().collect();
                let mut marks = active.to_vec();
                push_mark(&mut marks, "code");
                out.push((content, marks));
                i = close + 1;
                continue 'outer;
            }
        }

        // Bold + italic: ***text*** (checked before ** and *).
        if matches_at(chars, i, &['*', '*', '*']) {
            if let Some(close) = find_delim(chars, i + 3, &['*', '*', '*']) {
                flush_buf(out, &mut buf, active);
                let mut marks = active.to_vec();
                push_mark(&mut marks, "bold");
                push_mark(&mut marks, "italic");
                parse_inline(&chars[i + 3..close], &marks, out);
                i = close + 3;
                continue 'outer;
            }
        }

        // Bold: **text** or __text__.
        for delim in [['*', '*'], ['_', '_']] {
            if matches_at(chars, i, &delim) {
                if let Some(close) = find_delim(chars, i + 2, &delim) {
                    flush_buf(out, &mut buf, active);
                    let mut marks = active.to_vec();
                    push_mark(&mut marks, "bold");
                    parse_inline(&chars[i + 2..close], &marks, out);
                    i = close + 2;
                    continue 'outer;
                }
            }
        }

        // Italic: *text* or _text_ (single marker, not part of a double).
        for delim in ['*', '_'] {
            if chars[i] == delim && chars.get(i + 1) != Some(&delim) {
                if let Some(close) = find_char(chars, i + 1, delim) {
                    flush_buf(out, &mut buf, active);
                    let mut marks = active.to_vec();
                    push_mark(&mut marks, "italic");
                    parse_inline(&chars[i + 1..close], &marks, out);
                    i = close + 1;
                    continue 'outer;
                }
            }
        }

        // Ordinary character — accumulate into the current plain-text run.
        buf.push(chars[i]);
        i += 1;
    }

    flush_buf(out, &mut buf, active);
}

/// Push the accumulated `buf` as a run carrying `active` marks, then clear it.
fn flush_buf(out: &mut Vec<(String, Vec<&'static str>)>, buf: &mut String, active: &[&'static str]) {
    if !buf.is_empty() {
        out.push((std::mem::take(buf), active.to_vec()));
    }
}

/// Add `mark` to `marks` unless already present (keeps mark sets deduplicated).
fn push_mark(marks: &mut Vec<&'static str>, mark: &'static str) {
    if !marks.contains(&mark) {
        marks.push(mark);
    }
}

/// True if `delim` occurs in `chars` starting exactly at index `i`.
fn matches_at(chars: &[char], i: usize, delim: &[char]) -> bool {
    i + delim.len() <= chars.len() && chars[i..i + delim.len()] == *delim
}

/// Index of the next occurrence of the multi-char `delim` at or after `from`.
fn find_delim(chars: &[char], from: usize, delim: &[char]) -> Option<usize> {
    let mut j = from;
    while j + delim.len() <= chars.len() {
        if chars[j..j + delim.len()] == *delim {
            return Some(j);
        }
        j += 1;
    }
    None
}

/// Index of the next single `marker` at or after `from`, skipping any doubled
/// occurrence (so a `*` italic scan does not stop on a `**` bold delimiter).
fn find_char(chars: &[char], from: usize, marker: char) -> Option<usize> {
    let mut j = from;
    while j < chars.len() {
        if chars[j] == marker {
            if chars.get(j + 1) == Some(&marker) {
                j += 2; // part of a double marker — skip both
                continue;
            }
            return Some(j);
        }
        j += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headings_map_to_h1_h2_h3() {
        assert_eq!(
            to_doc_blocks("# One\n## Two\n### Three"),
            vec![
                json!({ "type": "h1", "content": [{ "type": "text", "text": "One" }] }),
                json!({ "type": "h2", "content": [{ "type": "text", "text": "Two" }] }),
                json!({ "type": "h3", "content": [{ "type": "text", "text": "Three" }] }),
            ]
        );
    }

    #[test]
    fn deeper_headings_and_no_space_are_paragraphs() {
        // #### has no ClickUp equivalent; #tag has no space — both are text.
        assert_eq!(
            to_doc_blocks("#### Four\n#notaheading"),
            vec![json!({
                "type": "p",
                "content": [{ "type": "text", "text": "#### Four\n#notaheading" }]
            })]
        );
    }

    #[test]
    fn consecutive_lines_form_one_paragraph() {
        assert_eq!(
            to_doc_blocks("line one\nline two\n\nsecond para"),
            vec![
                json!({ "type": "p", "content": [{ "type": "text", "text": "line one\nline two" }] }),
                json!({ "type": "p", "content": [{ "type": "text", "text": "second para" }] }),
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
                    { "type": "list_item", "content": [{ "type": "text", "text": "one" }] },
                    { "type": "list_item", "content": [{ "type": "text", "text": "two" }] },
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
                    { "type": "list_item", "content": [{ "type": "text", "text": "first" }] },
                    { "type": "list_item", "content": [{ "type": "text", "text": "second" }] },
                    { "type": "list_item", "content": [{ "type": "text", "text": "tenth" }] },
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
            vec![json!({
                "type": "blockquote",
                "content": [{ "type": "text", "text": "quoted\nmore" }]
            })]
        );
    }

    #[test]
    fn inline_bold_produces_content_with_bold_mark() {
        // `**bold**` must become a `content` run with a bold mark, NOT a raw
        // `text` field containing literal asterisks (ClickUp does not render them).
        assert_eq!(
            to_doc_blocks("some **bold** here"),
            vec![json!({
                "type": "p",
                "content": [
                    { "type": "text", "text": "some " },
                    { "type": "text", "text": "bold", "marks": [{ "type": "bold" }] },
                    { "type": "text", "text": " here" },
                ]
            })]
        );
    }

    #[test]
    fn inline_italic_produces_italic_mark() {
        assert_eq!(
            inline_to_content("an *italic* word"),
            vec![
                json!({ "type": "text", "text": "an " }),
                json!({ "type": "text", "text": "italic", "marks": [{ "type": "italic" }] }),
                json!({ "type": "text", "text": " word" }),
            ]
        );
    }

    #[test]
    fn inline_code_produces_code_mark() {
        assert_eq!(
            inline_to_content("run `cargo test` now"),
            vec![
                json!({ "type": "text", "text": "run " }),
                json!({ "type": "text", "text": "cargo test", "marks": [{ "type": "code" }] }),
                json!({ "type": "text", "text": " now" }),
            ]
        );
    }

    #[test]
    fn inline_bold_italic_produces_both_marks() {
        assert_eq!(
            inline_to_content("***wow***"),
            vec![json!({
                "type": "text",
                "text": "wow",
                "marks": [{ "type": "bold" }, { "type": "italic" }]
            })]
        );
    }

    #[test]
    fn underscore_bold_and_italic_variants() {
        assert_eq!(
            inline_to_content("__b__ and _i_"),
            vec![
                json!({ "type": "text", "text": "b", "marks": [{ "type": "bold" }] }),
                json!({ "type": "text", "text": " and " }),
                json!({ "type": "text", "text": "i", "marks": [{ "type": "italic" }] }),
            ]
        );
    }

    #[test]
    fn plain_text_yields_single_content_run() {
        // No inline formatting → a single plain text run, no `marks` key.
        assert_eq!(
            inline_to_content("just plain text"),
            vec![json!({ "type": "text", "text": "just plain text" })]
        );
        assert_eq!(
            to_doc_blocks("just plain text"),
            vec![json!({
                "type": "p",
                "content": [{ "type": "text", "text": "just plain text" }]
            })]
        );
    }

    #[test]
    fn mixed_document_preserves_block_order() {
        let md =
            "# Plan\n\nIntro **para**.\n\n- Step *1*\n- Step 2\n\n1. First\n\n> note\n\n```js\nok()\n```";
        assert_eq!(
            to_doc_blocks(md),
            vec![
                json!({ "type": "h1", "content": [{ "type": "text", "text": "Plan" }] }),
                json!({
                    "type": "p",
                    "content": [
                        { "type": "text", "text": "Intro " },
                        { "type": "text", "text": "para", "marks": [{ "type": "bold" }] },
                        { "type": "text", "text": "." },
                    ]
                }),
                json!({
                    "type": "bullet_list",
                    "children": [
                        { "type": "list_item", "content": [
                            { "type": "text", "text": "Step " },
                            { "type": "text", "text": "1", "marks": [{ "type": "italic" }] },
                        ] },
                        { "type": "list_item", "content": [{ "type": "text", "text": "Step 2" }] },
                    ]
                }),
                json!({
                    "type": "ordered_list",
                    "children": [{ "type": "list_item", "content": [{ "type": "text", "text": "First" }] }]
                }),
                json!({ "type": "blockquote", "content": [{ "type": "text", "text": "note" }] }),
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
