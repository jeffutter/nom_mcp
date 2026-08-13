---
id: TASK-41
title: Add MCP Apps UI widget for goal progress
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-13 17:15'
updated_date: '2026-08-13 18:23'
labels:
  - planned
dependencies:
  - TASK-42
references:
  - 'https://modelcontextprotocol.io/extensions/apps/overview'
  - >-
    https://github.com/modelcontextprotocol/ext-apps/blob/main/specification/2026-01-26/apps.mdx
  - TASK-1.11
  - TASK-2.17
documentation:
  - CONTEXT.md
  - backlog/docs/doc-5 - nom_mcp-v1-implementation-spec.md
priority: medium
type: feature
ordinal: 46000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
nom_mcp currently returns every tool result as plain text/JSON, even though `set_widget_display`/`get_widget_display` (added in TASK-2.17 per the design in TASK-1.11) already persist a per-user on/off preference for "rich MCP-client-rendered widgets vs plain text/JSON." That flag is plumbing only today — no tool branches on it.

The MCP ecosystem now has an official extension for this, "MCP Apps" (modelcontextprotocol/ext-apps, launched 2026-01-26), which Claude and other MCP-Apps-capable clients support: a tool response can point to a `ui://` HTML resource (via `_meta`) that the client fetches and renders in a sandboxed iframe alongside — not instead of — the existing structured/text output.

Add the first, minimal widget using this mechanism for `get_goal_progress` (nom-core/src/goal/mod.rs), since it already returns a clean, self-contained payload (per-nutrient consumed-vs-target and weight progress) that's a natural fit for a small visual progress view. This is the first real consumer of the widget-display toggle, and should establish the pattern (resource construction, gating on the toggle, response shape) that later tools can reuse for their own widgets.

Keep scope to this one tool — do not build a general widget framework or migrate other tools in this task.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 When `widget_display_enabled` is true, the `get_goal_progress` tool definition returned by `list_tools` includes a `_meta.ui.resourceUri` pointing at a registered `ui://` HTML resource, per the MCP Apps extension (SEP-1724 / modelcontextprotocol/ext-apps spec 2026-01-26).
- [x] #2 A `resources/read` call for that `ui://` resource returns a valid `text/html;profile=mcp-app` document that visually represents the same data `get_goal_progress` already returns: per-nutrient consumed-vs-target comparison (calories, protein, carbs, fat, fiber) and weight progress for the queried date.
- [x] #3 Calling `get_goal_progress` (`call_tool`) continues to return its existing JSON content unchanged regardless of the widget-display setting — the widget augments how a capable host renders the result, it does not replace or alter the tool's own response.
- [x] #4 When `widget_display_enabled` is false (the default), the `get_goal_progress` tool definition has no `_meta.ui` field, and `list_tools`/`call_tool` output is byte-for-byte unchanged from current behavior.
- [x] #5 Automated tests cover both `list_tools` shapes (with and without `_meta.ui`) gated by `widget_display_enabled`, and a `resources/read` test for the new `ui://` resource.
- [x] #6 CONTEXT.md and doc-5's widget-display section are updated to reflect that `get_goal_progress` is now a real consumer of the toggle, rather than describing it as unused plumbing.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 Manually verified (or explicitly noted as unverifiable in this environment) that the rendered widget displays correctly in at least one MCP-Apps-capable client.
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Implementation Plan: MCP Apps UI widget for get_goal_progress

### Protocol shape (verified against modelcontextprotocol/ext-apps spec 2026-01-26)

The `_meta.ui.resourceUri` pointer lives on the **Tool declaration** returned by
`list_tools`, not on the `CallToolResult`. Flow:

1. `list_tools` response: the `get_goal_progress` `Tool` object carries
   `_meta: {"ui": {"resourceUri": "ui://nom-mcp/goal-progress"}}` — but ONLY when
   `widget_display_enabled` is true. When false, `meta` stays `None` (identical
   to every other tool today).
2. Host resolves that URI via `resources/read` → server returns a static HTML
   document, `mimeType: "text/html;profile=mcp-app"`.
3. `call_tool("get_goal_progress", ...)` is **completely unchanged** — same
   `ContentBlock::text(json)` as today, in both widget states. The host pushes
   this JSON into the already-rendered widget iframe via its own
   `ui/notifications/tool-result` bridge; the server does not need to know
   about that hop.

This means the `Operation` trait and `GetGoalProgress::execute_json` are
**not touched at all** — `get_goal_progress` has no `surfaces()` override
(defaults to `Surfaces::ALL`, i.e. also exposed on CLI/HTTP), so any
widget-gating logic inside `execute_json` would leak into non-MCP surfaces.
All widget-gating lives in `nom-core/src/operation/mcp_handler.rs`, which is
MCP-only.

### 1. Extract a shared widget-flag reader (nom-core/src/widget/mod.rs)

`GetWidgetDisplay::execute_json` (lines ~59-100) already has the read logic:
```rust
SELECT widget_display_enabled FROM settings LIMIT 1
```
defaulting to `false` when no row exists. Extract this into:
```rust
pub(crate) async fn widget_display_enabled(conn: &Connection) -> Result<bool, ErrorData>
```
and have `GetWidgetDisplay::execute_json` call it (returns `{"enabled": ...}` as
today — no behavior change, just deduplication). This gives mcp_handler.rs a
single source of truth to call instead of re-deriving the SQL.

### 2. Gate the Tool's `_meta.ui` in list_tools (nom-core/src/operation/mcp_handler.rs)

`list_tools` (lines 182-189) currently just calls `build_tools()`. Change it to:
1. Call `build_tools()` as today.
2. Open a connection using the existing `#[cfg(test)]`/`db_path` pattern (same
   as `dispatch_read_resource` already does).
3. Call `widget::widget_display_enabled(&conn)`.
4. If `true`, find the tool named `"get_goal_progress"` in the built `Vec<Tool>`
   and set `tool.meta = Some(Meta(json_map!{"ui": {"resourceUri": "ui://nom-mcp/goal-progress"}}))`
   (`Tool.meta` is a plain public field — confirmed in rmcp 2.2.0
   `src/model/tool.rs`, no builder method needed, direct assignment is fine).
   If `false`, leave every tool's `meta` as `None` — identical to current output.

Special-case by tool name inline here (do not add a new `Operation` trait
method) — the ticket scopes this to one tool, and adding trait surface for a
single consumer would be premature generalization.

### 3. Serve the static widget resource (nom-core/src/operation/mcp_handler.rs)

Add a match arm to `dispatch_read_resource` (lines ~86-153) for
`"ui://nom-mcp/goal-progress"`:
```rust
"ui://nom-mcp/goal-progress" => Ok(ReadResourceResult::new(vec![
    ResourceContents::TextResourceContents {
        uri: uri.to_string(),
        mime_type: Some("text/html;profile=mcp-app".to_string()),
        text: GOAL_PROGRESS_WIDGET_HTML.to_string(),
        meta: None,
    },
])),
```
where `GOAL_PROGRESS_WIDGET_HTML` is `const &str = include_str!(...)` pointing
at a new static asset (see step 4). No DB access needed here — the widget gets
live data from the `call_tool` response via the host's bridge, not by the
server re-querying on resource fetch.

Do **not** add this URI to `build_resources()`/`list_resources()` — the spec
explicitly allows (and, since discovery is via the tool's `_meta.ui.resourceUri`,
recommends) omitting UI-only resources from the general resource listing.

### 4. Widget HTML asset

Add `nom-core/assets/goal_progress_widget.html` (new file, no build step —
self-contained HTML + vanilla JS, consistent with the ticket's "simple" scope
and the spec's vanilla-JS example pattern). Responsibilities:
- Implement the minimal MCP Apps postMessage handshake (`ui/initialize` on
  load, listen for `ui/notifications/tool-result`). Use the
  `examples/basic-server-vanillajs` starter in
  https://github.com/modelcontextprotocol/ext-apps/tree/main/examples as the
  reference implementation for the handshake — do not hand-roll the JSON-RPC
  framing from scratch, that's exactly the kind of subtle protocol detail
  worth copying from a working example.
- Render the payload shape `GetGoalProgress` already returns (confirmed
  current shape): `calories`/`protein_g`/`carbs_g`/`fat_g`/`fiber_g`, each
  `{consumed, target?, remaining?, percent?, direction?, status?}`, plus
  `weight: {latest_weight?, target_weight?, remaining?, status?}`. Simple bars
  or numeric rows per nutrient are enough — this is intentionally a minimal
  first widget, not a dashboard.

### 5. Tests (nom-core/src/operation/mcp_handler.rs, following existing
   `TempDb` + `#[serial_test::serial] #[tokio::test]` patterns already used
   throughout this file and widget/mod.rs)

- `list_tools` with no `settings` row (today's default) → `get_goal_progress`
  tool's `meta` is `None`, matching every other tool (AC#4).
- `list_tools` with `widget_display_enabled = true` (seed via
  `SetWidgetDisplay::execute_json` or a raw INSERT against the temp DB) →
  `get_goal_progress` tool's `meta` contains
  `{"ui": {"resourceUri": "ui://nom-mcp/goal-progress"}}`; assert at least one
  other tool's `meta` is still `None` (proves the gate is scoped to this one
  tool, not applied globally) (AC#1).
- `dispatch_read_resource("ui://nom-mcp/goal-progress")` → `Ok`, `mime_type ==
  Some("text/html;profile=mcp-app")`, non-empty `text` (AC#2).
- `call_tool("get_goal_progress", ...)` → assert byte-identical
  `CallToolResult.content` whether `widget_display_enabled` is true or false
  (AC#3 — the strongest test in this ticket, since it's the thing most likely
  to regress silently).

### 6. Documentation

- `CONTEXT.md` lines 35-37 (Widget Display glossary entry): replace "v1 stores
  and exposes it but no tool or Resource output branches on it yet" with a
  statement that `get_goal_progress`'s `list_tools` output is now gated on it
  (link TASK-41).
- `backlog/docs/doc-5 - nom_mcp-v1-implementation-spec.md` §8 (line 116): same
  update — this is the line that currently says "no tool or Resource output
  branches on it yet"; it becomes the first thing that's no longer true after
  this ticket.

### Verification

- `cargo test -p nom-core --lib` (goal, widget, mcp_handler modules).
- `cargo clippy -p nom-core --lib`.
- DoD manual-verification step: requires an actual MCP-Apps-capable host
  (e.g. Claude Desktop) connected to a locally-run `nom-mcp` server with
  `widget_display_enabled` toggled on. If no such client is reachable from
  the implementing environment, the DoD item should be explicitly marked "not
  verifiable in this environment" per its own wording, rather than silently
  skipped — Claude Desktop's own MCP Apps rendering is documented upstream as
  currently unreliable (see ext-apps#671 / claude-ai-mcp#165, both open as of
  this writing), so a failure to visually render there is not necessarily a
  bug in this implementation.

### Explicitly out of scope (per ticket's own scope note)

- Checking the client's negotiated `io.modelcontextprotocol/ui` capability
  (from its `initialize` request) before attaching `_meta.ui`. The spec lists
  this as a SHOULD, with graceful degradation on non-supporting hosts as the
  documented fallback behavior. `RequestContext<RoleServer>`'s availability
  for reading negotiated peer capabilities inside `list_tools` wasn't
  confirmed during planning — worth a follow-up ticket if false-positive
  widget advertisement to non-supporting clients turns out to matter in
  practice, but not required for this ticket's ACs.
- Any other tool's widget (weekly-summary, meal logging, etc.) — this ticket
  establishes the pattern for `get_goal_progress` only, as scoped.

### Dependency: TASK-42 (rmcp 2.2.0 → 3.1.x upgrade)

This plan's code-level specifics (the `call_tool`/`read_resource` signatures
in `mcp_handler.rs`, and the `Meta`/`tool.meta = Some(Meta(...))` construction)
were verified against rmcp 2.2.0, which TASK-42 replaces with 3.1.x. TASK-42
changes those exact two `ServerHandler` methods (MRTR-aware return types) and
splits `Meta` into `MetaObject`/`RequestMetaObject`/`NotificationMetaObject`.

Do not start this ticket until TASK-42 is done. When picking this up, re-read
the current `mcp_handler.rs` and rmcp 3.x's `Tool`/metadata types before
following steps 2-3 above literally — the shape (a Tool-level `_meta.ui`
pointer built from whichever metadata type now applies) should still be
correct in spirit, but the exact Rust incantations in this plan predate the
upgrade.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented per the plan, adjusted for rmcp 3.1.2's actual API surface (verified against the installed crate source, not just the plan's rmcp-2.2.0-era notes):

- nom-core/src/widget/mod.rs: extracted `pub(crate) async fn widget_display_enabled(conn) -> Result<bool, ErrorData>` out of GetWidgetDisplay::execute_json (single source of truth for the settings-table read, now shared with mcp_handler.rs).
- nom-core/src/operation/mcp_handler.rs:
  - Added GOAL_PROGRESS_UI_RESOURCE_URI ("ui://nom-mcp/goal-progress"), GOAL_PROGRESS_WIDGET_HTML (include_str! of the new asset), and goal_progress_ui_meta() building an rmcp::model::MetaObject (rmcp 3.x renamed Tool.meta's type from the plan's assumed Meta to MetaObject; JsonObject = serde_json::Map<String, Value>, constructed via Map + .into()).
  - Pulled list_tools' body into a new pub(crate) async fn build_tools_gated(&self) -> Result<Vec<Tool>, rmcp::ErrorData>, mirroring the existing build_tools/dispatch_read_resource pattern, so tests can gate on widget_display_enabled without constructing a RequestContext<RoleServer> (not practically mockable — requires a live Peer<RoleServer>). The ServerHandler::list_tools trait method is now a one-line delegate.
  - Similarly pulled call_tool's body into pub(crate) async fn dispatch_call_tool(&self, name, arguments) -> Result<CallToolResult, ErrorData>, same rationale, used by the AC#3 byte-identical regression test.
  - Added a resources/read match arm for GOAL_PROGRESS_UI_RESOURCE_URI serving the static HTML with mime_type text/html;profile=mcp-app. Deliberately not added to build_resources()/list_resources() per the spec's UI-only-resources-via-tool-meta discovery convention.
  - get_goal_progress's Operation/execute_json is untouched, confirming the plan's key claim: all gating lives in mcp_handler.rs (MCP-only), never touching the CLI/HTTP-shared Operation.
- nom-core/assets/goal_progress_widget.html: new self-contained widget (inline CSS + vanilla JS, no build step, no external requests — required by the spec's restrictive default CSP of connect-src 'none' / script-src 'self' 'unsafe-inline' that applies whenever a UI resource declares no csp domains, which this one doesn't). Implements the ui/initialize handshake, ui/notifications/initialized, and renders on ui/notifications/tool-result by parsing the CallToolResult's content[0].text as the GoalProgress JSON. The handshake is the exact JSON-RPC-over-postMessage framing given directly in the ext-apps spec's own "you don't need an SDK to talk MCP with the host" reference snippet (specification/2026-01-26/apps.mdx, Transport Layer section) — the example repo's basic-server-vanillajs sample turned out to import the actual @modelcontextprotocol/ext-apps npm package and go through a Vite bundler, so it wasn't usable verbatim for a single self-contained HTML resource file; the spec's raw snippet is the ground truth this was built from instead. Also applies host theme/style variables from ui/notifications/host-context-changed and reports size via ui/notifications/size-changed.
- Tests added in mcp_handler.rs: widget-disabled default (meta None on get_goal_progress AND on an unrelated registered tool), widget-enabled (meta set on get_goal_progress only, unrelated tool still None), resources/read for the widget URI (mime type + non-empty HTML), and call_tool byte-identical serialization before/after flipping the setting. All via the new build_tools_gated/dispatch_call_tool inherent methods.
- CONTEXT.md and doc-5 §8 updated to describe get_goal_progress as the real consumer instead of 'no tool or Resource output branches on it yet'.

Verification: cargo test -p nom-core --lib (241 passed), cargo test --workspace (all green), cargo clippy --workspace --all-targets (clean, after fixing one collapsible-if in the new gating code).

DoD #1 (manual verification in an actual MCP-Apps-capable client, e.g. Claude Desktop): not verifiable in this autonomous environment — no such client is reachable here to connect to a locally-run nom-mcp server and visually confirm the widget renders. Per the DoD item's own wording ("or explicitly noted as unverifiable in this environment") and the referenced open issues (ext-apps#671, claude-ai-mcp#165) noting Claude Desktop's MCP Apps rendering is currently unreliable, this is recorded here rather than silently skipped. The server-side contract (tool meta shape, resource mime type/content, unchanged call_tool output) is fully covered by the automated tests above.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added the first MCP Apps UI widget (SEP-1865 / ext-apps 2026-01-26) for get_goal_progress, gated entirely on the existing widget_display_enabled setting: list_tools attaches _meta.ui.resourceUri only when enabled, resources/read serves a new self-contained text/html;profile=mcp-app widget (nom-core/assets/goal_progress_widget.html) for that URI, and call_tool's own JSON response is provably unchanged in both states. All gating lives in nom-core/src/operation/mcp_handler.rs (MCP-only); the goal Operation itself is untouched. CONTEXT.md and doc-5 updated; 241 nom-core tests + workspace tests + clippy all pass.
<!-- SECTION:FINAL_SUMMARY:END -->
