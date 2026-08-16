---
id: TASK-47
title: Track intermittent fasting window derived from meal data
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-15 23:20'
updated_date: '2026-08-16 00:04'
labels: []
dependencies: []
references:
  - CONTEXT.md
  - nom-core/src/goal/mod.rs
  - nom-core/src/weekly/mod.rs
  - nom-core/src/meal/mod.rs
  - nom-core/src/storage/schema.sql
priority: medium
type: feature
ordinal: 52000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Add automatic intermittent-fasting tracking: the fasting window for a given day is the time from that day's last logged Meal to the next logged Meal (earliest Meal on any later day — no manual fasting logging, no schema change; derived entirely from meals.logged_at / logged_date).

Confirmed product decisions (user, 2026-07):
- Daily report surface: add a `fasting_hours` field to the existing `get_goal_progress` operation (per-date report; rides CLI/HTTP/MCP automatically). No new operation.
- Skipped-day semantics: if the next calendar day has no meals, the window extends to the earliest meal on ANY later day (e.g. skipping a full day yields a ~48h fast). If the queried day has no meals, or no meal exists after it, the window is undefined (field omitted / null).
- Weekly report surface: the shared `fetch_weekly_summary()` gains a fasting section with the weekly AVERAGE fasting hours across the rolling 7-day window (averaged over days in the window that have a completed window) plus a count of such days. This covers both the `nom://weekly-summary` MCP resource and the `get_weekly_progress` tool at once.
- Unit: fractional hours (f64).

Domain-language note: CONTEXT.md needs a new glossary term for this concept before naming types/fields (see AGENTS.md).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Fasting window is derived automatically from existing meal data (last Meal of day D by logged_date -> earliest Meal with logged_date > D); no new table, column, or manual input
- [x] #2 get_goal_progress response includes fasting_hours (fractional hours) for the queried date; field is omitted when the date has no meals or no meal exists after that date
- [x] #3 Weekly summary (both nom://weekly-summary resource and get_weekly_progress tool) reports the average fasting hours over the rolling 7-day window plus the number of window days with a completed window; average is null/absent when no window completed
- [x] #4 Multi-day skip behavior: a day followed by a meal-free day reports the duration up to the first meal on the next day that has one
- [x] #5 Tests cover: normal adjacent-day window, no meals on queried day, no meals after queried day, multi-day skip, and the weekly-average math (including zero-completed-window case)
- [x] #6 CONTEXT.md gains a glossary entry defining the fasting concept in ubiquitous language; README documents the new fields in the get_goal_progress and weekly-summary sections
- [x] #7 CI green: cargo fmt --check, clippy -D warnings, nextest --all-features --workspace, cargo test --doc, rustdoc build
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Implementation plan (recorded 2026-08-15, before implementation)

**Approach**: derive fasting windows from existing meal data — `meals.logged_at` (UTC instants, `%Y-%m-%dT%H:%M:%SZ`) grouped by `meals.logged_date` (local date). No schema change, no new Operation, no registry change; both report surfaces ride their existing operations.

### Steps

1. **New module `nom-core/src/fasting.rs`** (+ `pub mod fasting;` in lib.rs)
   - `pub struct FastingWindow { pub date: String, pub hours: f64 }` — one completed window per day.
   - `pub async fn fetch_fasting_windows(conn, start_date, end_date) -> Result<Vec<FastingWindow>, ErrorData>`:
     - Q1: `SELECT logged_date, MIN(logged_at), MAX(logged_at) FROM meals WHERE logged_date BETWEEN ? AND ? GROUP BY logged_date ORDER BY logged_date`
     - Q2: `SELECT MIN(logged_at) FROM meals WHERE logged_date > ?` (end_date) — terminal fallback so the last day of the range can still complete its window when the next meal falls after the range.
     - For each day D having a max timestamp: next = min_ts of the first later day with meals (forward scan of Q1), else Q2's value if present; `hours = (next − last).num_seconds() as f64 / 3600.0`. Days without a last meal are skipped.
   - TempDb unit tests: adjacent-day window (e.g. 23:00 → 07:00 = 8.0h), no meals on D, no meals after D, multi-day skip (~48h), empty DB.

2. **Daily surface — `goal/mod.rs` GetGoalProgress**
   - Add `#[serde(skip_serializing_if = "Option::is_none")] fasting_hours: Option<f64>` to `GoalProgress`.
   - `execute_json`: call `fetch_fasting_windows(&conn, &query_date, &query_date)` (reuses step 1; 2 queries total), take `.first().map(|w| w.hours)`.
   - Tests: field present with correct value when seeded; omitted when queried day has no meals; omitted when no meal exists after the day.

3. **Weekly surface — `weekly/mod.rs` fetch_weekly_summary**
   - New `pub struct FastingSummary { #[serde(skip_serializing_if = "Option::is_none")] average_hours: Option<f64>, days_with_fasting: u32 }`; add `fasting: FastingSummary` to `WeeklySummary`.
   - Compute from `fetch_fasting_windows(conn, &start_date, &today)`: `average_hours = sum/count` when count > 0 else None. Covers `nom://weekly-summary` resource + `get_weekly_progress` tool together.
   - Tests: average math over seeded windows; zero-completed-window case (None + 0); `test_get_weekly_progress_matches_fetch_weekly_summary` stays green (both sides serialize the same struct).

4. **Docs**
   - CONTEXT.md: new glossary entry **Fasting Window** (derived concept; _Avoid_: "fast timer", "IF streak").
   - README: mention fasting in the `get_goal_progress` row and the weekly-summary section.

### Risks / notes
- Timezone: grouping by local `logged_date` while subtracting UTC instants is monotonic across midnight boundaries (a meal's local time lies within its own local day), so durations are always ≥ 0; DST shifts don't affect subtraction of two Utc instants.
- Widget safety: goal-progress widget HTML is a static asset (`include_str!("../../assets/goal_progress_widget.html")`) — adding a JSON field cannot break it.
- Existing tests assert field-level values, not full-JSON snapshots — low blast radius.
- Parse `logged_at` with `chrono::DateTime::<Utc>::parse_from_rfc3339` (same shape LogMeal writes).
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Both design forks confirmed with user before planning: daily report rides get_goal_progress (no new operation); skipped-day semantics extend the window to the earliest meal on ANY later day.

Two initial test failures were bad expectations in my own new tests (Jan 11's window correctly incomplete with no later meal; 07:00Z->09:00Z across midnight is 26h not 2h) — code was correct, fixed the tests.

Smoke-tested through the real surfaces: local CLI (get_goal_progress --date) and an MCP stdio session (initialize -> tools/call get_weekly_progress).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added automatic intermittent-fasting tracking derived entirely from existing Meal data — no schema change, no new Operation, no registry change.

**What changed**
- New `nom-core/src/fasting.rs`: `fetch_fasting_windows(conn, start_date, end_date)` computes each day's Fasting Window (last Meal of the day by `logged_date` → earliest Meal on any later day, fractional hours) from two queries: per-day MIN/MAX(`logged_at`) over the range plus a terminal `MIN(logged_at) WHERE logged_date > end_date` so the last day of the range still completes when its next meal falls outside it. Days without Meals, or with no later Meal, yield no window (fast still open).
- Daily surface: `get_goal_progress` response gains `fasting_hours` (omitted when undefined) via a single-date call into the same helper. Rides CLI/HTTP/MCP automatically.
- Weekly surface: `WeeklySummary` gains a `fasting` section (`average_hours` over days with completed windows + `days_with_fasting`), computed once in `fetch_weekly_summary()`, so both the `nom://weekly-summary` resource and the `get_weekly_progress` tool report it.
- Docs: CONTEXT.md **Fasting Window** glossary entry; README updated for both surfaces.

**Verification**
- 13 new tests (7 in fasting.rs incl. adjacent-day, terminal lookup, multi-day skip ~36h, fractional hours, empty DB; 4 in goal; 2 in weekly incl. average math (8+26)/2=17 and zero-completed-window case). Full suite: 281/281 nextest, clippy `-D warnings` clean, fmt clean, doctests + rustdoc clean.
- End-to-end smoke: local CLI against a temp DB reported `"fasting_hours": 8.0` for a seeded 23:00Z→07:00Z gap; a real MCP stdio session's `get_weekly_progress` returned `{"average_hours": 17.0, "days_with_fasting": 2}`.

**Notes**: widget HTML is a static asset, so the new JSON field cannot affect rendering; existing tests used field-level assertions, so blast radius was contained. Work is uncommitted (left for review).
<!-- SECTION:FINAL_SUMMARY:END -->
