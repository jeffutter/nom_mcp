---
id: TASK-58.2
title: Add food-added widget asset and MCP _meta.ui wiring for log_meal
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-19 01:51'
updated_date: '2026-08-19 03:38'
labels:
  - task
dependencies:
  - TASK-58.1
parent_task_id: TASK-58
priority: high
ordinal: 68000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
New self-contained MCP Apps widget asset plus the MCP-surface wiring that gates and serves it. Depends on TASK-58.1 (the log_meal response must carry 'portions' + 'daily_totals' first).

Asset: nom-core/assets/food_added_widget.html, mirroring goal_progress_widget.html exactly — inline JSON-RPC-over-postMessage scaffold, ui/initialize handshake -> initialized, data from ui/notifications/tool-result via toolResult.content[0].text JSON.parse, host-context-changed theming (colorScheme + CSS variable overrides), ResizeObserver -> ui/notifications/size-changed, light-dark() CSS custom properties with the existing palette (--under blue / --met green / --over red), SVG donut rings using pathLength='100' so stroke-dasharray maps directly to percent. appInfo name 'nom-mcp-food-added-widget'. Fully self-contained (restrictive CSP: no external fetches). All interpolated strings escaped (escapeHtml pattern).

MCP wiring in nom-core/src/operation/mcp_handler.rs, following the three existing widgets:
- const FOOD_ADDED_UI_RESOURCE_URI = "ui://nom-mcp/food-added" + FOOD_ADDED_WIDGET_HTML via include_str!
- fn food_added_ui_meta(domain) through the shared ui_meta() helper
- build_tools_gated(): add a 'log_meal' match arm setting .meta = Some(food_added_ui_meta(domain)) — identical gating (widget_display_enabled flag + ui_blocked_clients suppression)
- build_resources(): register the ui:// resource with mime 'text/html;profile=mcp-app'
- dispatch_read_resource(): serve the HTML with WIDGET_HTML_TTL_MS (24h) + ui_contents_meta, like the other arms

Visual spec (approved design language): compact panel, target <= ~150px tall at 320px width. Header line = logged_date. Two-column layout: LEFT = logged portions as a compact list (food name + kcal, muted P/C/F subline per row); RIGHT = updated daily totals as 5 status-colored rings (under=blue, met=green, over=red) with percent-of-goal centered inside each ring and a tiny nutrient label below. Card/ring styling reference: docs/widget-design-reference-goals-cards.png (rounded cards, bold labels, thick round-cap rings) — note the reference PNG's ring colors are per-nutrient accents, but this widget uses STATUS colors per the parent ticket spec. The 5-ring + portion-list budget at 320px is tight: expect 34-40px rings in a 3x2 or 2x3 grid; verify visually and tune.

<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 resources/list includes ui://nom-mcp/food-added; resources/read serves the widget HTML (mime text/html;profile=mcp-app, 24h TTL, contents meta carries ui.resourceUri).
- [x] #2 log_meal's tool declaration carries _meta.ui pointing at the resource when widget display is enabled; suppressed when disabled or the requesting client is blocklisted — identical gating to the existing three widget tools (shared test fixture extended to register LogMeal; enabled/disabled/blocked-client cases covered).
- [x] #3 Widget renders the log_meal response (portions + daily_totals) from the tool-result bridge; layout <= ~150px tall at 320px width; light/dark theming works via the existing host-context mechanism.
- [x] #4 Handler tests cover: resource serving, gating enabled/disabled/blocked-client for log_meal, build_resources listing.
- [x] #5 Manual verification on a seeded local instance (TASK-54 flow): logging a multi-portion meal shows the widget alongside the text result. Screenshot batches <= 4 images per prompt (LiteLLM cap, see AGENTS.md); split larger comparisons across prompts/subagents.
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- New asset `nom-core/assets/food_added_widget.html`: fully self-contained (restrictive CSP, no external fetches), inline JSON-RPC-over-postMessage scaffold with ui/initialize handshake, data via ui/notifications/tool-result, host-context-changed theming (colorScheme + CSS variable overrides) on top of light-dark() custom properties with the shared palette (--under blue / --met green / --over red), ResizeObserver -> ui/notifications/size-changed, escapeHtml on all interpolated strings. appInfo name `nom-mcp-food-added-widget`. Layout: header line = logged_date; left column = logged portions (food name + kcal, muted P/C/F subline); right column = five status-colored donut rings (pathLength='100', stroke-dasharray = percent-of-goal centered inside each ring, tiny nutrient label below).
- MCP wiring in `nom-core/src/operation/mcp_handler.rs`, mirroring the three existing widgets: `FOOD_ADDED_UI_RESOURCE_URI = "ui://nom-mcp/food-added"` + `FOOD_ADDED_WIDGET_HTML` via include_str!; `food_added_ui_meta(domain)` through the shared ui_meta() helper; 'log_meal' match arm in build_tools_gated() (identical widget_display_enabled + ui_blocked_clients gating); registered in build_resources() with mime 'text/html;profile=mcp-app'; served in dispatch_read_resource() with WIDGET_HTML_TTL_MS (24h) + ui_contents_meta.
- Functional e2e on the TASK-54 seeded instance verified resource listing/serving, _meta.ui gating (enabled/disabled/blocked-client), and payload shape. Visual verification delegated to vision subagents in <=4-image batches per the vLLM image cap: rendered at 320x139 (inside the <=~150px budget), all status colors and light/dark theming correct — SHIP verdict.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Shipped TASK-58.2: new food_added_widget.html asset plus the MCP-surface wiring that gates and serves it at ui://nom-mcp/food-added. log_meal now carries _meta.ui under the same widget_display_enabled + ui_blocked_clients gating as the existing widget tools. All 5 ACs verified: handler tests green in CI, functional e2e on a seeded instance, visual review SHIP (320x139, all statuses/theming correct).
<!-- SECTION:FINAL_SUMMARY:END -->
