---
id: TASK-55
title: 'Redesign goal-progress widget: compact ring row + expanded card variant'
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-17 21:12'
updated_date: '2026-08-18 16:29'
labels:
  - widgets
  - planned
dependencies:
  - TASK-54
references:
  - docs/widget-design-reference-goals-cards.png
  - CONTEXT.md
priority: medium
type: enhancement
ordinal: 61000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The goal-progress widget (nom-core/assets/goal_progress_widget.html, served at ui://nom-mcp/goal-progress) is a vertical stack: title, date subtitle, five nutrient rows (label + 'consumed / target' + 6px bar), and a Weight section — roughly 300px tall. Since the LLM usually accompanies the widget with explanatory text, it needs to be significantly more vertically compact.

Approved design (user decision, 2026-08):

- DEFAULT: a single row of small rings — one per nutrient (calories, protein, carbs, fat, fiber), percent-of-goal centered inside each ring, nutrient label below, and weight (latest -> target + status) plus fasting hours in a slim footer line. Target ~100-120px tall at 320px wide.
- EXPANDED: a 2x2 card grid in the style of the reference screenshot docs/widget-design-reference-goals-cards.png (label + 'N under/over' delta on the left, progress ring on the right). Shown when the user explicitly asks for a fuller daily summary; the mechanism (optional argument on get_goal_progress vs a sibling operation) is decided at planning time, but both variants must be reachable from the LLM via the tool's schema/description without breaking existing clients.

Design language (applies to all widget work this round):

- Status colors: under = blue accent, met = green, over = red (existing --under/--met/--over tokens)
- Percent-of-goal inside rings; exact numbers in labels/footer
- Self-contained HTML only (host CSP blocks external requests); light-dark() + host-provided CSS variables; size-changed reporting preserved

Data source is unchanged: get_goal_progress already returns per-nutrient {consumed, target, remaining, percent, status, direction}, weight {latest_weight, target_weight, status}, and fasting_hours.

Must preserve: XSS-safe escaping of all interpolated values, widget_display_enabled gating on _meta.ui, ui_blocked_clients suppression, and existing mcp_handler.rs test coverage (extend as needed).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Default get_goal_progress rendering is a single row of rings <= ~120px tall at 320px width: percent-of-goal inside each ring, nutrient label below, all 5 nutrients present.
- [x] #2 Nutrients without a target degrade gracefully (neutral ring or no fill, consumed value still shown).
- [x] #3 Weight (latest vs target + status) and fasting hours render in a compact footer line, hidden gracefully when absent.
- [x] #4 An expanded 2x2 card variant matching the reference style is available when the user explicitly asks for a fuller daily summary, selectable through the tool's input schema/description without breaking existing clients.
- [x] #5 Ring colors follow status (under/met/over) and adapt to light/dark themes via the existing host-context mechanism.
- [x] #6 Widget stays self-contained (zero external requests), keeps size-changed reporting, and existing mcp_handler gating/blocklist/XSS tests still pass.
- [x] #7 Manual visual verification on a seeded local instance (TASK-54) in both light and dark mode.
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
# Implementation plan — TASK-55: goal-progress widget redesign (compact ring row + expanded card variant)

## Design decisions (locked at planning time)

1. **Variant mechanism: optional `variant` argument on `get_goal_progress` — NOT a sibling operation.** Both variants return identical data; only presentation differs. A sibling op would duplicate the registry entry and force either a second resource URI or a second HTML file duplicating ~150 lines of postMessage/handshake boilerplate. An additive enum argument keeps one tool name, one `ui://nom-mcp/goal-progress` resource, one HTML file. Existing clients (no arg) get the default; the LLM selects `expanded` via the input schema/description.
2. **Response echoes the resolved `variant`** as an additive top-level field (`"compact"` | `"expanded"`). The widget branches on it inside `render(data)` without depending on host-specific `tool-input` notification ordering (the widget has a stub `tool-input` handler; do not rely on it). Older servers without the field → widget defaults to compact. Verified: all existing goal/mod.rs and mcp_handler tests assert specific keys, never the exact key set, so the additive field breaks nothing.
3. **Rings: inline SVG `<circle pathLength="100">` + `stroke-dasharray="{fill} 100"`** — dash values map directly to percentages, no circumference math. Start at 12 o'clock via `transform="rotate(-90 cx cy)"`. Status colors via CSS classes referencing the existing `--under/--met/--over` tokens (light-dark() + host-provided variables keep working). Over-100%: cap the arc at a full ring, show the true percent number (e.g. "120%") in the over color. No external library — consistent with the vanilla-JS / zero-external-request constraint.
4. **Compact (default) layout:** drop the standalone h1; slim date header line; one flex row of 5 equal segments (ring ~44px, percent-of-goal centered inside, 9–10px nutrient label below); slim footer line with weight (latest → target + status chip) and fasting hours; body padding 12px. Target ≤ ~120px tall at 320px wide.
5. **Expanded layout:** same date header; CSS grid `repeat(2, minmax(0, 1fr))` cards, one card per nutrient THAT HAS A TARGET (skip no-target nutrients here, unlike compact which always shows all 5; placeholder when none have targets). Card: soft rounded background, bold label left, muted delta subtitle ("684 under" / "12 over" / "met"), status-colored donut (~40px) on the right — matching docs/widget-design-reference-goals-cards.png. With 5 targeted nutrients the 5th card wraps to a third row (grid auto-flow). Same shared footer line as compact.
6. **Degradation rules (both variants):** no target → neutral track ring (no colored arc) and the consumed value shown in place of the percent inside the ring (compact) / nutrient omitted from the grid (expanded); `percent == null` with a target present (zero-target guard) → same consumed-value fallback. Weight/fasting absent → footer hidden gracefully. Every interpolated string continues to pass through `escapeHtml()`.

## Files

- EDIT `nom-core/src/goal/mod.rs` — new `GoalProgressVariant` enum; `variant` on request + response; `description()` update; 3 unit tests
- REWRITE the rendering section of `nom-core/assets/goal_progress_widget.html` (postMessage JSON-RPC block, handshake, `applyHostContext`, `reportSize` + ResizeObserver, `extractGoalProgress` stay intact)
- NO changes to `nom-core/src/operation/mcp_handler.rs`, `nom-core/src/widget/mod.rs`, or `nom-mcp/src/main.rs` — gating/blocklist/resource-read paths are untouched; their tests must pass unmodified

## Operation spec (goal/mod.rs)

```rust
/// Presentation variant for the goal-progress widget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum GoalProgressVariant { Compact, Expanded }
```

- `GetGoalProgressRequest`: add `pub variant: Option<GoalProgressVariant>` — doc comment: "Widget presentation variant: 'compact' (default, single row of rings) or 'expanded' (fuller card-grid daily summary). The data returned is identical." An unknown string fails serde deserialization → the existing `Validation("request", ...)` mapping handles it; no new error path.
- `GoalProgress` (response struct): add `variant: GoalProgressVariant` (resolved value; always serialized, lowercase).
- `execute_json`: `let variant = req.variant.unwrap_or(GoalProgressVariant::Compact);` and include it in the response construction.
- `description()`: append: " Accepts an optional `variant` ('compact' | 'expanded', default 'compact') selecting how the widget renders the result: 'compact' is a single row of small rings; 'expanded' is a fuller card-grid daily summary. Use 'expanded' when the user asks for a fuller view."

## Widget spec (goal_progress_widget.html)

Keep verbatim: DOCTYPE/meta, `:root` tokens (extend with e.g. `--card-bg: light-dark(#ffffff, #1f1f1f)`), the entire postMessage JSON-RPC block, `applyHostContext`, `reportSize` + ResizeObserver, `extractGoalProgress`, `NUTRIENTS`, `fmt`, `escapeHtml`, handshake/lifecycle handlers.

Replace the rendering section with:

- `ringSvg(size, fillPct, cls)` — track circle + status-colored fill circle, `pathLength="100"`, `stroke-dasharray="{Math.max(0, Math.min(100, fillPct))} 100"`, `stroke-linecap="round"`, `transform="rotate(-90 size/2 size/2)"`; omit the fill circle when `fillPct` is null. CSS: `.ring-fill.status-under { stroke: var(--under); }` (same for met/over).
- `centerText(p, unit)` — `p.percent != null ? Math.round(p.percent) + "%" : fmt(p.consumed) + unit` (covers no-target and zero-target cases).
- `deltaText(p, unit)` — remaining null → ""; |remaining| < 1e-9 → "met"; positive → `fmt(remaining) + unit + " under"`; negative → `fmt(Math.abs(remaining)) + unit + " over"`.
- `footerHtml(data)` — weight segment (Latest X → Target Y + existing status-chip styling) plus "· Fasting {fmt(hours)}h"; returns "" when both are absent.
- `renderCompact(data)` — date header line + 5-segment flex row (each segment: position:relative ring wrapper with absolutely-centered percent span, label below) + footer.
- `renderExpanded(data)` — date header + 2-column grid of cards (only nutrients with `target != null`; each card: label, delta subtitle, right-aligned ~40px ring) + footer; placeholder when no nutrient has a target.
- `render(data)` — `data && data.variant === "expanded" ? renderExpanded(data) : renderCompact(data)`; keep the `reportSize()` call after the innerHTML swap.
- Sizing: body padding 12px; compact rings 44px / labels 9px / footer 11px → ≤ ~120px @ 320px width; expanded cards ~64px tall → ~220–260px total.

## Tests

New (goal/mod.rs, following the existing GetGoalProgress TempDb test pattern):

1. `test_get_goal_progress_variant_defaults_to_compact` — empty args → `result["variant"] == "compact"`
2. `test_get_goal_progress_variant_expanded_echoed` — `{"variant":"expanded"}` → `"expanded"`
3. `test_get_goal_progress_invalid_variant_rejected` — `{"variant":"huge"}` → Err with category Validation

Unchanged and must still pass: all 21 mcp_handler tests (widget-display gating, blocklist suppression, resource reads incl. the `<!DOCTYPE html>` assertions, byte-identical `call_tool` content, ui_meta domain), all existing goal/mod.rs tests, and `nom-mcp/tests/seed_e2e.rs` (drives get_goal_progress via CLI — additive field is harmless).

## Verification gates

`cargo fmt --all --check` · `cargo clippy --all-targets --all-features --workspace -- -D warnings` · `cargo nextest run --all-features --workspace` · `cargo test --doc --all-features --workspace` · `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --document-private-items --all-features --workspace --examples`

## Manual visual verification (AC#7, uses the TASK-54 seed instance)

> **Run this ENTIRE section in a FRESH subagent (clean context).** The 'coding' model group rejects any prompt containing more than 4 images (LiteLLM 400 "At most 4 image(s) may be provided"), and screenshots accumulate in the context of the session that takes them — once that session holds 5+, EVERY subsequent LLM call in it 400s and the run dies (this killed six prior execute attempts for this ticket). The delegating session must not attach or re-read these screenshots; it receives findings as TEXT only. Screenshot budget: exactly 4 total — compact/expanded × light/dark. Never take a 5th (no zoom crops, retries, or extra dates).

1. `nom-mcp seed_data --path /tmp/nom-dev/nom.db` (local CLI uses clap `--key value` flags, not `key=value` — see TASK-59)
2. `NOM_MCP_DB_PATH=/tmp/nom-dev/nom.db nom-mcp serve http --port 8000`
3. Point an MCP Apps-capable client at `http://localhost:8000/mcp`
4. Call `get_goal_progress` (no args) → compact ring row: all 5 rings, percent inside each, labels below, weight+fasting footer, ≤ ~120px @ 320px wide, status colors correct (seed data spans under/met/over; fiber is exactly "met")
5. Call `get_goal_progress` with `{"variant":"expanded"}` → 2×2 card grid matching the reference screenshot style (bold label + "N under/over" delta + status-colored ring on the right)
6. Repeat in dark mode (host theme) — tokens adapt via light-dark()
7. `rm -rf /tmp/nom-dev`

## Gotchas

- Do not touch the IPC/handshake block — TASK-53's iOS-load-error regression history lives there; the redesign is rendering-only.
- `pathLength="100"` + `stroke-dasharray` is the whole trick; do not compute circumferences manually.
- Keep `escapeHtml()` on every interpolation, including the new percent/delta strings (numbers are safe but stay uniform).
- `percent` can exceed 100 (e.g. 120) — cap the arc at 100, display the true number.
- The zero-target guard yields `percent: null` with a target present — handled by the `centerText` fallback.
- Seed-data fiber is integer-exactly "met" — a ready-made met-state test case in both variants.
- AC#7 visual check MUST run in a fresh subagent with ≤4 screenshots: >4 images in one session context → LiteLLM 400 on every subsequent call (systemic failure mode, see the note above the verification steps).
- The weekly widget (TASK-56) shares this design language; keep the ring helper generic enough to reuse later, but do NOT refactor weekly_progress_widget.html in this ticket.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
## Implementation notes (2026-08-18, execution run)

### What landed
Per the locked plan: additive `GoalProgressVariant` enum (compact|expanded, lowercase serde) in nom-core/src/goal/mod.rs; `variant: Option<GoalProgressVariant>` on GetGoalProgressRequest and resolved `variant` echoed in the GoalProgress response; description() documents the arg. Widget HTML rewritten to the compact ring-row default + expanded 2-column card grid, branching on the echoed variant field (older servers without it render compact). IPC/handshake, applyHostContext, reportSize+ResizeObserver, extractGoalProgress all untouched. No changes to mcp_handler.rs/widget/mod.rs/main.rs.

### Verification evidence
- **Gates**: fmt ✓, clippy -D warnings ✓, nextest 332/332 ✓ (incl. 3 new variant tests: default compact, expanded echo, invalid variant → Validation), doctests ✓, rustdoc -D warnings ✓.
- **AC#4 e2e**: rebuilt binary, restarted seeded instance (NOM_MCP_DB_PATH=/tmp/nom-dev/nom.db, port 8000); MCP tools/list inputSchema exposes variant as oneOf const compact/expanded with doc text; tools/call {} → variant=compact; tools/call {variant:expanded} → variant=expanded with byte-identical data; variant=huge → validation error 'unknown variant'. resources/read ui://nom-mcp/goal-progress served HTML byte-identical to repo asset.
- **AC#1/#3/#5 (DOM, headless Chromium via throwaway harness /tmp/widget-harness, real seeded payload)**: compact reports 320x120 via its own size-changed notification (exactly at the ~120px target); 5 rings, percents 92/115/57/103/100 centered inside, labels below; footer 'Wt 78 → 80' + UNDER chip; '· Fasting 14.5h' segment renders when present; footer element absent (height drops 120→98px) when weight+fasting both missing. Computed strokes: light #2563eb/#dc2626/#16a34a, dark #60a5fa/#f87171/#4ade80 — exact token values under color-scheme from hostContext; over-100% arcs capped at full ring while showing true percent.
- **AC#2**: no-target nutrient → track-only ring + consumed value ('172.9g') centered; zero-target guard (target=0, percent omitted) → same fallback ('30g'); expanded variant omits no-target nutrients (4 cards) and shows placeholder only when none have targets.
- **AC#6**: grep confirms zero external refs (no http(s)/link/script-src/fetch/XHR/import); size-changed reporting exercised directly by the harness (it is how all heights were measured); all 21 mcp_handler gating/blocklist/resource tests pass unmodified.
- **AC#7**: fresh subagent (clean context, exactly 4 image reads) re-shot compact/expanded x light/dark against the current code (harness widget copy verified byte-identical to repo asset) and passed all 11 checklist items with no defects; artifacts verify-*.png in /tmp/widget-harness. Dev server stopped and /tmp/nom-dev removed afterwards.
<!-- SECTION:NOTES:END -->
