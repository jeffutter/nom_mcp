---
id: TASK-56
title: 'Compact weekly-progress widget: shrink charts & tighten spacing'
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-17 21:12'
updated_date: '2026-08-18 05:26'
labels:
  - widgets
  - planned
dependencies:
  - TASK-54
references:
  - CONTEXT.md
priority: medium
type: enhancement
ordinal: 62000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The weekly-progress widget (nom-core/assets/weekly_progress_widget.html, served at ui://nom-mcp/weekly-progress) stacks a 140px calorie bar chart and a 100px weight line chart under a title/subtitle and section dividers — roughly 350px tall. It should be more vertically compact since LLM text accompanies it.

Approved approach (user decision): shrink & tighten — keep both charts (calorie bars with dashed goal line; weight line with dashed target line) but cut chart heights by ~40% (~90px and ~70px), and tighten body padding, typography, and section spacing. Target overall height ~200px at 320px width. No structural change: same data, same sections, same interactions (per-bar <title> tooltips, day-of-week labels, weight summary row with Start/End/delta + status chip).

Same constraints as the goal-progress redesign: self-contained HTML, light-dark() + host CSS variables, size-changed reporting, XSS-safe escaping, widget_display_enabled + ui_blocked_clients gating unchanged, existing mcp_handler tests pass.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Rendered height reduced >=35% vs the current widget at equivalent width, while retaining both charts, the goal/target reference lines, and the weight summary row.
- [x] #2 Per-bar hover tooltips (date + calories) and day-of-week labels remain.
- [x] #3 Light/dark + host-variable theming, self-containment, and size reporting all intact.
- [x] #4 Existing mcp_handler resource/gating tests pass unmodified or extended.
- [x] #5 Manual visual verification on a seeded local instance (TASK-54) in both light and dark mode.
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
# Implementation plan — TASK-56: compact weekly-progress widget

## Scope: single file, no sub-tickets

All changes land in `nom-core/assets/weekly_progress_widget.html` (embedded via `include_str!` at `mcp_handler.rs:62`, served as the `ui://nom-mcp/weekly-progress` resource). Chart shrinking and spacing tightening are inseparable — the height AC needs both together — and the diff is ~30 lines, so this is one atomic change; no sub-tickets. No Rust changes required: gating (`widget_display_enabled`, `ui_blocked_clients`), resource registration, `_meta.ui`, TTLs, and theming all live in mcp_handler/config and stay untouched.

## Baseline geometry (verified against source 2026-08-18)

At a 320px viewport width (content = 288px after 16px body padding), `.chart { width:100%; height:auto }` scales each SVG by its viewBox ratio: calorie chart renders ≈126px (viewBox 320×140), weight chart ≈90px (viewBox 320×100); total ≈390–400px (the ticket's ~350px estimate is close but measure the real baseline per Verification step 2 — the AC gate is relative: ≥35% reduction).

## Changes

### CSS (in the `<style>` block)

| Selector | Current | New |
|---|---|---|
| `body` | `padding: 16px` | `padding: 10px` |
| `h1` | `font-size: 15px; margin: 0 0 2px 0` | `font-size: 14px; margin: 0 0 1px 0` |
| `.subtitle` | `font-size: 12px; margin: 0 0 14px 0` | `font-size: 11px; margin: 0 0 8px 0` |
| `.section-title` | `font-size: 11px; margin: 16px 0 8px 0; padding-top: 10px` | `font-size: 10px; margin: 8px 0 4px 0; padding-top: 4px` (keep `border-top: 1px solid var(--border)`; keep the `:first-of-type` override as-is) |
| `.bar-label` | `font-size: 9px` | `font-size: 10px` (research finding: smaller bars need slightly larger labels for scanability; 7 slots × ~43 viewBox units fit 3-char labels at 10px with room) |
| `.summary-row` | `font-size: 13px; margin-top: 8px` | `font-size: 12px; margin-top: 6px` |
| `.weight-status` | `font-size: 11px; padding: 1px 6px` | `font-size: 10px; padding: 1px 5px` |

### JS chart geometry

- `caloriesChartSvg`: `height` 140 → **80**, `padTop` 8 → **4**, `padBottom` 20 → **16** (data area 60px; day-of-week labels stay outside the data area at `y = height - 4`).
- `weightChartSvg`: `height` 100 → **56**, `padTop` 10 → **4**, `padBottom` 10 → **4** (data area 48px).
- Minimum bar height (research-flagged risk): replace `Math.max(0, barH)` on the rect's `height` attribute with `v > 0 ? Math.max(1, barH) : 0` — zero-calorie days stay invisible (correct: nothing logged), non-zero days render ≥1px so their `<title>` tooltip remains hoverable.
- If the measured result lands well above ~250px and closer to the ~200px target is wanted, drop chart heights further (calories 72 / weight 50 keeps data areas ≥42px). Do not go below that without re-checking label legibility.

### Expected height (at 320px viewport, content 300px)

20 (padding) + ~19 (h1) + ~21 (subtitle) + ~16 (first section title) + 75 (calorie SVG 300×80/320) + ~29 (second section title incl. border) + ~52 (weight SVG 300×56/320) + ~20 (summary row) ≈ **~250px vs ~400px current → ~38% reduction**. Comfortably clears the ≥35% gate (AC#1) and lands near the ~200px target.

## Invariants — do not touch (AC#2/#3)

- Per-bar `<title>` tooltips (date + calories) and day-of-week labels — SVG-embedded, unaffected by height.
- Both dashed reference lines (`.goal-line`, `.weight-target-line`) and the weight summary row (Start/End/Δ + status chip).
- JSON-RPC handshake (`ui/initialize` → `initialized`), all `ui/notifications/*` handlers, and `reportSize()`/ResizeObserver `size-changed` reporting — it adapts to the new dimensions automatically; no change needed.
- Theming: `light-dark()` variables, host-variable overrides (`applyHostContext`), `color-scheme` meta.
- XSS safety: `escapeHtml` on every server-derived field; self-containment: inline CSS/JS only (CSP `script-src 'self' 'unsafe-inline'`), no external requests.
- `renderError`/placeholder paths and `extractWeeklyProgress` parsing.

## Tests (AC#4)

Existing mcp_handler tests pass unmodified — they assert URI/title/mime/gating, not HTML internals (`test_dispatch_read_resource_weekly_progress_widget` checks mime + non-empty + `<!DOCTYPE html>` only). Optional hardening (recommended, 1 line): extend that test to also assert `text.contains("ui/notifications/size-changed")` so a future edit cannot silently break the transport layer.

## Verification

1. Gates: `cargo fmt --all --check`, `cargo clippy --all-targets --all-features --workspace -- -D warnings`, `cargo nextest run --all-features --workspace`, `cargo test --doc --all-features --workspace`, `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --document-private-items --all-features --workspace --examples` (HTML-only change, but run the full suite per project convention).
2. **Height measurement (AC#1)** — throwaway harness (do NOT commit): a page that iframes the widget at exactly 320px wide, answers the iframe's `ui/initialize` request with `{ id, result: { hostContext: { theme: "light" } } }`, posts `ui/notifications/tool-result` with params `{ content: [{ text: "<weekly progress JSON>" }] }`, and logs the `ui/notifications/size-changed` notification — that is the authoritative rendered size. Run it against the committed baseline (before editing) and the edited asset, with both `theme: "light"` and `"dark"`, record both numbers; require new ≤ 0.65 × baseline.
   Payload source: seed a dev DB (step 3 flow) and call `get_weekly_progress` over the running server's `/mcp` streamable-HTTP endpoint — it is `Surfaces::MCP`-only, so there is no CLI/REST route. curl recipe: POST initialize to `/mcp` (headers `Accept: application/json, text/event-stream`; capture the `Mcp-Session-Id` response header), then `tools/call {"name":"get_weekly_progress","arguments":{}}` with that session header; the SSE `result` is the toolResult whose `content[0].text` holds the JSON string. Any valid weekly-progress JSON works for layout purposes.
3. **Manual visual check (AC#5)** — README "Local dev instance with one-command seed data" flow: `nom-mcp seed_data --path /tmp/nom-dev/nom.db` → `NOM_MCP_DB_PATH=/tmp/nom-dev/nom.db nom-mcp serve http --port 8000` → connect a real MCP client to `http://localhost:8000/mcp` (or use the harness). Verify in light AND dark: both charts present with their reference lines, hover tooltips work, day labels legible and non-overlapping, summary row intact, no clipping/overflow. **Cache gotcha:** the `ui://` resource advertises `WIDGET_HTML_TTL_MS` = 24h (Public) — a client may serve stale pre-change HTML during verification; use a fresh client profile/session (or the harness) for the post-change check.

## Risks / gotchas

- Day-label overlap: 7 labels × 10px across ~304 viewBox units = ~43 units/slot; 3-char labels ≈ 19 units — safe, but re-verify visually (AC#5).
- Single-point weight chart: circle r=3 inside a 48px data area is fine; sizing logic is data-count-independent and unchanged.
- Do not "helpfully" refactor the JSON-RPC block, escaping, or host-context code while touching neighbors — those are load-bearing (TASK-53 history: iOS broke on subtle UI-meta changes).
- Keep the file CSP-safe: no new external references, no `eval`, inline script only.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
## Implementation notes (2026-08-18, execution run)

### What landed
Single-file change to `nom-core/assets/weekly_progress_widget.html` per plan, using the plan's further-shrink geometry (calories viewBox 320×72, weight 320×50; data areas 52px/42px):
- CSS: body padding 16→10px; h1 15→14px (margin-bottom 2→1px); .subtitle 12→11px (margin-bottom 14→8px); .section-title 11→10px (margin 16/8→8/4, padding-top 10→4); .bar-label 9→10px; .summary-row 13→12px (margin-top 8→6px); .weight-status 11→10px (padding 1px 6px→1px 5px).
- JS: caloriesChartSvg height 140→72, padTop 8→4, padBottom 20→16, day labels at y=height-4; weightChartSvg height 100→50, padTop/padBottom 10→4.
- Min-bar fix: rect height `v > 0 ? Math.max(1, barH) : 0` — zero-calorie days stay invisible, non-zero days ≥1px so their <title> tooltip stays hoverable.
- Test hardening (plan's recommended option): `test_dispatch_read_resource_weekly_progress_widget` now also asserts the served HTML contains `ui/notifications/size-changed`, guarding the iframe-sizing transport against silent loss.

No Rust behavior changes: gating, resource registration, _meta.ui, TTLs, theming, escaping, handshake all untouched.

### Verification evidence
- **AC#1 height** — throwaway harness (/tmp/widget-harness, not committed): page iframing the widget at 320px wide (iframe seeded at 50px tall so documentElement.scrollHeight reflects content), answering ui/initialize with hostContext.theme, posting ui/notifications/tool-result, logging ui/notifications/size-changed. Ran headless Chromium (virtual-time) against `git show HEAD:` baseline vs the edited asset, light AND dark, 2 runs each; final size stable across runs:
  - baseline: 350px (light & dark)
  - new: 217px (light & dark)
  - reduction = 38.0% (≥35% gate ✓); 217 ≤ 0.65×350 = 227.5 ✓; near the ~200px target.
  - Note: measured width reads 305 during capture because a 15px vertical scrollbar appears while the 50px-tall iframe overflows; identical for both variants, so the height comparison is unaffected (post-resize width is 320).
- **Payload** — real, not synthetic: seeded via `seed_data --path /tmp/nom-dev/nom.db`, served `serve http --port 8000`, pulled `get_weekly_progress` over `/mcp` streamable-HTTP (initialize → tools/call, SSE result content[0].text). 7 days with meals, cal target 2000 (direction "target"), weight start 80.5 / end 78.0 / target 80.0 / status under.
- **AC#2** — per-bar <title> tooltips ("YYYY-MM-DD: N cal") and day-of-week labels unchanged in code; visible in screenshots; min-bar fix keeps shrunk bars hoverable.
- **AC#3** — dark-mode screenshot confirms light-dark() + host theme application; no external refs added (diff is CSS values + SVG geometry only); size reporting exercised directly by the harness (it is how the heights above were captured).
- **AC#4** — full suite green after the test extension: fmt ✓, clippy -D warnings ✓, nextest 329/329 ✓, doctests ✓, rustdoc -D warnings ✓.
- **AC#5** — visual check on the seeded instance in headless Chromium at 320px, light AND dark (screenshots shot_baseline_*.png / shot_new_*.png in /tmp/widget-harness): both charts present with dashed goal/target lines, day labels legible and non-overlapping, summary row (Start 80.5 / End 78 / Δ −2.5 / UNDER chip) fully visible, no clipping or overflow. Dev server stopped and /tmp/nom-dev removed afterwards.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Weekly-progress widget is now vertically compact: 217px rendered height at 320px width vs 350px baseline (−38%, measured via the widget's own size-changed reports in headless Chromium, light and dark, real seeded payload from get_weekly_progress over /mcp). Both charts retained with their dashed goal/target reference lines, per-bar hover tooltips, day-of-week labels, and the weight summary row; theming/self-containment/gating untouched. One-line test hardening guards size reporting in mcp_handler. All gates green (fmt, clippy -D warnings, nextest 329/329, doctests, rustdoc).
<!-- SECTION:FINAL_SUMMARY:END -->
