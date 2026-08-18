---
id: TASK-57
title: 'Weight trend widget: sparkline + week-over-week delta'
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-17 21:12'
updated_date: '2026-08-18 20:33'
labels:
  - widgets
  - planned
dependencies:
  - TASK-54
references:
  - docs/widget-design-reference-goals-cards.png
  - CONTEXT.md
priority: medium
type: feature
ordinal: 63000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The user wants a weight-trend widget: a compact sparkline of recent weight entries plus a week-over-week delta, so trends are visible at a glance next to the LLM's text.

Shape: a new static self-contained asset (alongside nom-core/assets/*.html) served at a new ui:// resource, with a _meta.ui pointer on the relevant weight operation(s) — which exact operations get the pointer (e.g., get_weight_by_date_range, log_weight_entry) is decided at planning time. Gating identical to the existing widget tools (widget_display_enabled + ui_blocked_clients).

Visual (approved design language): ~80-100px tall at 320px wide; minimal axis-free sparkline of the last ~14-30 entries, signed week-over-week delta colored by direction relative to the target (progressing toward target = green, away = red; neutral when no target), optional dashed target line. Light/dark via the existing host-context mechanism; self-contained; size-changed reporting.

Data: weight entries are already stored and range-queryable (get_weight_by_date_range); no schema changes expected.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 At least one weight operation exposes a _meta.ui pointer to the new ui:// weight-trend resource when widget display is enabled, with the same blocklist suppression as the other widget tools.
- [x] #2 The widget renders a sparkline of the most recent weight entries plus a signed week-over-week delta, colored by direction relative to the target (neutral when no target is set).
- [x] #3 Sparse data degrades gracefully: 0 or 1 entries shows a short placeholder instead of a broken/empty chart.
- [x] #4 Self-contained (zero external requests), light/dark themed, size reporting intact.
- [x] #5 Tests cover resource serving, gating, and the delta computation; manual verification on a seeded local instance (TASK-54).
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Approach

New MCP-only `get_weight_trend` operation + new self-contained widget asset + new `ui://nom-mcp/weight-trend` resource, wired through the existing `_meta.ui` gating (`widget_display_enabled` + `ui_blocked_clients`). No sub-tickets: the op, the asset, and the handler wiring are one coupled increment — each part is inert without the others — matching how sibling widget features (TASK-55/56/58) shipped as single planned tickets.

### Why a new operation instead of a pointer on an existing weight op

MCP Apps widgets receive data only from the `call_tool` result bridge (the restrictive default CSP blocks all network access from the widget iframe). Pointing at `get_weight_by_date_range` would either break its bare-array response shape across all four surfaces (it cannot be extended additively), or leave the widget blind: the LLM chooses the query range (non-deterministic visual), the payload carries no target weight (dashed target line impossible), and the delta would have to be computed client-side in JS (untestable — AC#5 requires Rust-tested delta computation). Precedent: `get_weekly_progress` is MCP-only precisely because "MCP Apps widgets bind to a `call_tool` result, not a resource read" (CONTEXT.md). TASK-58's additive extension works on `log_meal` only because that response is already an object.

### 1. Operation: `get_weight_trend` (nom-core/src/weight/mod.rs)

- `pub struct GetWeightTrend { clock: Clock, #[cfg(test)] db_path }` with `new(clock)` / `with_db_path` — mirrors `GetWeightByDateRange`/`LogWeight`.
- `Surfaces::MCP` only (precedent: `get_weekly_progress`). Fieldless `struct GetWeightTrendRequest {}` (object schema — never a unit struct, per AGENTS.md).
- Query: most recent 30 weight entries (`ORDER BY logged_at DESC LIMIT 30`, returned ascending); active goal's `target_weight` via `SELECT target_weight FROM goals WHERE effective_from <= ? ORDER BY effective_from DESC LIMIT 1` with as-of = today (same pattern as the weekly/goal modules).
- Output shape:
  ```json
  {
    "entries": [{"logged_date": "YYYY-MM-DD", "value": 76.4}],
    "delta": {"value": -0.3, "reference_date": "2026-08-11", "movement": "toward_target"},
    "target_weight": 75.0
  }
  ```
  - `delta.value` / `delta.reference_date` are null when fewer than 2 entries exist.
  - Baseline rule: current = latest entry (max `logged_at`); baseline = entry with max `logged_at` among those whose `logged_date` <= current.logged_date - 7 days; if none exists, fall back to the earliest entry; with only one entry total, delta is null.
  - `movement` (deliberately NOT named "direction" — that term is reserved for the nutrient `Goal::Direction` enum): `"toward_target"` / `"away_from_target"` / `"neutral"`. Neutral when no target, when |current - target| < 1e-9 (same at-target tolerance as `goal::weight_progress`), or when delta == 0; otherwise sign(delta) == sign(target - current) => toward, else away. Works for both loss and gain goals — CONTEXT.md: weight progress is read directly off the comparison to the latest Weight Entry (no Direction involved).
  - Delta/movement math lives in a pure function (small input struct -> output struct) fully unit-testable without a DB; `execute_json` only does fetch + serialize.
- Register in `build_registry()` (nom-mcp/src/main.rs) next to the other weight ops.

### 2. Widget asset: nom-core/assets/weight_trend_widget.html

- Same boilerplate as the siblings: postMessage JSON-RPC bridge, `ui/initialize` handshake (protocolVersion "2026-01-26", appInfo name "nom-mcp-weight-trend-widget"), `host-context-changed` theming (colorScheme + host CSS variables), ResizeObserver -> `ui/notifications/size-changed`, `escapeHtml` on every interpolated value.
- Data arrives via the `tool-result` notification (parse `content[0].text` JSON); render dumbly from the payload — no client-side delta math.
- Visual (~320px wide, ~100px tall): header row with latest weight (bold) plus signed delta chip colored via light-dark() vars (green = toward target, red = away, muted = neutral); axis-free SVG sparkline below (createElementNS, viewBox scaling, polyline path from min/max-normalized points with padding, end-of-series dot, optional dashed target line placed by the same yFor normalization that includes the target in the min/max range — direct prior art: `weightChartSvg` in weekly_progress_widget.html).
- Sparse states (AC#3): 0 entries -> "No weigh-ins yet"; 1 entry -> show the value + "First weigh-in" (no chart); >=2 -> chart. Values displayed unit-less (stored as-is, per LogWeightRequest).
- Self-contained: inline CSS/JS only, zero external requests.

### 3. Handler wiring (nom-core/src/operation/mcp_handler.rs)

- `WEIGHT_TREND_UI_RESOURCE_URI = "ui://nom-mcp/weight-trend"` + `include_str!("../../assets/weight_trend_widget.html")` const + `weight_trend_ui_meta(domain)` fn (mirrors the goal/weekly pair; `domain` omitted unless configured — TASK-53 caveat).
- `build_tools_gated`: add a `"get_weight_trend"` arm to the tool-name match (gating unchanged: `widget_display_enabled && !client_blocked`).
- `build_resources`: add the third `ui://` Resource (title "Weight Trend", description "Weight trend sparkline widget (MCP Apps UI)", mime `text/html;profile=mcp-app`); update `test_build_resources_lists_all_resources` (len 3 -> 4 plus assertions for the new entry).
- `dispatch_read_resource`: static arm mirroring the other two (WIDGET_HTML_TTL_MS, CacheScope::Public, `ui_contents_meta`).

### 4. Tests

- Unit (pure delta/movement function): 0 entries; 1 entry; 2 entries within 7 days (fallback-to-earliest baseline); exact 7-day match; no exact match (nearest <= D-7d); loss-goal toward/away; gain-goal toward/away; at-target -> neutral; no target -> neutral; zero delta -> neutral; 30-entry cap.
- Integration (`execute_json` against `TempDb`): end-to-end payload shape including the target join; multiple entries on the same date.
- mcp_handler: read-resource serves the HTML (mime `text/html;profile=mcp-app`, contains `<!DOCTYPE html>` and `ui/notifications/size-changed`); gating adds `_meta.ui` to `get_weight_trend` when enabled, omits it by default/disabled, suppresses it for blocked clients, and leaves non-widget tools untouched (extend the existing test fixtures/tests to register `GetWeightTrend`).

### 5. Docs + manual verification

- CONTEXT.md: add a one-line "Weight Trend" glossary entry (compact sparkline of recent Weight Entries + signed week-over-week delta relative to the target weight; surfaced via the `get_weight_trend` MCP tool bound to the weight-trend widget).
- Manual (AC#5), README "Local dev instance" flow: `nom-mcp seed_data --path /tmp/nom-dev/nom.db` (seeds 7 weight entries, a target weight, and widget display enabled) -> `NOM_MCP_DB_PATH=/tmp/nom-dev/nom.db nom-mcp serve http --port 8000` -> connect a widget-capable MCP client -> call `get_weight_trend`: expect sparkline + colored delta chip; then `set_widget_display false` -> pointer gone from `tools/list`. Optionally log an extra entry dated >=7 days before the latest to exercise the true 7-day baseline (the seed alone exercises the fallback-to-earliest path).
- Full CI gate: fmt, clippy `-D warnings`, nextest `--all-features`, doctests, rustdoc `-D warnings`.

### Acceptance mapping

- AC#1 -> section 3 (pointer on get_weight_trend + identical blocklist suppression)
- AC#2 -> section 1 (server-computed delta + movement) + section 2 (sparkline render, direction-aware colors)
- AC#3 -> section 2 sparse states + section 4 unit cases
- AC#4 -> section 2 (self-contained, themed, size reporting) + read-resource test
- AC#5 -> sections 4 + 5
<!-- SECTION:PLAN:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Weight-trend widget shipped end-to-end:

- New MCP-only `get_weight_trend` operation (nom-core/src/weight/mod.rs): most recent 30 Weight Entries (ascending), server-computed week-over-week delta (baseline = latest entry on/before D-7d, fallback to earliest; null below 2 entries), and the active target weight. `movement` classifies toward_target / away_from_target / neutral (works for loss and gain goals; neutral when no target, at-target tolerance, or zero delta). Delta/movement math is a pure, fully unit-tested function; integration tests cover payload shape, target join, same-date entries, and the 30-entry cap.
- New self-contained widget asset (nom-core/assets/weight_trend_widget.html): axis-free SVG sparkline with end-of-series dot, bold latest value + signed delta chip (green = toward target, red = away, muted = neutral), optional dashed target line, sparse states ("No weigh-ins yet" / "First weigh-in"), light/dark theming via host-context, size-changed reporting. Zero external requests.
- Handler wiring (nom-core/src/operation/mcp_handler.rs): `ui://nom-mcp/weight-trend` resource + `_meta.ui` pointer on `get_weight_trend` with gating identical to the other widget tools (widget_display_enabled + ui_blocked_clients suppression); resource-serving, gating, and dispatch tests extended.
- Registered in build_registry() (nom-mcp/src/main.rs); CONTEXT.md glossary entry added.

Verification: full CI gate green (fmt, clippy -D warnings, nextest 348/348, doctests, rustdoc -D warnings). Functional e2e on a seeded local instance via curl: tools/list gating on/off/blocked-client, resources/read (mime + content), call_tool payload incl. a true 7-day baseline after logging an old entry. Visual verification of all 5 rendered states (light, dark, toward-target green, away-target red, sparse-0, sparse-1) was delegated to vision subagents in ≤4-image batches per the vLLM image cap: every check PASS, overall verdict SHIP (two low-severity nits noted, none blocking: placeholder vertical balance; unit-less values are per spec).
<!-- SECTION:FINAL_SUMMARY:END -->
