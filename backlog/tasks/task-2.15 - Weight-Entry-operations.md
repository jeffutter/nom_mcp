---
id: TASK-2.15
title: Weight Entry operations
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-11 13:24'
updated_date: '2026-08-13 01:40'
labels:
  - planned
dependencies:
  - TASK-2.5
  - TASK-2.7
  - TASK-2.10
type: feature
ordinal: 34000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Scope
log_weight(value, logged_at?), update_weight_entry(id, value?, logged_at?), delete_weight_entry(id) — errors on not-found. get_weight_today/by_date/by_date_range — same per-scope query split and optional-logged_at backdating as Meal. Edits are plain field updates with no cascade concerns. All deletes are hard deletes, no soft-delete/undo.

See doc-5 §5, §13.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 log_weight/update_weight_entry/delete_weight_entry implemented; delete errors on not-found
- [x] #2 get_weight_today/by_date/by_date_range implemented using the Clock's today() for the 'today' variant
- [x] #3 delete is a hard delete with no soft-delete flag or undo path
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Implementation Plan

### Overview
Create a new `nom-core/src/weight/mod.rs` module with 6 weight entry operations, mirroring the meal module structure exactly. Weight entries are simpler than meals: no FK relationships, no snapshotting, no computed totals — just raw value storage with temporal handling.

### File Changes

#### 1. nom-core/src/lib.rs
- Add `pub mod weight;` alongside existing module declarations

#### 2. nom-core/src/weight/mod.rs (new file)
Six operations following meal module patterns:

**Shared types:**
- `WeightEntrySummary` — response struct with `id`, `logged_at`, `logged_date`, `value`
- Helper function `build_weight_summary(conn, id)` — queries single row, returns summary

**Write operations (need Clock):**
- `LogWeight` — request has `value: f64`, optional `logged_at`. If logged_at absent: use `Utc::now()` + `clock.today()` for `logged_date`. If present: parse ISO 8601, compute both fields. Insert into `weight_entries`. Return `{entry_id, logged_at, logged_date, value}`. Validate `value > 0`.
- `UpdateWeightEntry` — request has `entry_id`, optional `value`, optional `logged_at`. Pre-check existence (error if not found). Transactional UPDATE of nullable fields. Return updated `WeightEntrySummary`.
- `DeleteWeightEntry` — request has `entry_id`. Pre-check existence (error if not found). Hard DELETE in transaction. Return `{deleted: true, entry_id}`.

**Read operations (no clock needed):**
- `GetWeightToday` — uses `Clock::today()` to get today's date, queries `weight_entries WHERE logged_date = ? ORDER BY logged_at DESC`. Returns array of summaries.
- `GetWeightByDate` — request has `date` (YYYY-MM-DD). Same query pattern with provided date.
- `GetWeightByDateRange` — request has `start_date`, `end_date`. Query `WHERE logged_date >= ? AND logged_date <= ? ORDER BY logged_at DESC`. Covers both above as special cases.

**Pattern details per operation:**
- Each op has its own request struct with `#[derive(Debug, Deserialize, JsonSchema)]`
- Each op has `pub struct OpName { clock: Clock, #[cfg(test)] db_path }`
- Write ops implement `fn new(clock: Clock)`, read ops use `impl Default`
- All ops implement `#[cfg(test)] fn with_db_path()` for testability
- All ops implement `Operation` trait with `execute_json(Arc<Value>)`
- Error handling follows meal patterns: `ErrorData::validation()` for input, `ErrorData::not_found()` for missing entries, `ErrorData::storage_failure()` for DB errors

#### 3. nom-mcp/src/main.rs
- Add imports for all 6 weight operations
- Register all 6 operations in registry after meal registrations

### Testing Strategy
Integration tests alongside meal tests using `TempDb` fixture. Test each operation independently:
- log_weight: default timestamp, explicit logged_at, invalid value
- update_weight_entry: partial updates, not-found error
- delete_weight_entry: successful delete, not-found error
- get_weight_today/by_date/by_date_range: empty results, populated results, ordering

### Execution Order
Single atomic implementation — create weight module, wire into lib.rs, register in main.rs, add tests. No intermediate shippable increments that make sense alone.
<!-- SECTION:PLAN:END -->
