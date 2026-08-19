---
id: TASK-58
title: 'Food-added widget on log_meal: food macros + updated daily totals'
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-17 21:12'
updated_date: '2026-08-19 03:38'
labels:
  - widgets
  - planned
dependencies:
  - TASK-58.1
  - TASK-58.2
references:
  - docs/widget-design-reference-goals-cards.png
  - CONTEXT.md
priority: high
type: feature
ordinal: 64000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
When the user logs food, the LLM replies with text; the user wants a compact widget attached to that moment showing (a) the macros of the just-logged food/meal and (b) their updated daily totals after the addition.

Current gap: log_meal returns {meal_id, logged_at, logged_date, totals:{total_calories,total_protein_g,total_carbs_g,total_fat_g,total_fiber_g}} — no per-food breakdown and no daily context. This task extends the response additively (existing fields unchanged) with:

- Per-portion breakdown: food name + calories/protein/carbs/fat/fiber for each portion logged (respecting quantity mode and any adjustment)
- Post-insertion daily totals with per-nutrient progress (consumed/target/percent/status) — i.e., the same shape get_goal_progress returns for the logged_date

New widget: a static self-contained asset (alongside nom-core/assets/*.html) served at a new ui:// resource, with a _meta.ui pointer on the log_meal tool declaration gated exactly like the existing widget tools (widget_display_enabled flag + ui_blocked_clients suppression).

Visual (approved design language): compact panel, target <= ~150px tall at 320px width — logged food's macros as a compact list/chips on the left, updated daily totals as rings with percent-of-goal inside on the right (colors: under=blue, met=green, over=red). Reference for card/ring styling: docs/widget-design-reference-goals-cards.png.

Scope: log_meal only. update_meal is out of scope (note as follow-up if desired).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 log_meal's response gains a per-portion macro breakdown (food name + calories/protein/carbs/fat/fiber) and post-insertion daily totals with per-nutrient progress (consumed/target/percent/status); all existing response fields are unchanged.
- [x] #2 A new ui:// resource serves the self-contained widget rendering the logged food's macros next to updated daily-total rings; it appears in resources/list and is discoverable via the tool's _meta.ui.
- [x] #3 _meta.ui is suppressed when widget display is disabled or the requesting client is blocklisted — identical gating to the existing widget tools.
- [x] #4 Layout is compact (<= ~150px tall at 320px width), rings carry percent-of-goal, and light/dark theming works via the existing host-context mechanism.
- [x] #5 Tests cover: multi-portion meal response shape, adjustment handling, resource serving, and gating (enabled/disabled/blocked-client).
- [x] #6 Manual verification on a seeded local instance (TASK-54): logging a meal shows the widget alongside the text result.
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Orchestration plan — two sequential sub-tickets, both unplanned (each needs its own focused /backlog-planner session).

## Approach

TASK-58.1 (Rust core, ships first): extend log_meal's response additively with `portions` + `daily_totals`. Pure domain work in nom-core; all four surfaces (CLI/HTTP/MCP/remote) pick up the fields automatically through the registry. Shippable independently — useful to text-only LLM clients before any UI exists.

TASK-58.2 (UI + MCP surface, ships second): new self-contained widget asset `nom-core/assets/food_added_widget.html` plus mcp_handler wiring (ui:// resource,_meta.ui gating on log_meal, dispatch_read_resource arm). Consumes the response contract from .1. Depends on .1.

## Response contract (integration point between .1 and .2 — binding)

```
{ "meal_id": i64, "logged_at": str, "logged_date": "YYYY-MM-DD", "totals": { total_calories, total_protein_g, total_carbs_g, total_fat_g, total_fiber_g } }   // ALL UNCHANGED
+ "portions": [ PortionSummary ]     // id, food_id, food_name, quantity_mode, quantity, calories, protein_g, carbs_g, fat_g, fiber_g (same shape as MealSummary.portions)
+ "daily_totals": {                  // 5 x goal::NutrientProgress, Option fields omitted when null
    "calories" | "protein_g" | "carbs_g" | "fat_g" | "fiber_g":
      { consumed, target?, remaining?, percent?, direction?, status? }
  }
```

## Key decisions (binding on both sub-tickets)

1. `portions` is read back post-COMMIT via the existing `build_meal_summary(&conn, meal_id)` (take `.portions`). Do NOT change `resolve_portions()`'s signature (shared with UpdateMeal; it currently discards food names). Read-back guarantees response == stored state; the extra queries are negligible for a single-user local DB.
2. `daily_totals` comes from a NEW pub(crate) helper in `nom-core/src/goal/mod.rs`: `struct DailyNutrientProgress { calories, protein_g, carbs_g, fat_g, fiber_g: NutrientProgress }` + `async fn daily_nutrient_progress(conn, date) -> Result<DailyNutrientProgress, ErrorData>`, built on the existing private `fetch_active_goal` + `fetch_consumed_totals` and pub(crate) `nutrient_progress`. Extract GetGoalProgress's inline direction-parse closure into a shared function to avoid duplication. Do NOT refactor GetGoalProgress itself (it still needs the goal row for target_weight) — keep the diff focused.
3. Computed AFTER commit, on the SAME open connection (avoids a second advisory-lock probe), scoped to `logged_date` with the goal active as-of that date. Weight is excluded — logging a meal cannot change weight progress. Post-commit read failures propagate as storage_failure (codebase has no partial-success semantics anywhere; the meal is persisted either way).
4. No-goal case must match get_goal_progress exactly: `consumed` populated, target/remaining/percent/status null.
5. Widget ring colors are STATUS-based (under=blue, met=green, over=red) per the ticket spec. docs/widget-design-reference-goals-cards.png informs card/ring STYLING only (rounded cards, bold labels, thick round-cap rings) — its per-nutrient accent colors do not apply.
6. Gating is identical to the three existing widget tools: `widget_display_enabled` flag + `ui_blocked_clients` suppression in `build_tools_gated`; the ui:// resource is listed unconditionally like the others; served with WIDGET_HTML_TTL_MS (24h) + ui_contents_meta.

## Integration & verification (after both children done)

- Full CI mirror: `cargo nextest run --all-features --workspace` + `cargo test --doc --all-features --workspace`; `cargo fmt --all --check`; `cargo clippy --all-targets --all-features --workspace -- -D warnings`; `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --document-private-items --all-features --workspace --examples`.
- Manual (AC#6 of parent): seed a throwaway DB via the TASK-54 flow, `serve http`, connect an MCP Apps-capable client, log a multi-portion meal with an adjustment; verify the widget renders portions + daily-total rings alongside the text result, in light and dark, with correct size-changed reporting. Screenshot batches <= 4 images per prompt (LiteLLM cap — see AGENTS.md); split larger comparisons across prompts/subagents.
- update_meal stays OUT OF SCOPE (parent ticket says so); note as follow-up candidate if desired.

## Risks / considerations

- Post-commit read failure after a successful commit: propagated as storage_failure; accepted (data persisted, message honest).
- Concurrent same-date insert between commit and the daily_totals query: low risk under advisory-lock handoff semantics; accepted.
- Pixel budget: 5 labeled rings + portion list in <= ~150px at 320px width is tight — executor verifies visually and tunes ring sizes (expect 34-40px rings, 3x2 or 2x3 grid) and label sizes to fit.
- Test-fixture coupling: the mcp_handler gating tests use a shared fixture registering the three widget ops + TestOp; .2 must register LogMeal there (or a sibling fixture) and extend the enabled/disabled/blocked-client tests rather than duplicating them.
<!-- SECTION:PLAN:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Shipped in two sequential sub-tickets: TASK-58.1 extended log_meal's response additively with `portions` (per-portion food name + macros) and `daily_totals` (post-insertion daily totals in the get_goal_progress per-nutrient progress shape), and TASK-58.2 added the self-contained food_added_widget.html asset plus the MCP wiring serving it at ui://nom-mcp/food-added with_meta.ui gating identical to the existing widget tools. All 6 ACs verified: CI gate green (fmt, clippy -D warnings, nextest, doctests), functional e2e on the seeded local instance, visual review SHIP (320x139 at 320px width, all status colors and light/dark theming correct). Note: the execute step ran out of its 40-minute budget twice (implementation left uncommitted; finalization interrupted mid-ticket-bookkeeping) — work was resumed from the leftover tree and finalized manually.
<!-- SECTION:FINAL_SUMMARY:END -->
