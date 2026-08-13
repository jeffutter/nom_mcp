---
id: TASK-27
title: >-
  Fix: weight module list operations silently drop rows on storage error via
  redundant N+1 queries
status: To Do
assignee: []
created_date: '2026-08-13 02:07'
labels:
  - review-followup
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
SETUP (read first): This is a Rust workspace (nom-core, nom-mcp, nom-mcp-http; no WASM/web component in this repo). ALL commands must run inside the Nix dev shell: either run 'direnv allow' once, or prefix every command with 'nix develop -c'. Work from the repository root unless told otherwise. Do not change pinned dependency versions.

1. In nom-core/src/weight/mod.rs, change GetWeightToday::execute_json's SQL at :542 from 'SELECT id FROM weight_entries WHERE logged_date = ? ORDER BY logged_at DESC' to 'SELECT id, logged_at, logged_date, value FROM weight_entries WHERE logged_date = ? ORDER BY logged_at DESC'.
2. Replace the loop body at :552-564 (currently: read id, call build_weight_summary, silently drop on Err) with direct row-to-WeightEntrySummary mapping — read all four typed columns off 'row' the same way build_weight_summary itself does at :57-70, propagating each column-read error with '?' instead of swallowing it.
3. Repeat steps 1-2 identically for GetWeightByDate (:638-660) and GetWeightByDateRange (:736-762).
4. Decide whether build_weight_summary (:38-73) still has live callers after this change — LogWeight does not currently call it, but UpdateWeightEntry does (:342, to return the post-update summary). Keep build_weight_summary as-is for that single remaining caller; do not remove it.
5. Add or extend tests in the mod tests block (see the companion ticket TASK-26 for the base test scaffold — if that ticket has not landed yet, add a minimal temp-db test here covering: seed 2-3 weight_entries rows, call GetWeightByDate, assert the returned array has the same length and field values as what was seeded).
6. Run: nix develop -c cargo test -p nom-core
7. Run: nix develop -c cargo clippy --workspace --all-targets
8. Run: nix develop -c cargo fmt -p nom-core
<!-- SECTION:PLAN:END -->
