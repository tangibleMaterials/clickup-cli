# MCP Tool Filtering Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `--profile`, `--read-only`, `--groups`, `--tools`, and exclude variants to `clickup mcp serve` (plus matching env vars), so the MCP server exposes only a filtered subset of its 143 tools. Default remains "all 143 tools."

**Architecture:** Two new private modules under `src/mcp/`: `classify.rs` (pure function mapping tool name → `(Class, group)`) and `filter.rs` (parses CLI/env into a `Filter` and applies it to the tool list). `src/mcp.rs` threads the `Filter` through `tool_list()`, `serve()`, and `call_tool` — filtering both what the LLM is told exists (`tools/list`) and what the server will actually execute (`tools/call` rejects filtered tools with JSON-RPC `-32601`).

**Tech Stack:** Rust 2021, clap v4 (derive API), serde_json, tokio, std env. No new dependencies.

**Spec:** `docs/superpowers/specs/2026-04-22-mcp-tool-filtering-design.md`

---

## File Structure

**New:**
- `src/mcp/classify.rs` — `Class` enum, `ToolMeta` struct, `classify()` function, verb sets, override table.
- `src/mcp/filter.rs` — `Profile` enum, `Filter` struct, CLI/env parsing, pipeline application, error types.
- `tests/test_mcp_filter.rs` — classification self-check (all 143 tools), profile coverage, pipeline behavior, error cases, `call_tool` rejection.

**Modified:**
- `src/mcp.rs` — declare `pub mod classify; pub mod filter;`. Change `tool_list()` to `pub(crate) fn tool_list(filter: &Filter) -> Value`. Change `serve()` signature to `pub async fn serve(filter: Filter) -> Result<(), Box<dyn Error>>`. Add filter enforcement to the `tools/call` branch.
- `src/commands/mcp_cmd.rs` — add flags to the `Serve` variant, build `Filter` in `execute`, pass to `serve()`.
- `src/commands/agent_config.rs` — append MCP serve filter hint to `AGENT_REFERENCE`.
- `README.md` — new "Limiting MCP tools" subsection.
- `docs/mcp.md` — mirror README content with a profile table.
- `CLAUDE.md` — one-line mention of filter flags under "MCP Server".
- `CHANGELOG.md` — convert `[Unreleased]` to `[0.8.0]`, add `Added` block, new empty `[Unreleased]`, update compare links.
- `Cargo.toml` — version `0.7.0` → `0.8.0`.
- `npm/package.json` — version `0.7.0` → `0.8.0`.
- `Cargo.lock` — regenerates from `cargo build`.

---

## Key Algorithm: Classification

`classify(tool_name: &str) -> Option<ToolMeta>` applies these steps in order:

1. **Override table lookup.** 10 hand-curated entries for tools that don't fit the convention.
2. **Group extraction.** Strip `clickup_` prefix, then match the longest known group prefix (two-word groups `task_type` and `audit_log` come before one-word groups).
3. **Segment the remainder** on `_`.
4. **Destructive wins anywhere.** If any segment is in the destructive verb set → `Destructive`.
5. **Trailing-verb check.** If the last segment is in the write set → `Write`. Else if in the read set → `Read`.
6. **Write scan.** If no trailing match but any segment is in the write set → `Write`.
7. **Fail.** Return `None` — the test harness will catch this at CI time and force either a new override or a fix to the verb sets.

**Why this order:** steps 4 and 5 handle the common cases (`task_delete`, `task_list`, `task_create`). Step 6 handles compound-verb names like `task_add_dep`, `goal_add_kr`, `checklist_delete_item`, `list_add_task`. The separation of "last segment" from "any segment" avoids misclassifying `chat_reply_list` (a read operation whose second-to-last segment is the write verb "reply") — last=`list` is read, we stop before the write scan.

---

## Task 1: Create `classify` module scaffold

**Files:**
- Create: `src/mcp/classify.rs`
- Modify: `src/mcp.rs:1-6` (add `pub mod classify;`)

- [ ] **Step 1: Create `src/mcp/classify.rs` with types and verb sets**

```rust
//! Classification of MCP tool names into (Class, group).
//!
//! `classify()` is a pure function. The self-check test in
//! `tests/test_mcp_filter.rs` asserts that every tool in `tool_list()`
//! classifies without falling through to `None`.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    Read,
    Write,
    Destructive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolMeta {
    pub class: Class,
    pub group: &'static str,
}

/// Known resource groups. Two-word groups come first so prefix matching
/// in `classify()` prefers them over their one-word prefixes.
const KNOWN_GROUPS: &[(&str, &str)] = &[
    ("task_type", "task-type"),
    ("audit_log", "audit-log"),
    ("auth", "auth"),
    ("workspace", "workspace"),
    ("space", "space"),
    ("folder", "folder"),
    ("list", "list"),
    ("task", "task"),
    ("checklist", "checklist"),
    ("comment", "comment"),
    ("tag", "tag"),
    ("field", "field"),
    ("attachment", "attachment"),
    ("time", "time"),
    ("goal", "goal"),
    ("view", "view"),
    ("member", "member"),
    ("user", "user"),
    ("chat", "chat"),
    ("doc", "doc"),
    ("webhook", "webhook"),
    ("template", "template"),
    ("guest", "guest"),
    ("group", "group"),
    ("role", "role"),
    ("shared", "shared"),
    ("acl", "acl"),
];

const READ_VERBS: &[&str] = &[
    "list", "get", "search", "current", "pages", "followers", "members",
    "history", "whoami", "check", "replies", "tagged", "query",
];

const WRITE_VERBS: &[&str] = &[
    "create", "update", "set", "add", "start", "stop", "move", "apply",
    "invite", "rename", "share", "attach", "link", "reply", "send", "dm",
    "edit", "upload",
];

const DESTRUCTIVE_VERBS: &[&str] = &[
    "delete", "remove", "unshare", "unlink", "unset",
];

/// Tools that don't fit the naming convention. Each entry shortcircuits
/// the auto-deriver.
const OVERRIDES: &[(&str, Class, &str)] = &[
    ("clickup_search",                 Class::Read,  "workspace"),
    ("clickup_whoami",                 Class::Read,  "auth"),
    ("clickup_workspace_plan",         Class::Read,  "workspace"),
    ("clickup_workspace_seats",        Class::Read,  "workspace"),
    ("clickup_task_replace_estimates", Class::Write, "task"),
    ("clickup_task_time_in_status",    Class::Read,  "task"),
    ("clickup_time_tags",              Class::Read,  "time"),
    ("clickup_template_apply_list",    Class::Write, "template"),
    ("clickup_doc_page",               Class::Read,  "doc"),
    ("clickup_chat_tagged_users",      Class::Read,  "chat"),
];

pub fn classify(tool_name: &str) -> Option<ToolMeta> {
    // Step 1: override table
    if let Some(&(_, class, group)) = OVERRIDES.iter().find(|(n, _, _)| *n == tool_name) {
        return Some(ToolMeta { class, group });
    }

    // Step 2: group prefix (longest match wins because two-word entries come first)
    let rest = tool_name.strip_prefix("clickup_")?;
    let (raw_prefix, normalized_group) = KNOWN_GROUPS
        .iter()
        .find(|(prefix, _)| rest == *prefix || rest.starts_with(&format!("{}_", prefix)))
        .copied()?;
    let remainder = rest
        .strip_prefix(raw_prefix)
        .and_then(|r| r.strip_prefix('_'))
        .unwrap_or("");

    if remainder.is_empty() {
        return None;
    }

    let segments: Vec<&str> = remainder.split('_').collect();
    let last = *segments.last().unwrap();

    // Step 4: destructive anywhere
    if segments.iter().any(|s| DESTRUCTIVE_VERBS.contains(s)) {
        return Some(ToolMeta { class: Class::Destructive, group: normalized_group });
    }

    // Step 5: trailing verb
    if WRITE_VERBS.contains(&last) {
        return Some(ToolMeta { class: Class::Write, group: normalized_group });
    }
    if READ_VERBS.contains(&last) {
        return Some(ToolMeta { class: Class::Read, group: normalized_group });
    }

    // Step 6: any write segment
    if segments.iter().any(|s| WRITE_VERBS.contains(s)) {
        return Some(ToolMeta { class: Class::Write, group: normalized_group });
    }

    None
}

pub const ALL_GROUPS: &[&str] = &[
    "auth", "workspace", "space", "folder", "list", "task", "checklist",
    "comment", "tag", "field", "task-type", "attachment", "time", "goal",
    "view", "member", "user", "chat", "doc", "webhook", "template",
    "guest", "group", "role", "shared", "audit-log", "acl",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn destructive_wins_over_write_in_same_name() {
        assert_eq!(classify("clickup_task_remove_tag").unwrap().class, Class::Destructive);
    }

    #[test]
    fn trailing_read_beats_earlier_write() {
        // reply (write verb) appears before list (read verb); trailing wins
        assert_eq!(classify("clickup_chat_reply_list").unwrap().class, Class::Read);
    }

    #[test]
    fn write_scan_catches_compound_verbs() {
        assert_eq!(classify("clickup_goal_add_kr").unwrap().class, Class::Write);
        assert_eq!(classify("clickup_task_add_dep").unwrap().class, Class::Write);
    }

    #[test]
    fn two_word_group_prefix_wins() {
        let m = classify("clickup_task_type_list").unwrap();
        assert_eq!(m.group, "task-type");
        assert_eq!(m.class, Class::Read);
    }

    #[test]
    fn override_table_short_circuits() {
        assert_eq!(classify("clickup_task_replace_estimates").unwrap().class, Class::Write);
        assert_eq!(classify("clickup_search").unwrap().group, "workspace");
    }

    #[test]
    fn unknown_tool_returns_none() {
        assert!(classify("clickup_not_a_real_tool").is_none());
    }
}
```

- [ ] **Step 2: Declare the submodule in `src/mcp.rs`**

At the top of `src/mcp.rs`, after the existing `use` statements (around line 5), add:

```rust
pub mod classify;
pub mod filter;
```

(Declaring `filter` here too so the next task can create it without editing this file again.)

- [ ] **Step 3: Run the classify module's unit tests**

Run: `cargo test --lib mcp::classify`
Expected: the 6 unit tests in `classify.rs` pass. Build will fail on the missing `src/mcp/filter.rs` — we'll create that in Task 3. To unblock temporarily, also create an empty stub:

Create `src/mcp/filter.rs` with exactly:

```rust
// Stub — implementation lands in Task 3.
```

Then rerun `cargo test --lib mcp::classify`. Expected: `test result: ok. 6 passed`.

- [ ] **Step 4: Commit**

```bash
git add src/mcp.rs src/mcp/classify.rs src/mcp/filter.rs
git commit -m "feat(mcp): add tool classifier (class + group)"
```

---

## Task 2: Classification self-check test across all 143 tools

**Files:**
- Create: `tests/test_mcp_filter.rs`
- Modify: `src/mcp.rs` — make `tool_list()` visible to integration tests.

- [ ] **Step 1: Expose `tool_list()`**

In `src/mcp.rs`, change:

```rust
fn tool_list() -> Value {
```

to:

```rust
pub fn tool_list() -> Value {
```

(The function currently takes no arguments; it will grow a `&Filter` parameter in Task 5. Keep it argumentless for now.)

- [ ] **Step 2: Write the self-check integration test**

Create `tests/test_mcp_filter.rs`:

```rust
//! Integration tests for MCP tool classification and filtering.

use clickup_cli::mcp::classify::{classify, ALL_GROUPS};
use clickup_cli::mcp::tool_list;

#[test]
fn every_tool_classifies() {
    let tools = tool_list();
    let array = tools.as_array().expect("tool_list must return a JSON array");
    assert!(!array.is_empty(), "tool_list is empty");

    let mut unclassified: Vec<String> = Vec::new();
    let mut unknown_group: Vec<(String, String)> = Vec::new();

    for tool in array {
        let name = tool
            .get("name")
            .and_then(|v| v.as_str())
            .expect("each tool must have a string `name`");

        match classify(name) {
            None => unclassified.push(name.to_string()),
            Some(meta) => {
                if !ALL_GROUPS.contains(&meta.group) {
                    unknown_group.push((name.to_string(), meta.group.to_string()));
                }
            }
        }
    }

    assert!(
        unclassified.is_empty(),
        "unclassified tools (add to OVERRIDES or extend verb sets): {:?}",
        unclassified
    );
    assert!(
        unknown_group.is_empty(),
        "tools mapped to unknown groups: {:?}",
        unknown_group
    );
}

#[test]
fn expected_tool_count() {
    // Sanity check: we don't want a future refactor to silently drop tools.
    let tools = tool_list();
    let array = tools.as_array().unwrap();
    assert_eq!(array.len(), 143, "tool count changed; update this test");
}
```

- [ ] **Step 3: Run the test**

Run: `cargo test --test test_mcp_filter`
Expected: both tests **pass**. If any tool fails to classify, the test output lists them so you know exactly what to add to `OVERRIDES` or the verb sets in `classify.rs`. If the count assertion fails, someone changed `tool_list()` — reconcile before proceeding.

- [ ] **Step 4: Commit**

```bash
git add src/mcp.rs tests/test_mcp_filter.rs
git commit -m "test(mcp): assert all 143 tools classify"
```

---

## Task 3: `Filter` struct, `Profile` enum, pipeline

**Files:**
- Modify: `src/mcp/filter.rs` (replace the stub)
- Test: `tests/test_mcp_filter.rs` (append)

- [ ] **Step 1: Write failing tests for the filter pipeline**

Append to `tests/test_mcp_filter.rs`:

```rust
use clickup_cli::mcp::filter::{Filter, FilterError, Profile, RawFilter};

fn tool_names_in(filter: &Filter) -> Vec<String> {
    let tools = tool_list();
    let array = tools.as_array().unwrap();
    array
        .iter()
        .filter_map(|t| t.get("name").and_then(|v| v.as_str()).map(str::to_string))
        .filter(|n| filter.allows(n))
        .collect()
}

#[test]
fn default_exposes_all_tools() {
    let filter = Filter::resolve(RawFilter::default()).unwrap();
    assert_eq!(filter.profile, Profile::All);
    assert_eq!(tool_names_in(&filter).len(), 143);
}

#[test]
fn read_profile_excludes_writes_and_destructives() {
    let raw = RawFilter { profile: Some("read".into()), ..RawFilter::default() };
    let filter = Filter::resolve(raw).unwrap();
    let names = tool_names_in(&filter);
    assert!(names.iter().all(|n| {
        use clickup_cli::mcp::classify::{classify, Class};
        classify(n).map(|m| m.class == Class::Read).unwrap_or(false)
    }));
    assert!(names.contains(&"clickup_task_list".to_string()));
    assert!(!names.contains(&"clickup_task_delete".to_string()));
    assert!(!names.contains(&"clickup_task_create".to_string()));
}

#[test]
fn safe_profile_excludes_destructives_only() {
    let raw = RawFilter { profile: Some("safe".into()), ..RawFilter::default() };
    let filter = Filter::resolve(raw).unwrap();
    let names = tool_names_in(&filter);
    assert!(names.contains(&"clickup_task_create".to_string()));
    assert!(!names.contains(&"clickup_task_delete".to_string()));
    assert!(!names.contains(&"clickup_list_remove_task".to_string()));
}

#[test]
fn read_only_flag_equivalent_to_profile_read() {
    let raw = RawFilter { read_only: true, ..RawFilter::default() };
    let filter = Filter::resolve(raw).unwrap();
    assert_eq!(filter.profile, Profile::Read);
}

#[test]
fn groups_filter_restricts_to_listed_groups() {
    let raw = RawFilter {
        groups: Some(vec!["task".into(), "comment".into()]),
        ..RawFilter::default()
    };
    let filter = Filter::resolve(raw).unwrap();
    let names = tool_names_in(&filter);
    assert!(names.iter().all(|n| n.starts_with("clickup_task_")
        || n.starts_with("clickup_comment_")
        || n == "clickup_task_type_list" /* group "task-type" is NOT "task" */ == false));
    assert!(names.contains(&"clickup_task_get".to_string()));
    assert!(names.contains(&"clickup_comment_list".to_string()));
    assert!(!names.contains(&"clickup_chat_channel_list".to_string()));
}

#[test]
fn exclude_groups_drops_listed_groups() {
    let raw = RawFilter {
        exclude_groups: Some(vec!["chat".into(), "audit-log".into()]),
        ..RawFilter::default()
    };
    let filter = Filter::resolve(raw).unwrap();
    let names = tool_names_in(&filter);
    assert!(!names.iter().any(|n| n.starts_with("clickup_chat_")));
    assert!(!names.iter().any(|n| n.starts_with("clickup_audit_log_")));
    assert!(names.contains(&"clickup_task_list".to_string()));
}

#[test]
fn tools_filter_intersects_with_profile() {
    let raw = RawFilter {
        profile: Some("read".into()),
        tools: Some(vec!["clickup_task_get".into(), "clickup_task_list".into()]),
        ..RawFilter::default()
    };
    let filter = Filter::resolve(raw).unwrap();
    let names = tool_names_in(&filter);
    assert_eq!(names.len(), 2);
}

#[test]
fn tool_excluded_by_profile_errors() {
    let raw = RawFilter {
        profile: Some("read".into()),
        tools: Some(vec!["clickup_task_delete".into()]),
        ..RawFilter::default()
    };
    let err = Filter::resolve(raw).unwrap_err();
    assert!(matches!(err, FilterError::ToolExcludedByProfile { .. }));
}

#[test]
fn exclude_tools_drops_them() {
    let raw = RawFilter {
        exclude_tools: Some(vec!["clickup_task_delete".into()]),
        ..RawFilter::default()
    };
    let filter = Filter::resolve(raw).unwrap();
    let names = tool_names_in(&filter);
    assert!(!names.contains(&"clickup_task_delete".to_string()));
    assert!(names.contains(&"clickup_task_create".to_string()));
}

#[test]
fn empty_final_set_errors() {
    let raw = RawFilter {
        groups: Some(vec!["task".into()]),
        exclude_groups: Some(vec!["task".into()]),
        ..RawFilter::default()
    };
    let err = Filter::resolve(raw).unwrap_err();
    assert!(matches!(err, FilterError::EmptyFilter));
}

#[test]
fn read_only_plus_non_read_profile_errors() {
    let raw = RawFilter {
        profile: Some("safe".into()),
        read_only: true,
        ..RawFilter::default()
    };
    let err = Filter::resolve(raw).unwrap_err();
    assert!(matches!(err, FilterError::ConflictingProfile { .. }));
}

#[test]
fn unknown_profile_errors() {
    let raw = RawFilter { profile: Some("gibberish".into()), ..RawFilter::default() };
    let err = Filter::resolve(raw).unwrap_err();
    assert!(matches!(err, FilterError::UnknownProfile { .. }));
}

#[test]
fn unknown_group_errors() {
    let raw = RawFilter { groups: Some(vec!["nope".into()]), ..RawFilter::default() };
    let err = Filter::resolve(raw).unwrap_err();
    assert!(matches!(err, FilterError::UnknownGroup { .. }));
}

#[test]
fn unknown_tool_errors_with_hint() {
    let raw = RawFilter {
        tools: Some(vec!["clickup_task_lst".into()]),
        ..RawFilter::default()
    };
    let err = Filter::resolve(raw).unwrap_err();
    match err {
        FilterError::UnknownTool { name, suggestion } => {
            assert_eq!(name, "clickup_task_lst");
            assert_eq!(suggestion.as_deref(), Some("clickup_task_list"));
        }
        other => panic!("expected UnknownTool, got {:?}", other),
    }
}
```

- [ ] **Step 2: Run tests, confirm they fail**

Run: `cargo test --test test_mcp_filter`
Expected: compile errors (`Filter`, `FilterError`, `Profile`, `RawFilter` not found). This is the red state.

- [ ] **Step 3: Implement `src/mcp/filter.rs`**

Replace the stub with:

```rust
//! Runtime filter for the MCP tool list.
//!
//! `RawFilter` holds unparsed CLI/env values. `Filter::resolve` normalizes,
//! validates, and applies the filter pipeline, returning either a `Filter`
//! whose `allows()` is the tool-name gate used by `tool_list()` and
//! `call_tool`, or a `FilterError` to surface at startup.

use std::collections::HashSet;

use crate::mcp::classify::{classify, Class, ToolMeta, ALL_GROUPS};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    All,
    Read,
    Safe,
}

impl Profile {
    fn parse(s: &str) -> Option<Self> {
        match s {
            "all" => Some(Profile::All),
            "read" => Some(Profile::Read),
            "safe" => Some(Profile::Safe),
            _ => None,
        }
    }

    fn allows_class(self, class: Class) -> bool {
        match (self, class) {
            (Profile::All, _) => true,
            (Profile::Read, Class::Read) => true,
            (Profile::Read, _) => false,
            (Profile::Safe, Class::Destructive) => false,
            (Profile::Safe, _) => true,
        }
    }
}

/// Raw filter inputs before validation.
#[derive(Debug, Default, Clone)]
pub struct RawFilter {
    pub profile: Option<String>,
    pub read_only: bool,
    pub groups: Option<Vec<String>>,
    pub exclude_groups: Option<Vec<String>>,
    pub tools: Option<Vec<String>>,
    pub exclude_tools: Option<Vec<String>>,
}

#[derive(Debug)]
pub enum FilterError {
    UnknownProfile { name: String },
    UnknownGroup { name: String, valid: Vec<&'static str> },
    UnknownTool { name: String, suggestion: Option<String> },
    ConflictingProfile { profile: String },
    ToolExcludedByProfile { tool: String, profile: String },
    EmptyFilter,
}

impl std::fmt::Display for FilterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FilterError::UnknownProfile { name } => {
                write!(f, "unknown --profile: {} (valid: all, read, safe)", name)
            }
            FilterError::UnknownGroup { name, valid } => {
                write!(f, "unknown group: {} (valid: {})", name, valid.join(", "))
            }
            FilterError::UnknownTool { name, suggestion } => match suggestion {
                Some(s) => write!(f, "unknown tool: {} (did you mean {}?)", name, s),
                None => write!(f, "unknown tool: {}", name),
            },
            FilterError::ConflictingProfile { profile } => {
                write!(f, "conflicting profile flags: --read-only and --profile {}", profile)
            }
            FilterError::ToolExcludedByProfile { tool, profile } => write!(
                f,
                "tool {} is excluded by profile={}; drop --profile or remove {} from --tools",
                tool, profile, tool
            ),
            FilterError::EmptyFilter => {
                write!(f, "filter pipeline produced an empty tool set; nothing to expose")
            }
        }
    }
}

impl std::error::Error for FilterError {}

pub struct Filter {
    pub profile: Profile,
    pub groups: Option<Vec<String>>,
    pub exclude_groups: Option<Vec<String>>,
    allowed: HashSet<String>,
}

impl Filter {
    pub fn allows(&self, tool_name: &str) -> bool {
        self.allowed.contains(tool_name)
    }

    pub fn allowed_count(&self) -> usize {
        self.allowed.len()
    }

    pub fn resolve(raw: RawFilter) -> Result<Self, FilterError> {
        // 1. Resolve profile, reconciling --read-only with --profile.
        let profile = match (raw.profile.as_deref(), raw.read_only) {
            (None, false) => Profile::All,
            (None, true) => Profile::Read,
            (Some(p), false) => {
                Profile::parse(p).ok_or_else(|| FilterError::UnknownProfile { name: p.into() })?
            }
            (Some("read"), true) => Profile::Read,
            (Some(p), true) => return Err(FilterError::ConflictingProfile { profile: p.into() }),
        };
        let profile_label = match profile {
            Profile::All => "all",
            Profile::Read => "read",
            Profile::Safe => "safe",
        };

        // 2. Validate group inputs.
        for list in [&raw.groups, &raw.exclude_groups] {
            if let Some(groups) = list {
                for g in groups {
                    if !ALL_GROUPS.contains(&g.as_str()) {
                        return Err(FilterError::UnknownGroup {
                            name: g.clone(),
                            valid: ALL_GROUPS.to_vec(),
                        });
                    }
                }
            }
        }

        // 3. Enumerate all tool names + their ToolMeta (via tool_list + classify).
        //    We import lazily to avoid a cycle at module init.
        let all_names: Vec<(String, ToolMeta)> = crate::mcp::tool_list()
            .as_array()
            .expect("tool_list returns array")
            .iter()
            .filter_map(|t| {
                t.get("name")
                    .and_then(|v| v.as_str())
                    .and_then(|n| classify(n).map(|m| (n.to_string(), m)))
            })
            .collect();
        let known_names: HashSet<&str> =
            all_names.iter().map(|(n, _)| n.as_str()).collect();

        // 4. Validate --tools / --exclude-tools names against the full catalog.
        for list in [&raw.tools, &raw.exclude_tools] {
            if let Some(tools) = list {
                for t in tools {
                    if !known_names.contains(t.as_str()) {
                        return Err(FilterError::UnknownTool {
                            name: t.clone(),
                            suggestion: closest_name(t, &known_names),
                        });
                    }
                }
            }
        }

        // 5. Detect --tools entries that are excluded by the profile before pipelining.
        if let Some(tools) = &raw.tools {
            for t in tools {
                if let Some(meta) = all_names.iter().find(|(n, _)| n == t).map(|(_, m)| m) {
                    if !profile.allows_class(meta.class) {
                        return Err(FilterError::ToolExcludedByProfile {
                            tool: t.clone(),
                            profile: profile_label.into(),
                        });
                    }
                }
            }
        }

        // 6. Build the allowed set via the pipeline.
        let mut allowed: HashSet<String> = all_names
            .iter()
            .filter(|(_, m)| profile.allows_class(m.class))
            .map(|(n, _)| n.clone())
            .collect();

        if let Some(groups) = &raw.groups {
            allowed.retain(|n| {
                let g = all_names
                    .iter()
                    .find(|(name, _)| name == n)
                    .map(|(_, m)| m.group)
                    .unwrap_or("");
                groups.iter().any(|wanted| wanted == g)
            });
        }

        if let Some(excl) = &raw.exclude_groups {
            allowed.retain(|n| {
                let g = all_names
                    .iter()
                    .find(|(name, _)| name == n)
                    .map(|(_, m)| m.group)
                    .unwrap_or("");
                !excl.iter().any(|bad| bad == g)
            });
        }

        if let Some(tools) = &raw.tools {
            let wanted: HashSet<&str> = tools.iter().map(String::as_str).collect();
            allowed.retain(|n| wanted.contains(n.as_str()));
        }

        if let Some(excl) = &raw.exclude_tools {
            for t in excl {
                allowed.remove(t);
            }
        }

        if allowed.is_empty() {
            return Err(FilterError::EmptyFilter);
        }

        Ok(Filter {
            profile,
            groups: raw.groups,
            exclude_groups: raw.exclude_groups,
            allowed,
        })
    }
}

fn closest_name(needle: &str, haystack: &HashSet<&str>) -> Option<String> {
    haystack
        .iter()
        .min_by_key(|candidate| levenshtein(needle, candidate))
        .filter(|candidate| levenshtein(needle, candidate) <= 3)
        .map(|s| s.to_string())
}

fn levenshtein(a: &str, b: &str) -> usize {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let (n, m) = (a.len(), b.len());
    if n == 0 {
        return m;
    }
    if m == 0 {
        return n;
    }
    let mut prev: Vec<usize> = (0..=m).collect();
    let mut curr = vec![0usize; m + 1];
    for i in 1..=n {
        curr[0] = i;
        for j in 1..=m {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[m]
}
```

- [ ] **Step 4: Run tests, confirm all pass**

Run: `cargo test --test test_mcp_filter`
Expected: all 15+ tests in `test_mcp_filter.rs` pass, plus the 2 from Task 2 still pass.

- [ ] **Step 5: Commit**

```bash
git add src/mcp/filter.rs tests/test_mcp_filter.rs
git commit -m "feat(mcp): add Filter with profiles, groups, and tool allowlist/denylist"
```

---

## Task 4: Thread `Filter` through `tool_list()`, `serve()`, and `call_tool`

**Files:**
- Modify: `src/mcp.rs` (signatures + filter enforcement)
- Test: `tests/test_mcp_filter.rs` (append)

- [ ] **Step 1: Write the failing test for filtered `tool_list`**

Append to `tests/test_mcp_filter.rs`:

```rust
use clickup_cli::mcp::filtered_tool_list;

#[test]
fn filtered_tool_list_returns_only_allowed_tools() {
    let raw = RawFilter { profile: Some("read".into()), ..RawFilter::default() };
    let filter = Filter::resolve(raw).unwrap();
    let value = filtered_tool_list(&filter);
    let array = value.as_array().unwrap();
    for tool in array {
        let name = tool.get("name").unwrap().as_str().unwrap();
        assert!(filter.allows(name), "tool {} leaked past filter", name);
    }
    assert_eq!(array.len(), filter.allowed_count());
}
```

- [ ] **Step 2: Run test, confirm it fails**

Run: `cargo test --test test_mcp_filter filtered_tool_list_returns_only_allowed_tools`
Expected: compile error — `filtered_tool_list` not found.

- [ ] **Step 3: Add `filtered_tool_list` and update `serve()` + `call_tool`**

In `src/mcp.rs`, immediately after the existing `pub fn tool_list() -> Value { ... }` definition, add:

```rust
/// Returns `tool_list()` with any tool the filter disallows removed.
pub fn filtered_tool_list(filter: &filter::Filter) -> Value {
    let all = tool_list();
    let mut array = all.as_array().cloned().unwrap_or_default();
    array.retain(|tool| {
        tool.get("name")
            .and_then(|v| v.as_str())
            .map(|n| filter.allows(n))
            .unwrap_or(false)
    });
    Value::Array(array)
}
```

Change the `serve()` signature:

```rust
pub async fn serve(filter: filter::Filter) -> Result<(), Box<dyn std::error::Error>> {
```

Inside `serve()`, replace the `tools/list` branch:

```rust
"tools/list" => ok_response(&id, json!({"tools": tool_list()})),
```

with:

```rust
"tools/list" => ok_response(&id, json!({"tools": filtered_tool_list(&filter)})),
```

Replace the `tools/call` branch to reject filtered tools:

```rust
"tools/call" => {
    let params = msg.get("params").cloned().unwrap_or(json!({}));
    let tool_name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

    if tool_name.is_empty() {
        let result = tool_error("Missing tool name".to_string());
        ok_response(&id, result)
    } else if !filter.allows(tool_name) {
        error_response(
            &id,
            -32601,
            &format!("Method not found: {} (filtered out at startup)", tool_name),
        )
    } else {
        let result = call_tool(tool_name, &arguments, &client, &workspace_id).await;
        ok_response(&id, result)
    }
}
```

(The old branch wrapped every outcome as a success; the rejection path returns a proper JSON-RPC error response instead of a tool result, matching `-32601 method not found`.)

- [ ] **Step 4: Update the MCP command to build a default Filter**

In `src/commands/mcp_cmd.rs`, change `execute` to construct a default `Filter` for now. Replace the existing `execute` body:

```rust
pub async fn execute(command: McpCommands) -> Result<(), CliError> {
    match command {
        McpCommands::Serve => {
            let filter = crate::mcp::filter::Filter::resolve(
                crate::mcp::filter::RawFilter::default(),
            )
            .map_err(|e| CliError::ConfigError(e.to_string()))?;
            crate::mcp::serve(filter)
                .await
                .map_err(|e| CliError::ConfigError(e.to_string()))
        }
    }
}
```

(CLI flags get wired in Task 5.)

- [ ] **Step 5: Build and run all tests**

Run: `cargo build && cargo test`
Expected: build succeeds; all existing tests plus the new filter tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/mcp.rs src/commands/mcp_cmd.rs tests/test_mcp_filter.rs
git commit -m "feat(mcp): enforce Filter in tools/list and tools/call"
```

---

## Task 5: CLI flags and env-var parsing on `mcp serve`

**Files:**
- Modify: `src/commands/mcp_cmd.rs`
- Test: `tests/test_mcp_filter.rs` (append env-var test)

- [ ] **Step 1: Add clap flags to `McpCommands::Serve`**

Replace the contents of `src/commands/mcp_cmd.rs`:

```rust
use clap::Subcommand;
use crate::error::CliError;
use crate::mcp::filter::{Filter, RawFilter};

#[derive(Subcommand)]
pub enum McpCommands {
    /// Start the MCP server (reads JSON-RPC from stdin, writes to stdout).
    Serve {
        /// Preset tool bundle: `all` (default), `read`, `safe`.
        #[arg(long, value_name = "NAME")]
        profile: Option<String>,

        /// Shortcut for `--profile read`.
        #[arg(long)]
        read_only: bool,

        /// Include only tools in these resource groups (comma-separated).
        #[arg(long, value_name = "LIST", value_delimiter = ',')]
        groups: Option<Vec<String>>,

        /// Drop tools in these resource groups (comma-separated).
        #[arg(long, value_name = "LIST", value_delimiter = ',')]
        exclude_groups: Option<Vec<String>>,

        /// Include only these tools by exact name (comma-separated).
        #[arg(long, value_name = "LIST", value_delimiter = ',')]
        tools: Option<Vec<String>>,

        /// Drop these tools by exact name (comma-separated).
        #[arg(long, value_name = "LIST", value_delimiter = ',')]
        exclude_tools: Option<Vec<String>>,
    },
}

fn env_list(var: &str) -> Option<Vec<String>> {
    std::env::var(var).ok().map(|v| {
        v.split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect()
    })
}

fn env_bool(var: &str) -> bool {
    matches!(
        std::env::var(var).ok().as_deref(),
        Some("1" | "true" | "yes" | "on")
    )
}

fn env_string(var: &str) -> Option<String> {
    std::env::var(var).ok().filter(|s| !s.is_empty())
}

pub async fn execute(command: McpCommands) -> Result<(), CliError> {
    match command {
        McpCommands::Serve {
            profile,
            read_only,
            groups,
            exclude_groups,
            tools,
            exclude_tools,
        } => {
            let raw = RawFilter {
                profile: profile.or_else(|| env_string("CLICKUP_MCP_PROFILE")),
                read_only: read_only || env_bool("CLICKUP_MCP_READ_ONLY"),
                groups: groups.or_else(|| env_list("CLICKUP_MCP_GROUPS")),
                exclude_groups: exclude_groups
                    .or_else(|| env_list("CLICKUP_MCP_EXCLUDE_GROUPS")),
                tools: tools.or_else(|| env_list("CLICKUP_MCP_TOOLS")),
                exclude_tools: exclude_tools
                    .or_else(|| env_list("CLICKUP_MCP_EXCLUDE_TOOLS")),
            };
            let filter = Filter::resolve(raw)
                .map_err(|e| CliError::ConfigError(e.to_string()))?;
            crate::mcp::serve(filter)
                .await
                .map_err(|e| CliError::ConfigError(e.to_string()))
        }
    }
}
```

- [ ] **Step 2: Build**

Run: `cargo build`
Expected: clean build.

- [ ] **Step 3: Sanity-check CLI**

Run: `cargo run -- mcp serve --help`
Expected: help output lists all six new flags with descriptions.

Also:

Run: `cargo run -- mcp serve --profile gibberish`
Expected: immediate error ending with `unknown --profile: gibberish (valid: all, read, safe)` and non-zero exit.

Run: `CLICKUP_MCP_PROFILE=read cargo run -- mcp serve --profile safe`
Expected: the server starts with profile=safe (CLI wins over env) — if CLICKUP_TOKEN is unset it will then error on the missing token, which is fine for this check.

- [ ] **Step 4: Commit**

```bash
git add src/commands/mcp_cmd.rs
git commit -m "feat(mcp): wire --profile / --read-only / --groups / --tools flags"
```

---

## Task 6: Startup log line

**Files:**
- Modify: `src/mcp.rs`

- [ ] **Step 1: Emit the summary line from `serve()`**

In `src/mcp.rs`, inside `serve()`, add this block **after** `filter` is in scope but **before** the `while let Some(line)` loop (i.e. near the existing `ClickUpClient::new` setup):

```rust
let profile_label = match filter.profile {
    filter::Profile::All => "all",
    filter::Profile::Read => "read",
    filter::Profile::Safe => "safe",
};
let groups_str = filter
    .groups
    .as_ref()
    .map(|g| format!(", groups=[{}]", g.join(",")))
    .unwrap_or_default();
let excluded_groups_str = filter
    .exclude_groups
    .as_ref()
    .map(|g| format!(", exclude-groups=[{}]", g.join(",")))
    .unwrap_or_default();
eprintln!(
    "MCP: profile={}{}{}, exposing {}/{} tools",
    profile_label,
    groups_str,
    excluded_groups_str,
    filter.allowed_count(),
    tool_list().as_array().map(Vec::len).unwrap_or(0),
);
```

- [ ] **Step 2: Smoke test**

Run: `cargo build && echo '' | CLICKUP_TOKEN=dummy cargo run -- mcp serve --profile read 2>&1 >/dev/null | head -1`
Expected: a single line like `MCP: profile=read, exposing 37/143 tools` on stderr (exact count will depend on classification; the format is what matters).

- [ ] **Step 3: Commit**

```bash
git add src/mcp.rs
git commit -m "feat(mcp): log active filter on startup"
```

---

## Task 7: Documentation updates

**Files:**
- Modify: `README.md`, `docs/mcp.md`, `CLAUDE.md`, `src/commands/agent_config.rs`

- [ ] **Step 1: README — add "Limiting MCP tools" subsection**

Open `README.md`. Find the existing MCP section (search for `## MCP` or `mcp serve`). Immediately before the next top-level `##` heading, insert:

````markdown
### Limiting MCP tools

By default `clickup mcp serve` exposes all 143 tools. You can restrict this at startup to shrink the LLM's context and enforce access control. Flags and matching env vars:

| Flag | Env var | Purpose |
| --- | --- | --- |
| `--profile <name>` | `CLICKUP_MCP_PROFILE` | Preset: `all` (default), `read`, `safe` |
| `--read-only` | `CLICKUP_MCP_READ_ONLY=1` | Alias for `--profile read` |
| `--groups a,b,c` | `CLICKUP_MCP_GROUPS` | Include only these resource groups |
| `--exclude-groups x,y` | `CLICKUP_MCP_EXCLUDE_GROUPS` | Drop these groups |
| `--tools t1,t2` | `CLICKUP_MCP_TOOLS` | Include only these tools by exact name |
| `--exclude-tools t1` | `CLICKUP_MCP_EXCLUDE_TOOLS` | Drop these tools |

`--read-only` agent:

```json
{
  "mcpServers": {
    "clickup": {
      "command": "clickup",
      "args": ["mcp", "serve", "--read-only"]
    }
  }
}
```

Task-focused agent (task + comment + time groups only):

```json
{
  "mcpServers": {
    "clickup": {
      "command": "clickup",
      "args": ["mcp", "serve", "--groups", "task,comment,time"]
    }
  }
}
```

Filtered tools are rejected at `tools/call` as well as hidden from `tools/list`, so a misbehaving agent can't smuggle a destructive call past the filter.
````

- [ ] **Step 2: `docs/mcp.md` — mirror the section**

Open `docs/mcp.md`. Find a logical place (after the basic "how to run" instructions, before any advanced internals) and insert the same block as Step 1, adapted to the file's Jekyll-style tone if needed. Keep the table and both JSON snippets.

- [ ] **Step 3: `CLAUDE.md` — add one-line note**

In `CLAUDE.md`, find the existing `## MCP Server` section and change:

```markdown
Start with `clickup mcp serve`. Returns token-efficient compact JSON (same flattening as CLI tables). Exposes 143 tools with 100% ClickUp API coverage — every endpoint available via CLI is also available as an MCP tool.
```

to:

```markdown
Start with `clickup mcp serve`. Returns token-efficient compact JSON (same flattening as CLI tables). Exposes 143 tools with 100% ClickUp API coverage — every endpoint available via CLI is also available as an MCP tool.

To limit what the server exposes, pass `--profile {all|read|safe}`, `--read-only`, `--groups`, `--exclude-groups`, `--tools`, or `--exclude-tools` (or the matching `CLICKUP_MCP_*` env vars).
```

- [ ] **Step 4: Append MCP filter hint to `AGENT_REFERENCE`**

In `src/commands/agent_config.rs`, locate the `AGENT_REFERENCE` string (around line 40). Just before the closing `<!-- clickup-cli:end -->` marker, insert:

```
 MCP server: `clickup mcp serve [--profile all|read|safe] [--read-only] [--groups LIST] [--tools LIST]` (also via `CLICKUP_MCP_PROFILE`, `CLICKUP_MCP_GROUPS`, `CLICKUP_MCP_TOOLS`).
```

(The space prefix preserves the run-on sentence style of the rest of the reference.)

- [ ] **Step 5: Build docs sanity check**

Run: `cargo build && cargo test`
Expected: everything still passes; the AGENT_REFERENCE is just a string constant so no semantic tests need to change.

- [ ] **Step 6: Commit**

```bash
git add README.md docs/mcp.md CLAUDE.md src/commands/agent_config.rs
git commit -m "docs: document MCP tool filtering flags"
```

---

## Task 8: Changelog entry + version bump + release

**Files:**
- Modify: `CHANGELOG.md`, `Cargo.toml`, `npm/package.json`
- Generated: `Cargo.lock`

- [ ] **Step 1: Update `CHANGELOG.md`**

Replace the current `## [Unreleased]` section with:

```markdown
## [Unreleased]

## [0.8.0] - 2026-04-22

### Added
- `clickup mcp serve` now accepts filtering flags so the MCP server can expose a subset of its 143 tools at startup:
  - `--profile <all|read|safe>` (default `all`): `read` exposes only read-class tools; `safe` excludes destructive tools.
  - `--read-only` shortcut for `--profile read`.
  - `--groups` / `--exclude-groups` to include or drop resource groups (e.g. `task,comment,time`).
  - `--tools` / `--exclude-tools` to include or drop individual tools by exact name.
  - Matching environment variables: `CLICKUP_MCP_PROFILE`, `CLICKUP_MCP_READ_ONLY`, `CLICKUP_MCP_GROUPS`, `CLICKUP_MCP_EXCLUDE_GROUPS`, `CLICKUP_MCP_TOOLS`, `CLICKUP_MCP_EXCLUDE_TOOLS`.
- Filters apply to both `tools/list` (shrinks the LLM's context) and `tools/call` (rejects filtered tools with JSON-RPC `-32601`), so filtering is an access-control guarantee, not just a context optimization.
- Startup log line on stderr summarizing the active filter, e.g. `MCP: profile=read, exposing 37/143 tools`.
- Internal tool classifier mapping every MCP tool to a `(class, group)` pair with a CI self-check that fails if a tool can't be classified.

### Fixed
- Release workflow (`.github/workflows/build.yml`): `cargo publish` now runs with `--allow-dirty` (build artifacts in the workspace were making the tree "dirty") and all three publish steps (crates.io, npm, GitHub Packages) now check whether the version already exists before publishing and fail hard on any other error. The previous `|| echo "skipped"` pattern silently swallowed the crates.io failure during the v0.7.0 release.
```

(The "Fixed" block is the content that was previously sitting under `[Unreleased]` — we're rolling it into 0.8.0.)

Update the compare-link footnotes at the bottom of the file:

```markdown
[Unreleased]: https://github.com/nicholasbester/clickup-cli/compare/v0.8.0...HEAD
[0.8.0]: https://github.com/nicholasbester/clickup-cli/compare/v0.7.0...v0.8.0
[0.7.0]: https://github.com/nicholasbester/clickup-cli/compare/v0.6.7...v0.7.0
```

- [ ] **Step 2: Bump `Cargo.toml`**

In `Cargo.toml`, change:

```toml
version = "0.7.0"
```

to:

```toml
version = "0.8.0"
```

- [ ] **Step 3: Bump `npm/package.json`**

In `npm/package.json`, change:

```json
"version": "0.7.0",
```

to:

```json
"version": "0.8.0",
```

- [ ] **Step 4: Regenerate `Cargo.lock`**

Run: `cargo build`
Expected: `Cargo.lock` updates the root `clickup-cli` version to `0.8.0`. No unrelated changes.

- [ ] **Step 5: Full test run**

Run: `cargo test`
Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add CHANGELOG.md Cargo.toml Cargo.lock npm/package.json
git commit -m "release: v0.8.0"
```

- [ ] **Step 7: Hand release tagging back to the user**

Do **not** tag or push. Tagging `v0.8.0` triggers `.github/workflows/build.yml`, which publishes to crates.io, npm, and GitHub Packages. That step is the user's call. Report the finished branch and let them drive the tag:

```
All tasks complete on branch <current branch>. Tag v0.8.0 and push when ready; the release workflow will handle crates.io + npm + GitHub Release.
```

---

## Self-Review Checklist

Run through this before handing the plan to execution.

1. **Every spec requirement has a task:**
   - Classification rule + override table → Task 1.
   - Self-check test → Task 2.
   - Filter config surface (flags + env vars) → Tasks 3 + 5.
   - Filter pipeline + errors → Task 3.
   - Defense-in-depth on `tools/call` → Task 4.
   - Startup log → Task 6.
   - Docs (README, docs/mcp.md, CLAUDE.md, agent-config) → Task 7.
   - Changelog + version bumps → Task 8.
2. **Types stay consistent.** `Filter`, `RawFilter`, `Profile`, `FilterError`, `Class`, `ToolMeta`, `classify`, `filtered_tool_list` — same names everywhere they appear.
3. **No placeholders.** Every code block is full code; every shell command shows expected output.
4. **Commits are atomic.** Each task ends with a single commit covering only that task's files.
