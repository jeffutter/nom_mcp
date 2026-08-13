---
id: TASK-26
title: 'Fix: TASK-2.15 shipped zero unit tests for the 6 weight-entry operations'
status: To Do
assignee: []
created_date: '2026-08-13 02:06'
labels:
  - review-followup
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
- [ ] #1 nom-core/src/weight/mod.rs has a #[cfg(test)] mod tests block using the TempDb/with_db_path fixture pattern already used by nom-core/src/meal/mod.rs's test module
- [ ] #2 LogWeight has tests covering: default logged_at (uses clock.today()), explicit valid logged_at (backdating), and value <= 0.0 rejected as Validation error
- [ ] #3 UpdateWeightEntry has tests covering: value-only patch, logged_at-only patch, both together, and not-found error for a nonexistent entry_id
- [ ] #4 DeleteWeightEntry has tests covering: successful hard delete (row no longer queryable afterward) and not-found error for a nonexistent entry_id
- [ ] #5 GetWeightToday, GetWeightByDate, and GetWeightByDateRange each have tests covering: empty result set, a populated result set, and DESC ordering by logged_at
- [ ] #6 The task's Implementation Notes/Final Summary claims about test counts are corrected to reflect the true nix develop -c cargo test -p nom-core pass count
- [ ] #7 nix develop -c cargo test -p nom-core passes
- [ ] #8 nix develop -c cargo clippy --workspace --all-targets is clean
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
SETUP (read first): This is a Rust+WebAssembly core (crates/gql-core) with a TypeScript/React web app (web/). Wait — correction for this repo: nom_mcp is a Rust workspace (nom-core, nom-mcp, nom-mcp-http) with no WASM/web component. ALL commands must run inside the Nix dev shell: either run 'direnv allow' once, or prefix every command with 'nix develop -c'. Work from the repository root unless told otherwise. Do not change pinned dependency versions.

1. Open nom-core/src/meal/mod.rs and find its '#[cfg(test)] mod tests' block (near the end of the file) — study its TempDb/temp-file fixture setup, the pattern of constructing an operation via '::new(clock).with_db_path(db_path.clone())' (or '::new().with_db_path(...)' for read-only ops), and how it asserts on the returned serde_json::Value. Weight operations already implement the identical '#[cfg(test)] db_path: Option<PathBuf>' + 'with_db_path()' fixture hook (see e.g. nom-core/src/weight/mod.rs:88-107 for LogWeight) — you are wiring tests onto scaffolding that already exists, not adding it.
2. Add a '#[cfg(test)] mod tests' block at the end of nom-core/src/weight/mod.rs (after line 767). Set up a shared helper that creates a fresh temp SQLite/turso DB with the schema migrated (mirror however meal's tests do this — likely a shared test-support helper in nom-core/src/storage or nom-core/src/meal/mod.rs's own test setup; reuse it rather than re-inventing).
3. For LogWeight (nom-core/src/weight/mod.rs:88-213): write test_log_weight_default_timestamp (no logged_at, assert logged_date matches a fixed Clock's today()), test_log_weight_explicit_logged_at (assert both logged_at and logged_date reflect the parsed timestamp), and test_log_weight_rejects_non_positive_value (value = 0.0 and a negative value both return Err with category Validation and field 'value').
4. For UpdateWeightEntry (nom-core/src/weight/mod.rs:232-361): write test_update_weight_entry_value_only, test_update_weight_entry_logged_at_only, test_update_weight_entry_both_fields, and test_update_weight_entry_not_found (nonexistent entry_id returns Err with the NotFound category).
5. For DeleteWeightEntry (nom-core/src/weight/mod.rs:374-481): write test_delete_weight_entry_success (delete then verify a follow-up GetWeightByDate/direct query no longer returns the row) and test_delete_weight_entry_not_found.
6. For GetWeightToday, GetWeightByDate, GetWeightByDateRange (nom-core/src/weight/mod.rs:490-767): for each, write a test with an empty DB (expect an empty array), a test with 2-3 seeded entries (expect all of them back), and a test asserting DESC ordering by logged_at (insert entries out of order, assert the response array is newest-first).
7. Update the Implementation Notes section of backlog/tasks/task-2.15*.md to correct the false test-count claim once real tests exist and the true count is known.
8. Run: nix develop -c cargo test -p nom-core (confirm all weight:: tests plus the existing 165 pass)
9. Run: nix develop -c cargo clippy --workspace --all-targets (confirm clean)
10. Run: nix develop -c cargo fmt -p nom-core (confirm no diff)
<!-- SECTION:PLAN:END -->
