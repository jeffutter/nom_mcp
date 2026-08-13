---
id: TASK-31
title: >-
  Fix: build_weight_summary duplicates weight_entry_summary_from_row's
  row-mapping logic
status: To Do
assignee: []
created_date: '2026-08-13 05:26'
labels:
  - review-followup
dependencies:
  - TASK-27
priority: high
ordinal: 100
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while reviewing TASK-27 (nom-core/src/weight/mod.rs:38-91). TASK-27's fixup commit extracted weight_entry_summary_from_row() (lines 38-53) to satisfy the Rule of Three across GetWeightToday/GetWeightByDate/GetWeightByDateRange, but build_weight_summary() (lines 56-91), which runs the identical 'SELECT id, logged_at, logged_date, value ... WHERE id = ?' query and maps the row to WeightEntrySummary field-by-field, was left as a byte-for-byte duplicate of that same mapping instead of calling the new helper. Concision/Organization-axis finding: the knowledge of how a weight_entries row maps to WeightEntrySummary now has two representations in the same file — if a column is added or a field renamed, only one of the two call sites is likely to be updated, silently producing an inconsistent DTO between the list operations and the single-row fetch used by UpdateWeightEntry.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 build_weight_summary's Some(row) branch calls weight_entry_summary_from_row(&row) instead of re-listing each row.get::<T>(n) call
- [ ] #2 No behavior change: UpdateWeightEntry's post-update fetch still returns the same WeightEntrySummary shape
- [ ] #3 nix develop -c cargo test -p nom-core passes
- [ ] #4 nix develop -c cargo clippy --workspace --all-targets is clean
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
SETUP (read first): This is a Rust workspace (nom-core, nom-mcp, nom-mcp-http; no WASM/web component in this repo). ALL commands must run inside the Nix dev shell: either run 'direnv allow' once, or prefix every command with 'nix develop -c'. Work from the repository root unless told otherwise. Do not change pinned dependency versions.

1. Open nom-core/src/weight/mod.rs. Read weight_entry_summary_from_row() (lines 38-53) — it takes a &turso::Row and returns Result<WeightEntrySummary, ErrorData>, mapping columns 0-3 (id, logged_at, logged_date, value) in that exact order.
2. Read build_weight_summary() (lines 56-91). Its SQL ('SELECT id, logged_at, logged_date, value FROM weight_entries WHERE id = ?') selects columns in the identical order weight_entry_summary_from_row() expects.
3. In build_weight_summary()'s 'Some(row) => Ok(WeightEntrySummary { ... })' arm (lines 75-88), replace the inline struct construction with a single call: 'Some(row) => weight_entry_summary_from_row(&row),' — this works directly since weight_entry_summary_from_row already returns Result<WeightEntrySummary, ErrorData>, matching this match arm's expected type.
4. Confirm the 'None => Err(ErrorData::not_found())' arm is unchanged.
5. Run: nix develop -c cargo test -p nom-core (confirm all 185 tests still pass — this is a pure refactor, no test additions needed since existing tests for UpdateWeightEntry already exercise build_weight_summary's row-mapping path).
6. Run: nix develop -c cargo clippy --workspace --all-targets (confirm clean).
7. Run: nix develop -c cargo fmt --check (confirm no diff).
<!-- SECTION:PLAN:END -->
