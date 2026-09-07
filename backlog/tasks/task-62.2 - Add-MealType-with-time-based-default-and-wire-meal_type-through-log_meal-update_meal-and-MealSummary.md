---
id: TASK-62.2
title: >-
  Add MealType with time-based default and wire meal_type through log_meal,
  update_meal and MealSummary
status: Dev Ready
assignee: []
created_date: '2026-09-07 01:28'
updated_date: '2026-09-07 01:30'
labels:
  - task
  - planned
dependencies:
  - TASK-62.1
documentation:
  - >-
    backlog/docs/research/doc-6 -
    Research-meal-type-annotation-time-based-default-and-schema-evolution.md
  - CONTEXT.md
parent_task_id: TASK-62
priority: high
type: task
ordinal: 71000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Core vertical slice of TASK-62: introduce the Meal Type attribute end to end on the write path and in meal read-back (AC #1-#4, #6). Depends on TASK-62.1 for the column.

Scope: a new `MealType` type + time-based derivation from the shared Clock, an optional `meal_type` argument on log_meal and update_meal with Validation-class rejection of bad values, and the value reappearing in every meal read (`MealSummary`, hence search_meals / get_meals_by_date_range / log_meal response) on all four surfaces.

Locked decisions (do not re-litigate during execution):
- Field/column name `meal_type`; values lowercase breakfast | lunch | dinner; no Snack/Unknown value.
- Default windows over the LOCAL wall-clock hour, half-open, gapless, wrapping midnight: breakfast 05:00–10:59, lunch 11:00–15:59, dinner 16:00–04:59.
- Pre-existing rows keep NULL and serialize as JSON null — no backfill, no query-time inference.
- Request field is `Option<String>` validated by hand, NOT a typed enum (a typed enum fails inside serde_json::from_value and reports field "request" instead of "meal_type", breaking AC #4 on every surface). Copy the calories_direction precedent at goal/mod.rs:514-522.

Out of scope: summary grouping (TASK-62.3), docs/glossary (TASK-62.4), widgets (TASK-62.5), filtering queries by meal type.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A logged Meal stores exactly one of breakfast/lunch/dinner; when no argument is supplied it is derived from the logged instant read in the shared Clock's timezone
- [ ] #2 Boundary hours 05:00, 11:00, 16:00 (and 00:00) map to the documented windows, exhaustively tested over all 24 local hours
- [ ] #3 An explicit meal_type argument overrides the derivation, and an invalid value yields ErrorData::validation with field == "meal_type" on CLI, HTTP, MCP and remote-CLI
- [ ] #4 Pre-existing rows with NULL meal_type read back as JSON null with no data loss and no query-time inference
- [ ] #5 MealSummary carries meal_type, so search_meals, get_meals_by_date_range and the log_meal response expose it on every surface
- [ ] #6 update_meal accepts an override, re-derives when only logged_at changes, and leaves the column untouched when neither is supplied
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Decisions (locked)

- Name: `meal_type`. Values: lowercase `breakfast` | `lunch` | `dinner`. No snack/unknown value exists.
- Default windows over the **local** wall-clock hour, half-open and gapless, wrapping midnight: breakfast `05:00–10:59`, lunch `11:00–15:59`, dinner `16:00–04:59`. A post-midnight log extends dinner; `logged_date` still follows the local calendar day and must NOT change here.
- Legacy rows (`meal_type IS NULL`) stay NULL and serialize as `"meal_type": null`. No backfill and no read-time inference: `logged_at` is stored UTC so SQL bucketing would be wrong for non-UTC users, and stamping a value the user never chose fabricates data.
- Request side is `Option<String>` + hand validation, matching `calories_direction` (goal/mod.rs:514-522). A typed enum in the request struct fails inside `serde_json::from_value`, which the ops map to `ErrorData::validation("request", ...)` — wrong field name on CLI, HTTP, MCP and remote-CLI alike.

## Step 1 — new module `nom-core/src/meal_type.rs` (~120 lines incl. tests; `fasting.rs` is the sibling precedent for a small meal-derived module)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum MealType { Breakfast, Lunch, Dinner }
```

Shape copied from `Direction` (goal/mod.rs:27-34). Methods:

- `pub fn from_local_hour(hour: u32) -> Self` — exhaustive over `0..=23` using named constants (`const BREAKFAST_HOURS: RangeInclusive<u32> = 5..=10;`, `LUNCH_HOURS = 11..=15`, dinner = the remainder). No inline magic numbers or chained comparisons.
- `pub fn parse(raw: &str) -> Option<Self>` — the `parse_direction` pattern (goal/mod.rs:126-133).
- `pub fn as_str(&self) -> &'static str`.

Unit tests: all 24 hours map to exactly one variant; boundary asserts `04 → dinner`, `05 → breakfast`, `10 → breakfast`, `11 → lunch`, `15 → lunch`, `16 → dinner`, `23 → dinner`, `00 → dinner`.

Register nothing — this is a plain module, add `pub mod meal_type;` where `fasting` is declared.

## Step 2 — Clock gains local-hour access (`nom-core/src/clock.rs`)

Beside `logged_date` (L73-75) add:

```rust
/// Local wall-clock hour of `utc_datetime` in the resolved timezone.
/// Derived at write time alongside logged_date; never recomputed retroactively.
pub fn local_hour(&self, utc_datetime: &DateTime<Utc>) -> u32
```

Implementation mirrors `logged_date`: `utc_datetime.with_timezone(&self.tz).hour()`. The UTC→local projection is infallible (only local→UTC can be ambiguous), so no `Result`. `Clock` is `Copy` and already reachable from `LogMeal`/`UpdateMeal` (struct fields at meal/mod.rs:619, :794).

## Step 3 — `log_meal` (`nom-core/src/meal/mod.rs`)

1. `LogMealRequest` (L606-616): add

```rust
/// Which meal this is: 'breakfast', 'lunch' or 'dinner'. Defaults from the
/// logged time in the server's timezone: breakfast 05:00-10:59, lunch
/// 11:00-15:59, dinner 16:00-04:59.
#[serde(skip_serializing_if = "Option::is_none")]
pub meal_type: Option<String>,
```

That doc text becomes the CLI help (cli_router.rs:48-52 reads `description`) and the MCP `inputSchema` verbatim (mcp_handler.rs:673), so one comment covers all surfaces.

2. In `execute_json` (L654-771), after the existing `logged_at`/`logged_date` resolution (L676-693), resolve the type once from the SAME `DateTime<Utc>` used for `logged_date` (`Utc::now()` on the no-argument path):

```rust
let meal_type = match req.meal_type.as_deref() {
    Some(raw) => MealType::parse(raw).ok_or_else(|| {
        ErrorData::validation("meal_type",
            format!("must be one of 'breakfast', 'lunch', 'dinner', got '{raw}'"))
    })?,
    None => MealType::from_local_hour(self.clock.local_hour(&dt)),
};
```

3. `insert_meal` (L250-314): take one extra param `meal_type: MealType`, append `meal_type` to the INSERT column list and one `?` bound via `.as_str()`. Appending (not inserting mid-list) keeps the argument list free of same-type adjacency hazards of the kind TASK-12/TASK-19 removed.

4. `MealSummary` (L81-93): add `pub meal_type: Option<String>` WITHOUT `skip_serializing_if`, so legacy rows emit `"meal_type": null` instead of dropping the key. Fill it in `build_meal_summary` (L418-599): append `, meal_type` to the meals SELECT (L420-426) and read index 13 with the NULL-tolerant `turso::Value::Null` pattern shown at L231-241.

## Step 4 — `update_meal` (L777-1100)

Add the same `Option<String>` field + validation to `UpdateMealRequest` (L777-791). Resolution order:

1. explicit `meal_type` argument → store it;
2. else if `logged_at` is being changed → re-derive from the new instant (mirrors how `logged_date` is recomputed in the same UPDATE at L877-883; leaving a stale label after moving a meal from 20:00 to 08:00 would be plainly wrong);
3. else leave the column untouched.

Side benefit: editing a legacy row's time gives it a real meal type. Record this trade-off in CONTEXT.md (TASK-62.4).

## Step 5 — read surfaces need zero plumbing

`search_meals` (L1280-1357) and `get_meals_by_date_range` (L1410-1461) both render through `build_meal_summary`, so AC #6 follows from step 3. `cli_router.rs`, `http_router.rs` (:20-37, whole JSON body forwarded), `mcp_handler.rs` (`call_tool`, :624-637) and `nom-mcp-remote` are all generic over `input_schema` — verify by inspection, change nothing.

## Step 6 — tests (`#[cfg(test)] mod tests` in meal/mod.rs, `TempDb` + `Clock { tz: chrono_tz::UTC }` pattern; see the template at L1637-1661)

- Default derivation: `log_meal` with `logged_at` at `…T05:00:00Z` / `…T11:00:00Z` / `…T16:00:00Z` → breakfast/lunch/dinner under a UTC clock.
- Timezone sensitivity: same UTC instants under `Clock { tz: "America/New_York".parse().unwrap() }` → shifted types, proving derivation uses the Clock TZ like `logged_date` does (AC #2).
- Explicit override beats the clock: `logged_at` 20:00 + `meal_type: "lunch"` → `lunch`.
- Invalid value: `meal_type: "snack"` → `Err` with category `Validation` AND `field == "meal_type"` (assert the field, not just the category — that is what AC #4 turns on).
- Legacy row: raw `INSERT INTO meals (...)` without `meal_type`, then `get_meals_by_date_range` → entry's `meal_type` is `null`; then `update_meal` on that row's `logged_at` fills it in (AC #5).
- `update_meal`: explicit override, re-derivation when only `logged_at` changes, column untouched when neither is supplied.
- Seed fixtures (`seed/mod.rs:363-366`): stamp realistic `meal_type` values per fixture so demos and widgets look sane; keep `nom-mcp/tests/seed_e2e.rs` repeatability green.

## Verification

```sh
nix develop .#ci -c cargo fmt --all
nix develop .#ci -c cargo clippy --all-targets --all-features --workspace -- -D warnings
nix develop .#ci -c cargo nextest run --all-features --workspace
nix develop .#ci -c cargo test --doc --all-features --workspace
# manual smoke across two surfaces
nix develop .#ci -c cargo run -p nom-mcp --bin nom-mcp -- log_meal --portions '[{"food_id":1,"quantity":100.0,"quantity_mode":"grams"}]' --meal_type lunch
nix develop .#ci -c cargo run -p nom-mcp --bin nom-mcp -- get_meals_by_date_range --start_date 2026-01-01 --end_date 2030-01-01
```

## Done when

AC #1-#4 hold on all four surfaces, AC #6 holds for meal queries, legacy rows read back as `null`, and the full CI set is green.
<!-- SECTION:PLAN:END -->
