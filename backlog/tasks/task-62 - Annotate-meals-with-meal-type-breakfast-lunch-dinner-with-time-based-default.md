---
id: TASK-62
title: Annotate meals with meal type (breakfast/lunch/dinner) with time-based default
status: Done
assignee:
  - '@ralph'
created_date: '2026-09-07 00:05'
updated_date: '2026-09-07 02:53'
labels:
  - planned
dependencies:
  - TASK-62.1
  - TASK-62.2
  - TASK-62.3
  - TASK-62.4
documentation:
  - CONTEXT.md
  - >-
    backlog/docs/research/doc-6 -
    Research-meal-type-annotation-time-based-default-and-schema-evolution.md
priority: medium
type: feature
ordinal: 69000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Logged meals currently have no indication of which meal they were, so weekly summaries and any future per-meal reporting can't distinguish breakfast from dinner. Users want each Meal tagged with its meal type, defaulting sensibly from when it was logged so the common case needs no extra argument, while still allowing an explicit override (e.g. logging lunch in the evening, or a late breakfast). The tag is only useful if it comes back out: querying past foods/meals and viewing daily or weekly summaries must show which meal each entry belonged to, not just store it silently. Per the repo glossary (CONTEXT.md), meal type should be a first-class domain attribute, not inferred after the fact from timestamps at query time.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A logged Meal stores a meal type with exactly one of breakfast, lunch, or dinner
- [x] #2 When no meal type is supplied, it is derived from the logged time using the shared Clock's timezone (consistent with how 'today' is resolved elsewhere)
- [x] #3 An explicit meal type argument on the meal-logging operation overrides the time-based default on every surface that exposes the operation (CLI, HTTP, MCP, remote-CLI)
- [x] #4 An invalid meal type value is rejected with the standard Validation ErrorData (same shape on all surfaces)
- [x] #5 Existing meals without a stored meal type are handled gracefully (migration or read-time fallback) with no data loss
- [x] #6 Queries of past meals/foods (e.g. list or get meals for a date range) return each meal's meal type in their results on every surface
- [x] #7 Daily and weekly summary outputs group or label entries by meal type so a reader can tell breakfast from lunch from dinner
- [x] #8 Tests cover: default derivation at boundary times, explicit override, invalid value rejection, pre-existing rows without a meal type, and meal type appearing in past-meal query and summary outputs
- [x] #9 Documentation updated: README/guide coverage of the new argument and CONTEXT.md gains the meal type term in the domain glossary
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Approach

Store meal type as a first-class attribute on the Meal row (never inferred at query time, per the ticket description and CONTEXT.md), derived at write time from the logged instant read through the shared `Clock`, overridable by an explicit argument, and surfaced again in every meal read plus a new per-meal-type breakdown in the daily/weekly summaries.

Research is complete — see `backlog/docs/research/doc-6 - Research-meal-type-annotation-time-based-default-and-schema-evolution.md`. Every open question is decided below, which is why all four implementation children carry plans rather than waiting for their own planning passes.

## Decisions locked for all children

1. **Name**: `meal_type`, values lowercase `breakfast | lunch | dinner`. No snack/unknown value.
2. **Default windows** over the local wall-clock hour, half-open, gapless, wrapping midnight: breakfast `05:00–10:59`, lunch `11:00–15:59`, dinner `16:00–04:59`. A post-midnight log extends dinner; `logged_date` keeps following the local calendar day (changing that is out of scope even though the new label makes the mismatch visible).
3. **Schema evolution**: generalize the migration runner to a version list and append the column via migration v2 (`ALTER TABLE meals ADD COLUMN meal_type TEXT CHECK (...)`). `schema.sql` stays byte-identical so v1's recorded hash remains valid; using ALTER on both fresh and upgraded paths makes the two converge on identical column order.
4. **Legacy rows stay NULL** and serialize as `"meal_type": null`. No SQL backfill (the timezone lives in Rust's `Clock`, not the DB, so hour bucketing in SQL would mislabel non-UTC users) and no read-time inference (that is exactly what the ticket rules out).
5. **Validation shape**: request field is `Option<String>` checked against a `VALID_MEAL_TYPES` array, copying `calories_direction` (goal/mod.rs:514-522), so a bad value yields `ErrorData::validation("meal_type", ...)`. A typed enum in the request struct would fail inside `serde_json::from_value` and report field `"request"` on all four surfaces — wrong for AC #4. CLI gets no `possible_values` (cli_router builds plain string args); this matches how `variant` behaves today.
6. **No new dependency**: three variants follow the in-house `Direction` pattern; `strum` stays transitive.
7. **Surfaces need no plumbing**: `cli_router`, `http_router`, `mcp_handler` and `nom-mcp-remote` are generic over `input_schema`, so one new optional field reaches all four automatically. Verify by inspection, do not hand-write per-surface code.

## Children and execution order

| Order | Ticket | Ships |
|---|---|---|
| 1 | TASK-62.1 | Migration runner generalized to a version list; nullable `meal_type` column added by v2. Schema only, safe to deploy alone. |
| 2 | TASK-62.2 (deps 62.1) | `MealType` + `Clock::local_hour`, derivation and override in `log_meal`/`update_meal`, `MealSummary.meal_type` → covers AC #1-#4, #6. |
| 3 | TASK-62.3 (deps 62.2) | One shared `(logged_date, meal_type)` aggregate feeding weekly `daily_totals[].by_meal_type` and `get_goal_progress.meals_by_type` → AC #7. Day-level fields stay byte-compatible so widgets keep binding. |
| 4 | TASK-62.4 (deps 62.2, 62.3) | README + CONTEXT.md glossary entry → AC #9. |
| — | TASK-62.5 (child, intentionally NOT a dependency) | Optional widget polish; low priority, unplanned. Parent may close without it. |

AC #5 (legacy rows) is split deliberately: storage behavior in 62.1, read/write behavior in 62.2. AC #8 (tests) lives inside each child's own plan rather than a separate test ticket.

## Integration checks between children

- After 62.1: confirm a pre-existing DB file upgrades (copy a seeded DB, point `NOM_MCP_DB_PATH` at it, run any CLI op) before writing domain code against the column.
- After 62.2: run one command per surface against the same DB — local CLI `log_meal --meal_type lunch`, `POST /api/log_meal` on `serve http`, `nom-mcp-remote log_meal ...`, and an MCP `tools/call` — then `get_meals_by_date_range` and diff the JSON. They must agree field-for-field.
- After 62.3: read `nom://weekly-summary` and call `get_weekly_progress` on the same seeded DB and assert identical payloads (they serialize the same struct; if they differ, the fold is broken).
- Final gate for closing TASK-62: full CI plus a walk of all nine acceptance criteria against a real seeded database, including a legacy-null row inserted by raw SQL.

```sh
nix develop .#ci -c cargo fmt --all --check
nix develop .#ci -c cargo clippy --all-targets --all-features --workspace -- -D warnings
nix develop .#ci -c cargo nextest run --all-features --workspace
nix develop .#ci -c cargo test --doc --all-features --workspace
```

## Deliberately out of scope

- Filtering or grouping *queries* by meal type (`search_meals` still searches food names only) — not in the ACs.
- Changing `logged_date` semantics for post-midnight logs.
- Widgets showing meal type (TASK-62.5).
- Any snack/fourth-bucket expansion: if snacks ever become a real category, that is a schema + glossary decision of its own, since Android Health Connect and LogMeal both needed a fourth value.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Final gate walked against a real seeded DB (NOM_MCP_DB_PATH=/tmp/nom62/db.sqlite, seeded by seed_data, plus one legacy row inserted by raw SQL with meal_type NULL). Children: 62.1 = 0f2311c, 62.2 = 8d39c24, 62.3 = 5fa03f3, 62.4 = 93fcfd5. Evidence per criterion: #1/#2 log_meal with no argument at 02:38Z stored dinner (local 21:38 America/Chicago); update_meal --logged_at 19:00Z relabelled lunch (14:00 local) and 07:30Z relabelled dinner (02:30 local), proving derivation reads the Clock's timezone, not UTC. #3 override honoured on all four surfaces from one input_schema field with zero per-surface code: CLI --meal_type lunch -> meal_id 19 'lunch'; POST /api/log_meal {"meal_type":"dinner"} -> meal_id 22 'dinner'; nom-mcp-remote ... meal_type=breakfast logged; MCP tools/call log_meal meal_type=lunch returned meal_type 'lunch'. #4 invalid value rejected identically-shaped Validation errors: CLI exit 4 "validation error on field 'meal_type': must be one of 'breakfast', 'lunch', 'dinner', got 'snack'", REST HTTP 400 {"category":"Validation","field":"meal_type",...}, remote-CLI byte-identical to the CLI line; the MCP tool call returns isError with category+reason text, which is how every validation error already surfaces there (the shared Display for ErrorData omits the field on that surface for all operations, pre-existing and unchanged here). #5 the raw-SQL NULL row still lists normally as "meal_type": null, contributes its 123 kcal to both the day total and a trailing null bucket, and nothing was backfilled. #6 get_meals_by_date_range output is byte-identical between REST and local CLI (diffed) and carries meal_type on every meal over MCP too (breakfast/dinner/lunch/null). #7/#8 weekly daily_totals[].by_meal_type and get_goal_progress.meals_by_type verified live; nom://weekly-summary resource JSON and get_weekly_progress tool output compared equal (byte-identical payloads, both carrying the breakdown). Full CI: fmt --check clean, clippy -D warnings clean, 379 nextest tests pass, doctests pass. TASK-62.5 (widget polish) stays open by design — it was never a dependency of this ticket.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Meals now carry a first-class meal_type (breakfast | lunch | dinner), stored at write time and never inferred at query time. Default comes from the logged instant through the shared Clock's timezone (breakfast 05:00-10:59, lunch 11:00-15:59, dinner 16:00-04:59, wrapping midnight); an explicit argument overrides it on every surface for free, because all four read the same input_schema; editing logged_at re-derives unless overridden in the same call. Schema evolution went through a generalized version-list migration runner with v2 appending a nullable column, so fresh and upgraded databases converge on identical column order and pre-existing rows keep NULL rather than being backfilled or guessed at. It comes back out per Meal (meal_type) and per summary: one shared (logged_date, meal_type) aggregate feeds both the Weekly Summary's daily_totals[].by_meal_type (hence the nom://weekly-summary resource and get_weekly_progress) and get_goal_progress.meals_by_type, with legacy nulls bucketed last and day-level fields untouched so widget bindings keep working. Shipped as 62.1-62.4 across four commits; 62.5 (widget polish) intentionally remains open.
<!-- SECTION:FINAL_SUMMARY:END -->
