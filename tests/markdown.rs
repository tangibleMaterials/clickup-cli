//! Integration test suite for the markdown → ClickUp doc block converter
//! (`clickup_cli::markdown::to_doc_blocks`).
//!
//! The suite is split by block type. Shared assertion helpers and macros live
//! in `markdown/mod.rs`; each `#[path]`-included module below owns one slice of
//! the behaviour so failures point straight at the relevant conversion rule.

#[macro_use]
#[path = "markdown/mod.rs"]
mod helpers;

#[path = "markdown/blockquotes.rs"]
mod blockquotes;
#[path = "markdown/code_blocks.rs"]
mod code_blocks;
#[path = "markdown/edge_cases.rs"]
mod edge_cases;
#[path = "markdown/headings.rs"]
mod headings;
#[path = "markdown/lists.rs"]
mod lists;
#[path = "markdown/mixed.rs"]
mod mixed;
#[path = "markdown/paragraphs.rs"]
mod paragraphs;
