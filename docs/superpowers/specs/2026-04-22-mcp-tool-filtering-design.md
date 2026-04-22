# MCP Tool Filtering — Design

**Status:** Approved
**Date:** 2026-04-22
**Target version:** v0.8.0

## Problem

`clickup mcp serve` unconditionally exposes all 143 tools via `tools/list`. MCP clients fetch this list once per session and inject it into the model's context, so every session pays the full context cost whether or not the agent uses all groups. There is also no way to run the server in a read-only posture, which users want for safer agent configurations (reporting bots, browsing assistants, untrusted contexts).

Two concerns collapse into one lever: whatever `tools/list` returns is both what the model sees (context cost) and what it can call (access scope). The design therefore adds a single startup-time filter that shapes both.

## Goals

- Let the MCP client pick a subset of tools via CLI flags or env vars in the MCP server config.
- Support coarse safety modes (read-only, no-destructive).
- Support fine-grained selection by resource group and by individual tool name.
- Reduce context footprint proportionally to the filter.
- Preserve current behavior when no filter is set (default = all 143 tools).
- Keep maintenance cost low: new tools should classify themselves automatically by naming convention.

## Non-goals

- Per-call dynamic tool discovery. Vanilla MCP does not support clients requesting subsets per call; filtering is startup-time only.
- Runtime reconfiguration (SIGHUP reload, tool hot-swap). Out of scope.
- Row-level access control (per-workspace, per-list permissions). Out of scope; this is tool-level only.
- Auth-token-based filtering. The token already enforces API-level permissions; this layer is about shaping what the model is told it can do.

## Design

### 1. Tool classification

Each of the 143 tools gets classified into `(class, group)`:

- **class** ∈ { `Read`, `Write`, `Destructive` }
- **group** = one of the 28 existing resource groups (`task`, `comment`, `time`, `chat`, …)

Classification uses **auto-derivation from the tool name**, with a small override table for exceptions.

#### Auto-derivation rule

Applied in order:

1. Look up `tool_name` in `OVERRIDES`. If found, return that entry.
2. Strip the `clickup_` prefix and split on `_`. First segment is `group`. Remaining segments form the verb portion.
3. Scan the verb segments. Class is decided by the highest-priority verb found, with priority `Destructive > Write > Read`:
   - If any segment matches a destructive verb → `Destructive`.
   - Else if any segment matches a write verb → `Write`.
   - Else if any segment matches a read verb → `Read`.
   - Else → classification fails and the self-check panics in debug builds, forcing an override entry.

Scanning all segments (not just the trailing one) matters because ClickUp names like `task_add_dep`, `task_remove_tag`, `checklist_delete_item`, and `list_add_task` carry the verb before a noun. Priority order ensures that `goal_delete_kr` classifies as `Destructive` even though `kr` is the trailing segment.

**Verb sets:**

- **Read verbs:** `list`, `get`, `search`, `current`, `pages`, `followers`, `members`, `history`, `tags`, `whoami`, `check`, `replies`, `tagged`
- **Write verbs:** `create`, `update`, `set`, `add`, `start`, `stop`, `move`, `apply`, `invite`, `rename`, `share`, `attach`, `link`, `reply`, `send`, `page`, `dm`, `edit`, `upload`
- **Destructive verbs:** `delete`, `remove`, `unshare`, `unlink`, `unset`

#### Override table

A `const OVERRIDES: &[(&str, Class, &str)]` handles tools that don't fit the convention (no group segment, or a verb that isn't in any set). Initial entries:

| Tool | Class | Group | Reason |
|------|-------|-------|--------|
| `clickup_search` | Read | `workspace` | No group segment; workspace-wide search |
| `clickup_whoami` | Read | `auth` | No group segment |
| `clickup_workspace_plan` | Read | `workspace` | `plan` too generic to add to read verbs |
| `clickup_workspace_seats` | Read | `workspace` | `seats` too generic to add to read verbs |
| `clickup_task_replace_estimates` | Write | `task` | `replace` reads as destructive but overwrites in place |
| `clickup_task_time_in_status` | Read | `task` | No segment matches a verb |

The override table is the escape hatch for any tool that doesn't fit the convention. New tools should follow the convention and not need entries.

#### Classification self-check

A unit test iterates every name in `tool_list()`, calls `classify()` on each, and asserts that (a) classification succeeds and (b) the returned group is one of the 28 known groups. This runs in both debug and release test runs, so any new tool that breaks the convention fails CI before it ships. `serve()` also runs the same check in debug builds as a belt-and-braces guard.

### 2. Filter configuration

`clickup mcp serve` accepts the following flags. Each has an environment-variable equivalent so MCP client configs that can't easily pass CLI args still work.

| Flag | Env var | Meaning |
|------|---------|---------|
| `--profile <name>` | `CLICKUP_MCP_PROFILE` | Named preset: `all` (default), `read`, `safe` |
| `--read-only` | `CLICKUP_MCP_READ_ONLY=1` | Shortcut for `--profile read` |
| `--groups <a,b,c>` | `CLICKUP_MCP_GROUPS` | Include only these resource groups |
| `--exclude-groups <x,y>` | `CLICKUP_MCP_EXCLUDE_GROUPS` | Drop these groups |
| `--tools <t1,t2>` | `CLICKUP_MCP_TOOLS` | Include only these tools (exact names) |
| `--exclude-tools <t1>` | `CLICKUP_MCP_EXCLUDE_TOOLS` | Drop these tools |

CLI flag takes precedence over the matching env var.

#### Profile definitions

| Profile | Tools included |
|---------|----------------|
| `all` | All 143 tools. Default when nothing is set. |
| `read` | All tools with class = `Read`. |
| `safe` | All tools with class ∈ { `Read`, `Write` } (excludes `Destructive`). |

#### Filter pipeline

Applied in this order:

1. Start with the profile's base set (default: `all`).
2. If `--groups` is set, restrict to tools whose group is in the list.
3. If `--exclude-groups` is set, drop tools whose group is in the list.
4. If `--tools` is set, restrict to the named tools (intersected with step 3's result).
5. If `--exclude-tools` is set, drop the named tools.
6. If the final set is empty, exit with a non-zero status and a clear error.

#### Validation and errors

- `--read-only` together with `--profile safe` (or any non-`read` profile) → startup error: `"conflicting profile flags: --read-only and --profile safe"`.
- Unknown profile name → error listing valid profiles.
- Unknown group name → error listing the 28 valid groups.
- Unknown tool name → error with a Levenshtein-based "did you mean" hint.
- `--tools X` where `X` is filtered out by the profile/group pipeline → error: `"tool X is excluded by profile=read; drop --profile or remove X from --tools"`.

#### Defense in depth on `tools/call`

Filtering applies to both `tools/list` and `tools/call`. If a client somehow invokes a filtered tool (stale cache, misbehaving agent, direct JSON-RPC), the server returns `-32601 Method not found: <tool>` rather than executing it. This keeps the access-control guarantee independent of client behavior.

#### Startup log

On startup, `serve()` writes one line to stderr summarizing the active filter:

```
MCP: profile=read, groups=[task,comment,time], exposing 37/143 tools
```

### 3. Implementation footprint

- **`src/mcp.rs`** — Refactor `tool_list()` to take `&Filter` and return the filtered array. `call_tool` gains the same filter and rejects filtered tools with `-32601`. The existing giant match stays intact; only the entry-point functions grow a parameter.
- **`src/mcp/classify.rs`** *(new)* — `Class` enum, `ToolMeta` struct, `classify(name) -> ToolMeta`, `OVERRIDES` table, verb sets. No dependencies beyond `std`.
- **`src/mcp/filter.rs`** *(new)* — `Filter` struct, `Filter::from_cli_and_env(...)` parsing, pipeline application, validation errors. Produces `HashSet<&'static str>` of allowed tool names.
- **`src/commands/mcp_cmd.rs`** — `McpCommands::Serve` grows the new flags. `execute` builds a `Filter`, passes it into `mcp::serve(filter)`.
- **`serve()`** signature changes to `serve(filter: Filter)`.

### 4. Tests

New `tests/test_mcp_filter.rs`:

- Every one of the 143 tools classifies without panic (pulled from `tool_list()`).
- `profile=all` exposes 143 tools; `profile=read` exposes exactly the read-class count; `profile=safe` excludes destructive.
- Pipeline order: `--profile read --groups task` intersects correctly.
- `--exclude-groups chat` removes chat tools and nothing else.
- `--tools clickup_task_get` alone exposes one tool.
- Unknown group / tool / profile produces the expected error text.
- Conflicting flags (`--read-only --profile safe`) error at parse time.
- Empty final set errors at startup.
- `call_tool` on a filtered name returns `-32601`.

### 5. Documentation

- **`README.md`** — New "Limiting MCP tools" subsection under the MCP section. Shows three canonical `claude_desktop_config.json` snippets: default, `--read-only`, and `--groups task,comment,time`.
- **`docs/mcp.md`** (Jekyll site, clickup-cli.com) — Mirrors README content. Adds a table of profiles with tool counts and a short example for each flag.
- **`CLAUDE.md`** — One line in the MCP Server section noting that `clickup mcp serve` accepts `--profile`, `--read-only`, `--groups`, `--tools`, plus env-var equivalents.
- **`agent-config`** embedded reference text — Updated to mention `clickup mcp serve [--profile N]`.

### 6. Changelog and release

- **`CHANGELOG.md`** — Convert `[Unreleased]` into `## [0.8.0] - 2026-04-22` with an `### Added` block describing profiles, flags, env vars, and the classification layer. Add a new empty `[Unreleased]` section on top. Update compare links: `[Unreleased] = v0.8.0...HEAD`, `[0.8.0] = v0.7.0...v0.8.0`.
- **Version bumps:**
  - `Cargo.toml`: `0.7.0` → `0.8.0`
  - `npm/package.json`: `0.6.7` → `0.8.0` (align versions this release)
  - `Cargo.lock` regenerates from `cargo build`.
- **Release:** Tagging `v0.8.0` triggers the existing `.github/workflows/build.yml` pipeline, which publishes to crates.io, npm, and GitHub Packages, and cuts a GitHub Release. No workflow changes required.

## Alternatives considered

- **Explicit per-tool annotation** (class/group typed into each tool definition). Gives a single source of truth per tool but adds 143 manual entries now and requires remembering one per new tool. Rejected in favor of auto-derive + overrides, which gets ~130 of 143 right automatically and makes convention-following tools free.
- **Pure auto-derivation with no overrides.** Rejected: brittle for tools that don't fit `clickup_<group>_<verb>` (e.g., `clickup_search`, `clickup_whoami`, `clickup_task_replace_estimates`).
- **Additional baked-in profiles (`core`, `tasks`).** Deferred. The `--groups` / `--tools` escape hatches cover these cases today; we can ship curated profiles later if usage patterns justify it.
- **Per-call dynamic discovery.** Not supported by vanilla MCP clients; would require a non-standard extension.

## Risks and open questions

- **Classification drift.** New tools added without considering class/group will be silently misclassified by the auto-deriver. Mitigated by the classification self-check unit test described above, which fails CI if any tool fails to classify.
- **Version alignment.** npm has been drifting behind crates.io (0.6.7 vs 0.7.0). This release aligns both at 0.8.0; future releases should keep them in lockstep.
- **Config-file filtering.** Intentionally not adding `[mcp]` section to `.clickup.toml` / global config in this release. Flags + env vars are enough, and adding a third config surface is scope creep. Easy to layer on later.
