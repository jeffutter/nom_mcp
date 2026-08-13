---
id: TASK-2.17
title: MCP Resource (weekly-summary) and Widget Display tools
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-11 13:24'
updated_date: '2026-08-13 11:34'
labels:
  - planned
dependencies:
  - TASK-2.7
  - TASK-2.14
  - TASK-2.15
  - TASK-2.16
type: feature
ordinal: 36000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Scope
Weekly Summary Resource — MCP-only, fixed URI nom://weekly-summary, no params, live-computed on every read (no caching). Stays outside the Operation trait entirely — hand-written list_resources/read_resource on ServerHandler, data-fetching in a capability-layer function. Rolling last-7-days window (not calendar week). Nutrients section shaped like get_goal_progress (per-nutrient daily-average consumed/target/remaining/percent/direction/status) plus a per-day array of raw daily totals. Weight section: start/end/delta from Weight Entries in the window (null start/delta if none logged, but latest_known_weight still comes from the most recent entry before the window), plus target-weight comparison.

Widget Display: get_widget_display()/set_widget_display(enabled: bool), ordinary Operations with surfaces()=MCP only, backed by the settings table (widget_display_enabled BOOLEAN). v1 is plumbing only — no tool or Resource output branches on it yet.

See doc-5 §8, decision-3.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 nom://weekly-summary resource returns nutrients (daily-average vs target, per-day breakdown) and weight (start/end/delta, latest_known_weight, target comparison) for a rolling 7-day window
- [x] #2 get_widget_display/set_widget_display are Operations with surfaces()=MCP only, persisted in the settings table
- [x] #3 no other Operation or Resource output currently branches on widget_display_enabled
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Implementation Plan: MCP Resource (weekly-summary) and Widget Display tools

### Overview
Two distinct deliverables in one ticket:
1. **Weekly Summary Resource** — MCP-only resource at nom://weekly-summary, live-computed rolling 7-day window with nutrients (daily-average vs targets + per-day breakdown) and weight (start/end/delta, latest_known_weight, target comparison)
2. **Widget Display Operations** — get_widget_display/set_widget_display as ordinary Operations with surfaces()=MCP only, backed by the existing settings table

Both modify McpHandler (resource methods + capability), so they ship atomically. No sub-tickets needed — total scope is ~450 lines across 3 files.

---

### File 1: Extend McpHandler for Resources (nom-core/src/operation/mcp_handler.rs)

#### A. Enable resources capability in get_info()

Override get_info() to return ServerInfo with resources capability enabled. Use ServerCapabilities::builder().enable_tools().enable_resources().build(). Include server_info with name "nom-mcp" and version from CARGO_PKG_VERSION.

#### B. Implement list_resources()

Return exactly one resource — nom://weekly-summary with title "Weekly Summary", description "Rolling 7-day nutrition and weight summary", mime_type "application/json".

#### C. Implement read_resource()

Parse URI, match on "nom://weekly-summary", dispatch to fetch_weekly_summary(), serialize result as TextResourceContents. Return ErrorData::validation for unknown URIs.

#### D. Add Clock and db_path fields to McpHandler

Add clock: Clock and #[cfg(test)] db_path: Option<PathBuf> to struct. Update constructor to accept clock parameter. This lets the resource handler compute the 7-day window relative to today.

---

### File 2: Weekly Summary Data Fetching (nom-core/src/weekly/mod.rs)

New module following goal/meal/weight patterns. Single public async function fetch_weekly_summary(conn, clock) plus output types and tests.

#### Output Types

Reuse NutrientProgress and ProgressStatus from goal module (make them pub). Define:

- WeeklySummary: start_date, end_date, days_with_data, nutrients(NutrientsSummary), weight(WeightSummary)
- NutrientsSummary: calories/protein_g/carbs_g/fat_g/fiber_g (all NutrientProgress), daily_totals(Vec<DailyTotals>)
- DailyTotals: date, calories, protein_g, carbs_g, fat_g, fiber_g
- WeightSummary: latest_known_weight?, start_weight?, end_weight?, delta?, target_weight?, remaining?, status?

#### Data Fetching Function

Three SQL queries in a single connection:

1. Daily totals grouped by date (rolling 7-day window): SELECT logged_date, SUM(total_calories)... FROM meals WHERE logged_date BETWEEN ? AND ? GROUP BY logged_date ORDER BY logged_date

2. Weight entries in window: SELECT value FROM weight_entries WHERE logged_date BETWEEN ? AND ? ORDER BY logged_date

3. Latest known weight before window: SELECT value FROM weight_entries WHERE logged_date < ? ORDER BY logged_date DESC LIMIT 1

Compute daily averages: sum each nutrient across all days / 7 (not just days_with_data). Compare against active goal using nutrient_progress() helper from goal module.

Weight section logic:
- start_weight = first weight entry in window (null if none)
- end_weight = last weight entry in window (null if none)
- delta = end_weight - start_weight (null if either is null)
- latest_known_weight = most recent entry before OR within window (always resolves if any entry exists at all)
- target_weight, remaining, status = same logic as weight_progress() from goal module

#### Tests

Follow goal module test patterns exactly (TempDb, serial_test, tokio::test):

- test_fetch_weekly_summary_empty_db — returns zeroed nutrients, null weight
- test_fetch_weekly_summary_with_meals — verifies daily averages and per-day breakdown
- test_fetch_weekly_summary_with_goal — verifies target comparison fields populate
- test_fetch_weekly_summary_without_goal — consumed values populate, target-derived fields null
- test_fetch_weekly_summary_weight_trend — start/end/delta computed correctly
- test_fetch_weekly_summary_weight_no_entries_in_window — latest_known_weight resolves from pre-window entry
- test_fetch_weekly_summary_daily_totals_ordering — ORDER BY logged_date verified

---

### File 3: Widget Display Operations (nom-core/src/widget/mod.rs)

New module with two Operations, both Surfaces::MCP.

#### GetWidgetDisplay Operation

No params, reads settings table. Returns {enabled: bool}. Handles row not found (table empty) as default false.

#### SetWidgetDisplay Operation

Single param enabled: bool, writes settings table. Uses UPDATE then INSERT-if-no-changes pattern since settings has no primary key. Returns {enabled: bool}.

#### Settings Table Access Pattern

The settings table has no primary key — it is a single-row table. Use:
- Read: SELECT widget_display_enabled FROM settings LIMIT 1 — defaults to false if table empty
- Write: UPDATE settings SET widget_display_enabled = ?; if changes == 0, INSERT INTO settings (widget_display_enabled) VALUES (?)

#### Tests

- test_get_widget_display_default_false — fresh DB returns enabled=false
- test_set_widget_display_true — set true, verify persisted
- test_set_widget_display_then_get — round-trip verification
- test_set_widget_display_false — toggle back to false

---

### Wiring Changes

nom-core/src/lib.rs: add pub mod weekly and pub mod widget

nom-mcp/src/main.rs: import GetWidgetDisplay/SetWidgetDisplay from nom_core::widget, register both ops in registry, update McpHandler::new() call to pass Clock

Make NutrientProgress and ProgressStatus pub in goal/mod.rs so weekly/mod.rs can reuse them rather than duplicating

---

### Execution Order

1. Make NutrientProgress and ProgressStatus public in goal/mod.rs (one-line visibility change)
2. Create nom-core/src/weekly/mod.rs with fetch_weekly_summary(), output types, and tests
3. Create nom-core/src/widget/mod.rs with both Operations and tests
4. Extend McpHandler in mcp_handler.rs with resource support (get_info, list_resources, read_resource, add clock/db_path fields)
5. Wire everything in lib.rs and main.rs
6. Run full quality checks: fmt, clippy, doc, test suite

### Risk Assessment
- Low risk — schema exists, patterns proven, rmcp API stable at 2.2.0
- One gotcha: settings table has no PK; must handle single-row upsert carefully (UPDATE then INSERT-if-no-changes pattern)
- Testing: All integration tests use TempDb fixture; no external services needed
<!-- SECTION:PLAN:END -->
