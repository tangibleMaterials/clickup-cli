//! Markdown → ClickUp comment ops conversion, backed by the [`comrak`]
//! CommonMark parser.
//!
//! ClickUp's v2 comment API accepts two mutually exclusive body shapes:
//!
//! - `{ "comment_text": "..." }` — plain text, stored verbatim (markdown is
//!   NOT rendered, it appears as literal `#`, `**`, etc. in the UI).
//! - `{ "comment": [ ...ops... ] }` — an ordered array of "ops" (a
//!   Quill-delta-style stream) that ClickUp renders as rich content (headings,
//!   lists, code, quotes, …).
//!
//! This module turns a markdown string into that ops array so callers can post
//! rich comments.
//!
//! ## Why ops, not nested blocks
//!
//! ClickUp's comment renderer expects a FLAT stream of ops, each an object with
//! a `text` string and an optional `attributes` map — the same model Quill uses.
//! It does NOT understand a nested `{ "type": "p", "content": [...] }` tree: it
//! silently stores such a payload but never renders it, and worse, it derives
//! the `comment_text` display fallback by concatenating each top-level element's
//! `text` field. Nested blocks have no top-level `text`, so `comment_text`
//! becomes `"undefinedundefined…"` (one `undefined` per block) and the UI shows
//! that garbage instead of the content. Emitting the flat ops stream is the only
//! shape ClickUp both renders AND derives a correct `comment_text` from.
//!
//! Rather than hand-rolling a line-based parser, we hand the string to comrak,
//! which produces a typed CommonMark AST, then walk that tree and emit ops.
//!
//! ## Ops model
//!
//! Text carries INLINE formatting directly on the op:
//!
//! - Strong (`**bold**`)  → `{ "text": "…", "attributes": { "bold": true } }`
//! - Emph (`*italic*`)    → `{ "text": "…", "attributes": { "italic": true } }`
//! - Code (`` `code` ``)  → `{ "text": "…", "attributes": { "code": true } }`
//! - Nested strong+emph   → both keys on the same op
//!
//! BLOCK formatting is applied Quill-style to the newline that TERMINATES the
//! line, not to the text itself:
//!
//! | CommonMark node        | terminating-newline attributes         |
//! |------------------------|----------------------------------------|
//! | Heading level 1/2/3    | `{ "header": 1|2|3 }`                   |
//! | Heading level 4/5/6    | `{ "header": 3 }` (clamped)             |
//! | Paragraph              | (none — a plain `{ "text": "\n" }`)     |
//! | Fenced/indented code   | `{ "code-block": true }` (per line)     |
//! | Bullet list item       | `{ "list": "bullet" }` (+`indent` when nested) |
//! | Ordered list item      | `{ "list": "ordered" }` (+`indent` when nested) |
//! | Block quote line       | `{ "blockquote": true }`                |
//!
//! Unsupported blocks (thematic breaks, HTML blocks, tables, …) are skipped.
//! A `SoftBreak` becomes a space and a hard `LineBreak` becomes a newline within
//! the surrounding text op. Code fences carry their content one op per line so
//! the source is preserved verbatim; the fence language is not representable in
//! the comment ops model and is dropped.

use comrak::nodes::{AstNode, ListType, NodeValue};
use comrak::{parse_document, Arena, Options};
use serde_json::{json, Map, Value};

/// Convert a markdown string into the array of ClickUp comment ops suitable for
/// the `comment` field of the v2 comment API.
pub fn to_comment_ops(markdown: &str) -> Vec<Value> {
    let arena = Arena::new();
    let root = parse_document(&arena, markdown, &Options::default());

    let mut ops: Vec<Value> = Vec::new();
    for child in root.children() {
        emit_block(child, &mut ops);
    }
    ops
}

/// Emit the ops for a single block-level AST node, appending to `out`. Block
/// types ClickUp has no equivalent for (thematic breaks, HTML, tables…) emit
/// nothing.
fn emit_block<'a>(node: &'a AstNode<'a>, out: &mut Vec<Value>) {
    match &node.data.borrow().value {
        NodeValue::Heading(heading) => {
            emit_runs(&inline_runs(node), out);
            // ClickUp only has h1/h2/h3 — clamp deeper headings to 3.
            let level = heading.level.min(3);
            push_newline(out, [("header", json!(level))]);
        }

        NodeValue::Paragraph => {
            emit_runs(&inline_runs(node), out);
            push_newline(out, []);
        }

        NodeValue::CodeBlock(code) => {
            // comrak stores the code with a trailing newline; drop exactly one so
            // round-tripped source keeps any intentional interior blank lines.
            let literal = code.literal.strip_suffix('\n').unwrap_or(&code.literal);
            // Each source line is its own op terminated by a `code-block`
            // newline — that is how Quill (and ClickUp) mark a fenced block.
            for line in literal.split('\n') {
                push_text(out, line, &[]);
                push_newline(out, [("code-block", json!(true))]);
            }
        }

        NodeValue::List(list) => emit_list(node, list.list_type, 0, out),

        NodeValue::BlockQuote => {
            for child in node.children() {
                if matches!(child.data.borrow().value, NodeValue::Paragraph) {
                    emit_runs(&inline_runs(child), out);
                    push_newline(out, [("blockquote", json!(true))]);
                }
            }
        }

        _ => {}
    }
}

/// Emit the ops for a list, recursing into nested lists with a deeper `indent`.
/// Each item's inline text becomes text ops, terminated by a newline carrying
/// the list kind (and `indent` once nested); nested sub-lists are emitted
/// immediately after their parent item's line, mirroring Quill's flat model.
fn emit_list<'a>(node: &'a AstNode<'a>, list_type: ListType, depth: u64, out: &mut Vec<Value>) {
    let kind = match list_type {
        ListType::Bullet => "bullet",
        ListType::Ordered => "ordered",
    };

    for item in node.children() {
        if !matches!(item.data.borrow().value, NodeValue::Item(_)) {
            continue;
        }

        let mut runs: Vec<Run> = Vec::new();
        let mut nested: Vec<(&AstNode, ListType)> = Vec::new();
        let mut first_para = true;

        for child in item.children() {
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
                NodeValue::List(nested_list) => nested.push((child, nested_list.list_type)),
                _ => {}
            }
        }

        emit_runs(&runs, out);
        let mut attrs: Vec<(&str, Value)> = vec![("list", json!(kind))];
        if depth > 0 {
            attrs.push(("indent", json!(depth)));
        }
        push_newline(out, attrs);

        for (child, nested_type) in nested {
            emit_list(child, nested_type, depth + 1, out);
        }
    }
}

/// A text run: its string plus the (canonically ordered, deduplicated) marks
/// that apply to it.
type Run = (String, Vec<&'static str>);

/// Collect the inline text runs of a block node (its content, without the
/// terminating newline).
fn inline_runs<'a>(node: &'a AstNode<'a>) -> Vec<Run> {
    let mut runs: Vec<Run> = Vec::new();
    collect_children_inline(node, &[], &mut runs);
    runs
}

/// Emit one text op per run, attaching an `attributes` map for any marks.
fn emit_runs(runs: &[Run], out: &mut Vec<Value>) {
    for (text, marks) in runs {
        push_text(out, text, marks);
    }
}

/// Push a `{ "text": text, "attributes"?: {...} }` op. Empty text is dropped so
/// a run with no characters never produces a stray op.
fn push_text(out: &mut Vec<Value>, text: &str, marks: &[&'static str]) {
    if text.is_empty() {
        return;
    }
    let mut op = Map::new();
    op.insert("text".to_string(), Value::String(text.to_string()));
    if !marks.is_empty() {
        let mut attrs = Map::new();
        for mark in marks {
            attrs.insert((*mark).to_string(), Value::Bool(true));
        }
        op.insert("attributes".to_string(), Value::Object(attrs));
    }
    out.push(Value::Object(op));
}

/// Push a `{ "text": "\n", "attributes"?: {...} }` op that terminates a line and
/// carries its block-level formatting.
fn push_newline<'a, I>(out: &mut Vec<Value>, attrs: I)
where
    I: IntoIterator<Item = (&'a str, Value)>,
{
    let mut op = Map::new();
    op.insert("text".to_string(), Value::String("\n".to_string()));
    let attrs: Map<String, Value> = attrs.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
    if !attrs.is_empty() {
        op.insert("attributes".to_string(), Value::Object(attrs));
    }
    out.push(Value::Object(op));
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
