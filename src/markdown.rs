//! Markdown → ClickUp doc block conversion, backed by the [`comrak`] CommonMark
//! parser.
//!
//! ClickUp's v2 comment API accepts two mutually exclusive body shapes:
//!
//! - `{ "comment_text": "..." }` — plain text, stored verbatim (markdown is
//!   NOT rendered, it appears as literal `#`, `**`, etc. in the UI).
//! - `{ "comment": [ ...blocks... ] }` — an ordered array of "doc blocks" that
//!   ClickUp renders as rich content (headings, lists, code, quotes, …).
//!
//! This module turns a markdown string into that doc block array so callers can
//! post rich comments.
//!
//! Rather than hand-rolling a line-based parser, we hand the string to comrak,
//! which produces a typed CommonMark AST, then walk that tree and emit blocks.
//! Using a real parser means we inherit CommonMark's handling of the awkward
//! cases (nested emphasis, lazy continuation lines, indented content, escapes,
//! entity references) for free.
//!
//! ## Block mapping
//!
//! | CommonMark node        | ClickUp block                         |
//! |------------------------|---------------------------------------|
//! | Heading level 1/2/3    | `h1` / `h2` / `h3`                    |
//! | Heading level 4/5/6    | `h3` (ClickUp has no deeper heading)  |
//! | Paragraph              | `p`                                   |
//! | Fenced/indented code   | `code` (+ `attrs.language` when known)|
//! | Bullet list            | `bullet_list` → `list_item` children  |
//! | Ordered list           | `ordered_list` → `list_item` children |
//! | Block quote            | `blockquote`                          |
//!
//! Unsupported blocks (thematic breaks, HTML blocks, tables, …) are skipped.
//!
//! ## Inline mapping
//!
//! Blocks that carry text (`p`, headings, `list_item`, `blockquote`) emit a
//! `content` array of text runs, because ClickUp does NOT render raw inline
//! markdown inside a block's `text` string — it would show the literal `**` /
//! `*` / `` ` `` characters. Each run is `{ "type": "text", "text": "…" }` with
//! an optional `marks` array:
//!
//! - Strong (`**bold**`)  → `{ "type": "bold" }`
//! - Emph (`*italic*`)    → `{ "type": "italic" }`
//! - Code (`` `code` ``)  → `{ "type": "code" }`
//! - Nested strong+emph   → both marks on the same run
//!
//! Marks are emitted in a canonical order (`bold`, `italic`, `code`) regardless
//! of nesting order, so output is stable. A `SoftBreak` becomes a space and a
//! hard `LineBreak` becomes a newline within the surrounding text run.
//!
//! `code` blocks are the exception: they carry a literal `text` string (no mark
//! parsing) so the source is preserved byte-for-byte.

use comrak::nodes::{AstNode, ListType, NodeValue};
use comrak::{parse_document, Arena, Options};
use serde_json::{json, Value};

/// Convert a markdown string into an array of ClickUp doc blocks suitable for
/// the `comment` field of the v2 comment API.
pub fn to_doc_blocks(markdown: &str) -> Vec<Value> {
    let arena = Arena::new();
    let root = parse_document(&arena, markdown, &Options::default());

    root.children().filter_map(block_to_value).collect()
}

/// Convert a single block-level AST node into a ClickUp doc block, or `None` for
/// block types ClickUp has no equivalent for (thematic breaks, HTML, tables…).
fn block_to_value<'a>(node: &'a AstNode<'a>) -> Option<Value> {
    match &node.data.borrow().value {
        NodeValue::Heading(heading) => {
            let ty = match heading.level {
                1 => "h1",
                2 => "h2",
                // ClickUp only has h1/h2/h3 — clamp deeper headings to h3.
                _ => "h3",
            };
            Some(json!({ "type": ty, "content": inline_content(node) }))
        }

        NodeValue::Paragraph => Some(json!({ "type": "p", "content": inline_content(node) })),

        NodeValue::CodeBlock(code) => {
            // comrak stores the code with a trailing newline; drop exactly one so
            // round-tripped source keeps any intentional interior blank lines.
            let literal = code.literal.strip_suffix('\n').unwrap_or(&code.literal);
            let mut block = json!({ "type": "code", "text": literal });
            // The info string may be "rust", "rust,ignore", "ruby foo" — the
            // language is its first whitespace/comma-free token.
            let language = code
                .info
                .split(|c: char| c.is_whitespace() || c == ',')
                .find(|s| !s.is_empty())
                .unwrap_or("");
            if !language.is_empty() {
                block["attrs"] = json!({ "language": language });
            }
            Some(block)
        }

        NodeValue::List(list) => {
            let ty = match list.list_type {
                ListType::Bullet => "bullet_list",
                ListType::Ordered => "ordered_list",
            };
            let children: Vec<Value> = node.children().filter_map(list_item_to_value).collect();
            Some(json!({ "type": ty, "children": children }))
        }

        NodeValue::BlockQuote => {
            Some(json!({ "type": "blockquote", "content": blockquote_content(node) }))
        }

        _ => None,
    }
}

/// Convert a list `Item` node into a `list_item` block. The item's inline text
/// becomes its `content`; any nested lists are attached under `children` so the
/// list hierarchy is preserved.
fn list_item_to_value<'a>(node: &'a AstNode<'a>) -> Option<Value> {
    if !matches!(node.data.borrow().value, NodeValue::Item(_)) {
        return None;
    }

    let mut runs: Vec<Run> = Vec::new();
    let mut nested: Vec<Value> = Vec::new();
    let mut first_para = true;

    for child in node.children() {
        match &child.data.borrow().value {
            NodeValue::Paragraph => {
                // A multi-paragraph item is rare; separate paragraphs with a
                // newline so their text does not run together.
                if !first_para {
                    push_run(&mut runs, "\n".to_string(), &[]);
                }
                first_para = false;
                collect_children_inline(child, &[], &mut runs);
            }
            NodeValue::List(_) => {
                if let Some(block) = block_to_value(child) {
                    nested.push(block);
                }
            }
            _ => {}
        }
    }

    let mut item = json!({ "type": "list_item", "content": runs_to_content(runs) });
    if !nested.is_empty() {
        item["children"] = json!(nested);
    }
    Some(item)
}

/// Flatten a block quote's child paragraphs into one `content` array, joining
/// separate paragraphs with a newline.
fn blockquote_content<'a>(node: &'a AstNode<'a>) -> Vec<Value> {
    let mut runs: Vec<Run> = Vec::new();
    let mut first_para = true;

    for child in node.children() {
        if matches!(child.data.borrow().value, NodeValue::Paragraph) {
            if !first_para {
                push_run(&mut runs, "\n".to_string(), &[]);
            }
            first_para = false;
            collect_children_inline(child, &[], &mut runs);
        }
    }

    runs_to_content(runs)
}

/// A text run: its string plus the (canonically ordered, deduplicated) marks
/// that apply to it.
type Run = (String, Vec<&'static str>);

/// Build the `content` array for a block by walking its inline children.
fn inline_content<'a>(node: &'a AstNode<'a>) -> Vec<Value> {
    let mut runs: Vec<Run> = Vec::new();
    collect_children_inline(node, &[], &mut runs);
    runs_to_content(runs)
}

/// Walk every inline child of `node`, appending runs to `out`.
fn collect_children_inline<'a>(node: &'a AstNode<'a>, active: &[&'static str], out: &mut Vec<Run>) {
    for child in node.children() {
        collect_inline(child, active, out);
    }
}

/// Recursively convert an inline node into text runs. `active` is the set of
/// marks inherited from enclosing spans, so nested emphasis accumulates.
fn collect_inline<'a>(node: &'a AstNode<'a>, active: &[&'static str], out: &mut Vec<Run>) {
    match &node.data.borrow().value {
        NodeValue::Text(text) => push_run(out, text.to_string(), active),
        NodeValue::SoftBreak => push_run(out, " ".to_string(), active),
        NodeValue::LineBreak => push_run(out, "\n".to_string(), active),
        NodeValue::Code(code) => {
            let marks = add_mark(active, "code");
            push_run(out, code.literal.clone(), &marks);
        }
        NodeValue::Strong => {
            let marks = add_mark(active, "bold");
            collect_children_inline(node, &marks, out);
        }
        NodeValue::Emph => {
            let marks = add_mark(active, "italic");
            collect_children_inline(node, &marks, out);
        }
        // Links/images and any other inline container: keep the current marks
        // and recurse so their visible text still lands in the output.
        _ => collect_children_inline(node, active, out),
    }
}

/// Append `text` to `out`, coalescing with the previous run when it carries the
/// exact same marks. Empty strings are dropped.
fn push_run(out: &mut Vec<Run>, text: String, marks: &[&'static str]) {
    if text.is_empty() {
        return;
    }
    if let Some(last) = out.last_mut() {
        if last.1 == marks {
            last.0.push_str(&text);
            return;
        }
    }
    out.push((text, marks.to_vec()));
}

/// Return `active` with `mark` added, kept deduplicated and in canonical order.
fn add_mark(active: &[&'static str], mark: &'static str) -> Vec<&'static str> {
    let mut marks = active.to_vec();
    if !marks.contains(&mark) {
        marks.push(mark);
    }
    marks.sort_by_key(|m| mark_priority(m));
    marks
}

/// Canonical ordering for marks so output is independent of nesting order.
fn mark_priority(mark: &str) -> u8 {
    match mark {
        "bold" => 0,
        "italic" => 1,
        "code" => 2,
        _ => 3,
    }
}

/// Turn accumulated runs into the JSON `content` array. An empty run list still
/// yields one empty text run so every block reliably carries a `content` array.
fn runs_to_content(runs: Vec<Run>) -> Vec<Value> {
    if runs.is_empty() {
        return vec![json!({ "type": "text", "text": "" })];
    }

    runs.into_iter()
        .map(|(text, marks)| {
            if marks.is_empty() {
                json!({ "type": "text", "text": text })
            } else {
                let marks_json: Vec<Value> = marks.iter().map(|m| json!({ "type": m })).collect();
                json!({ "type": "text", "text": text, "marks": marks_json })
            }
        })
        .collect()
}
