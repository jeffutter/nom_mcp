---
id: TASK-26
title: 'Fix: TASK-2.15 shipped zero unit tests for the 6 weight-entry operations'
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-13 02:06'
updated_date: '2026-08-13 04:35'
labels:
  - review-followup
  - planned
dependencies:
  - TASK-2.15
priority: high
ordinal: 230
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while reviewing TASK-2.15 (nom-core/src/weight/mod.rs, entire file). The task's own approved Implementation Plan has an explicit 'Testing Strategy' section requiring: log_weight (default timestamp, explicit logged_at, invalid value), update_weight_entry (partial updates, not-found error), delete_weight_entry (successful delete, not-found error), and get_weight_today/by_date/by_date_range (empty results, populated results, ordering). None of this exists — the file has zero '#[cfg(test)] mod tests' block and zero #[test] functions. Confirmed empirically: 'nix develop -c cargo test -p nom-core weight::' reports '0 passed; 0 failed; 0 measured; 165 filtered out', and the total nom-core test count (165) is identical before and after this commit. The task's own Implementation Notes and Final Summary both claim 'Full test suite passes (170 tests)' / 'proper error handling, transaction wrapping, and test support' — both statements are false; no new tests were added and the true count is 165, not 170. Correctness-axis finding: all three ACs were checked off, and completion was claimed, with zero test evidence backing any of the six operations. This is the same failure pattern already found and fixed once this round in TASK-22 (TASK-2.12 shipping without its promised tests).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 nom-core/src/weight/mod.rs has a #[cfg(test)] mod tests block using the TempDb/with_db_path fixture pattern already used by nom-core/src/meal/mod.rs's test module
- [x] #2 LogWeight has tests covering: default logged_at (uses clock.today()), explicit valid logged_at (backdating), and value <= 0.0 rejected as Validation error
- [x] #3 UpdateWeightEntry has tests covering: value-only patch, logged_at-only patch, both together, and not-found error for a nonexistent entry_id
- [x] #4 DeleteWeightEntry has tests covering: successful hard delete (row no longer queryable afterward) and not-found error for a nonexistent entry_id
- [x] #5 GetWeightToday, GetWeightByDate, and GetWeightByDateRange each have tests covering: empty result set, a populated result set, and DESC ordering by logged_at
- [x] #6 The task's Implementation Notes/Final Summary claims about test counts are corrected to reflect the true nix develop -c cargo test -p nom-core pass count
- [x] #7 nix develop -c cargo test -p nom-core passes
- [x] #8 nix develop -c cargo clippy --workspace --all-targets is clean
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Add #[cfg(test)] mod tests to nom-core/src/weight/mod.rs covering all 6 weight operations. Use the existing TempDb fixture (nom-core/src/storage/test.rs) and with_db_path() hooks already on every operation struct. Mirror meal/mod.rs test patterns exactly. All tests use #[serial_test::serial] and #[tokio::test].

**AC #1 — Test module setup:**
- Extend existing '#[cfg(test)] mod tests' block after line ~833 (after the one regression test). Reuse its imports (super::*, crate::storage::test::TempDb, serial_test). Add Arc, Connection, Clock imports.
- Shared Clock: `Clock { tz: chrono_tz::UTC }` — same as meal tests. Hardcoded UTC makes today() deterministic for backdated writes.

**AC #2 — LogWeight tests (3 tests):**
- `test_log_weight_default_timestamp`: Call LogWeight with only `{ "value": 75.0 }`. Assert result has logged_at ISO string and logged_date matching Clock::new(chrono_tz::UTC).today(). Since clock uses Utc::now(), verify logged_date equals today's date string rather than hardcoding.
- `test_log_weight_explicit_logged_at`: Call with `{ "value": 75.0, "logged_at": "2025-01-15T10:30:00Z" }`. Assert logged_at == "2025-01-15T10:30:00Z", logged_date == "2025-01-15".
- `test_log_weight_rejects_non_positive_value`: Call with `{ "value": 0.0 }` and `{ "value": -5.0 }`. Both must return Err with `.unwrap_err().category == ErrorCategory::Validation`.

**AC #3 — UpdateWeightEntry tests (4 tests):**
- Seed entry first via raw INSERT, capture id. Then:\n  - `test_update_weight_entry_value_only$: Patch `{ "id": id, "value": 80.0 }`. Verify new value persisted.\n  - `test_update_weight_entry_logged_at_only$: Patch `{ "id": id, "logged_at": "2025-06-01T09:00:00Z" }$. Verify new timestamp/date.\n  - `test_update_weight_entry_both_fields$: Patch both simultaneously.\n  - `test_update_weight_entry_not_found$: Call with nonexistent id (e.g., 99999). Assert ErrorCategory::NotFound.
- After each update test, re-query with GetWeightByDate to confirm persistence.

**AC #4 — DeleteWeightEntry tests (2 tests):**
- `test_delete_weight_entry_success$: Seed entry, delete with `{ "id": id }$, then assert GetWeightByDate returns empty array for that date.\n- `test_delete_weight_entry_not_found$: Delete with nonexistent id. Assert ErrorCategory::NotFound.

**AC #5 — Query operation tests (9 tests total, 3 per operation):**
For each of GetWeightToday, GetWeightByDate, GetWeightByDateRange:\n- Empty DB test: Call with no seeded data. Assert result.is_array() && result.as_array().unwrap().is_empty().\n- Populated test: Seed 2 entries for relevant date(s). Assert array length == 2, all fields present (id, logged_at, logged_date, value).\n- Ordering test: Seed 3 entries with distinct timestamps (same date, different hours: 08:00, 12:00, 18:00). Insert in reverse chronological order. Assert response array is newest-first (DESC by logged_at).\n\nUse distinct hours to avoid timestamp collision within the same second.\n\n**AC #6 — Correct TASK-2.15 documentation:**
After tests pass, run `nix develop -c cargo test -p nom-core` to get the true test count. Edit backlog/tasks/task-2.15*.md to correct the false claim about 170 tests.\n\n**AC #7 & #8 — Quality gates:**
Run `nix develop -c cargo test -p nom-core`, `cargo clippy --workspace --all-targets`, and `cargo fmt -p nom-core`.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Added 18 unit tests (17 new + retained existing regression test) to nom-core/src/weight/mod.rs covering all 6 weight operations via #[cfg(test)] mod tests with TempDb fixture pattern. Fixed clippy bool_assert_comparison warning. Corrected TASK-2.15 Implementation Notes and Final Summary to reflect true test count (was falsely claiming 170 tests; actual count was 165 then 183 after this fix). Total nom-core test count: 183 unit tests + 1 integration test = 184.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added comprehensive unit test suite (18 tests) for all 6 weight operations in nom-core/src/weight/mod.rs: LogWeight (default timestamp, explicit logged_at, rejects non-positive values), UpdateWeightEntry (value-only, logged_at-only, both fields, not-found), DeleteWeightEntry (success with verification, not-found), GetWeightToday/GetWeightByDate/GetWeightByDateRange (empty results, populated results, DESC ordering each). Used TempDb fixture and async seed_entry helper with RETURNING id. Fixed clippy warning. Corrected false test count claims in TASK-2.15 documentation.
<!-- SECTION:FINAL_SUMMARY:END -->
