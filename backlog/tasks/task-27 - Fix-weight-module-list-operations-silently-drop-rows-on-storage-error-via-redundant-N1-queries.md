---
id: TASK-27
title: >-
  Fix: weight module list operations silently drop rows on storage error via
  redundant N+1 queries
status: In Progress
assignee:
  - '@ralph'
created_date: '2026-08-13 02:07'
updated_date: '2026-08-13 02:16'
labels:
  - review-followup
  - planned
dependencies:
  - TASK-2.15
priority: high
ordinal: 240
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while reviewing TASK-2.15 (nom-core/src/weight/mod.rs). GetWeightToday (:552-564), GetWeightByDate (:648-660), and GetWeightByDateRange (:750-762) each run an outer 'SELECT id FROM weight_entries WHERE ...' query, then call 'build_weight_summary(&conn, id).await' per row to re-fetch all four columns in a second query. Two problems: (1) Resilience — the result is discarded silently on failure: 'if let Ok(summary) = build_weight_summary(&conn, id).await { summaries.push(summary); }' drops the row with zero logging if the second query fails for any reason (e.g. a transient storage error on a row that was JUST selected in the first query), producing an incomplete result list with no indication anything went wrong. The sibling nom-core/src/meal/mod.rs does the equivalent two-query pattern at meal/mod.rs:1330-1337 and :1435-1442 but logs via 'tracing::warn!(meal_id = id, "meal not found during summary build")' on failure — weight's version has no such logging at all. (2) Concision — meal's second query is justified because meal summaries require a join across portions/foods; weight_entries is (per this file's own doc comment at :7-9) 'no FK relationships, no snapshotting... just raw value storage', so the outer query could simply select all four columns directly (id, logged_at, logged_date, value) instead of re-fetching per row via build_weight_summary — the second query buys nothing here.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 GetWeightToday's query selects id, logged_at, logged_date, and value directly in the single 'SELECT ... WHERE logged_date = ? ORDER BY logged_at DESC' and maps each row straight to a WeightEntrySummary — no per-row call to build_weight_summary
- [ ] #2 GetWeightByDate and GetWeightByDateRange are changed identically
- [ ] #3 build_weight_summary is retained only for its remaining callers (LogWeight and UpdateWeightEntry, which each need a single-row fetch after a write) — remove it if those are also inlined, but do not leave it as unused dead code either way
- [ ] #4 No error from row iteration is silently discarded — a row-read failure inside the loop propagates as ErrorData::storage_failure via the '?' operator, consistent with how the rest of each function already handles row errors
- [ ] #5 A regression test proves list results are complete even after a hypothetical row-level fetch would have previously failed (or, at minimum, a test proves the list operations return all seeded rows with correct field values, guarding against a future silent-drop regression)
- [ ] #6 nix develop -c cargo test -p nom-core passes
- [ ] #7 nix develop -c cargo clippy --workspace --all-targets is clean
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Implementation Plan

### Overview
Eliminate the N+1 query pattern in three list operations (GetWeightToday, GetWeightByDate, GetWeightByDateRange) by selecting all four columns directly instead of fetching IDs then calling build_weight_summary per row. This simultaneously fixes two bugs: (1) silent row drops when the second query fails, and (2) unnecessary round-trips that scale linearly with result size.

build_weight_summary is retained for its remaining caller (UpdateWeightEntry post-update fetch).

### File Changes

#### nom-core/src/weight/mod.rs

**Step 1: Fix GetWeightToday (lines ~540-564)**
- Change SQL from `SELECT id FROM weight_entries WHERE logged_date = ? ORDER BY logged_at DESC` to `SELECT id, logged_at, logged_date, value FROM weight_entries WHERE logged_date = ? ORDER BY logged_at DESC`
- Replace the loop body that reads `id` then calls `build_weight_summary(&conn, id).await` with direct column mapping from the row:
  
- The `if let Ok(summary)` wrapper is eliminated — errors propagate via `?` operator

**Step 2: Fix GetWeightByDate (lines ~638-660)**
- Identical change: expand SQL to include all columns, replace ID-only loop with direct row mapping
- Same error propagation pattern

**Step 3: Fix GetWeightByDateRange (lines ~736-762)**
- Identical change: expand SQL to include all columns, replace ID-only loop with direct row mapping
- Same error propagation pattern

**Step 4: Verify build_weight_summary retention**
- build_weight_summary is still called by UpdateWeightEntry at line ~342 (post-update summary fetch)
- Keep the function as-is; do not remove or modify

**Step 5: Add regression test**
- In a `#[cfg(test)] mod tests` block (note: TASK-26 covers comprehensive tests for all 6 ops; this ticket adds ONE focused regression test):
- Test: seed 3 weight entries for the same date, call GetWeightByDate, assert returned array has exactly 3 items with matching field values
- This guards against future regressions where silent drops could creep back in
- Use TempDb fixture pattern from meal module tests

### Verification
1. `nix develop -c cargo test -p nom-core` — all tests pass
2. `nix develop -c cargo clippy --workspace --all-targets` — clean
3. `nix develop -c cargo fmt -p nom-core`

### Why No Sub-Tickets
All changes are in a single file, tightly coupled (same pattern applied to 3 functions), and total diff is under 60 lines. Splitting into sub-tickets would create coordination overhead without meaningful independent shippability.
<!-- SECTION:PLAN:END -->
