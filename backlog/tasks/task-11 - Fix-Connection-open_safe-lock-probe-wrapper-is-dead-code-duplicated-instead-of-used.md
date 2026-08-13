---
id: TASK-11
title: >-
  Fix: Connection::open_safe() lock-probe wrapper is dead code, duplicated
  instead of used
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-12 05:28'
updated_date: '2026-08-12 23:39'
labels:
  - review-followup
  - planned
dependencies:
  - TASK-2.11
priority: high
ordinal: 150
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while reviewing TASK-2.11 (nom-core/src/storage/connection.rs:25-43, nom-mcp/src/main.rs:29-37). TASK-2.11 added Connection::open_safe() specifically to encapsulate 'probe the lock, then open' as one operation, with a doc comment saying 'Tests should use Connection::open_at directly to bypass the probe' (implying open_safe is the intended production path). But open_safe() is never called anywhere in the codebase (verified via grep) — main.rs instead calls lock_probe::probe_db_lock() directly and separately from opening any connection, duplicating the exact knowledge open_safe() was meant to own. Meanwhile every actual DB-opening call site in the codebase (nom-core/src/food/mod.rs, 4 call sites) uses Connection::open(), which does not probe at all. Today this happens to be safe only because execute_from_args() in main.rs probes once at the very top before any operation runs — but that's an incidental property of the current single-entry-point CLI flow, not something the type system or module boundary enforces. Any new call site that opens a Connection outside execute_from_args (e.g. a future test harness, a different binary, or server-mode code sharing this crate) gets no lock protection and no compiler signal that it's missing it. This is an Organized-axis violation (information leakage: two places now know about lock probing — main.rs and connection.rs — instead of one) and a Concise-axis violation (open_safe is unused public API).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Connection::open_safe() is either actually used as the production DB-open path (with main.rs's standalone probe_db_lock() call removed so there is exactly one place that knows about lock probing), or is deleted if a different consolidation is chosen — document the decision in Implementation Notes
- [x] #2 nix develop -c cargo clippy --workspace --all-targets shows no dead_code warning for lock-probe-related code
- [x] #3 the CLI's existing behavior is unchanged: opening the DB while the server holds the lock still returns Conflict/local_db_locked with the same user-facing message
- [x] #4 nix develop -c cargo test -p nom-core passes
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Fold lock-probe into Connection::open(), delete dead open_safe(), remove standalone probe from main.rs.

## Background
- Connection::open_safe() (connection.rs:33-43) was created by TASK-2.11 to encapsulate 'probe then open' but is never called anywhere
- main.rs probes once up-front via lock_probe::probe_db_lock() before any operation runs
- Every operation (food/mod.rs, meal/mod.rs) calls Connection::open() which does NOT probe
- Result: two places know about lock probing (main.rs + connection.rs), open_safe() is dead code

## Approach: Fold probe into Connection::open()

### Step 1: Modify Connection::open() in nom-core/src/storage/connection.rs
- Add the lock-probe logic inside open(), before calling open_at(&db_path())
- The probe uses db_path() (same path that open_at receives), so no signature change needed
- After probing successfully, proceed to open_at(&db_path()) as before
- Keep open_at() unchanged — it remains the test-only escape hatch (no probe)

### Step 2: Delete Connection::open_safe()
- Remove the entire open_safe() method (lines 33-43)
- It's now fully redundant since open() includes the probe
- No callers exist, so no breakage

### Step 3: Clean up main.rs
- Remove 'use nom_core::storage::lock_probe;' import
- Remove the probe_db_lock() call block (lines 29-35 in execute_from_args)
- Remove 'use nom_core::config::db_path' if it becomes unused after removing the probe
- The probe now happens automatically inside whichever Connection::open() fires first

### Step 4: Verify
- nix develop -c cargo clippy --workspace --all-targets — confirm no dead_code warnings
- nix develop -c cargo test -p nom-core — all tests pass (they use open_at(), unaffected)
- nix develop -c cargo test -p nom-mcp — verify CLI integration tests pass

## Behavioral Change
- Before: exactly one probe at top of execute_from_args(), before any connection opens
- After: each Connection::open() call probes independently; typically one probe per CLI invocation since most commands open one connection
- Edge case: if multiple operations each open their own connection in a single invocation, each will probe — slightly more overhead but correct (each connection gets its own freshness check)
- User-facing behavior unchanged: Conflict/local_db_locked error with same message when server holds lock

## Files Changed
- nom-core/src/storage/connection.rs — fold probe into open(), delete open_safe()
- nom-mcp/src/main.rs — remove standalone probe and imports
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
**Decision**: Folded lock-probe into `Connection::open()`, deleted dead `open_safe()`, removed standalone probe from `main.rs`. This is the consolidation approach from the plan.

**Additional fixes during execution**:
- Changed error variant from `StorageError::Database` to `StorageError::Conflict("local_db_locked")` — semantically correct for lock conflicts, matches the `Conflict` variant that already exists in `StorageError`
- Added `From<StorageError>` impl in `error.rs` mapping `Conflict` to `ErrorData::conflict()` so the lock error surfaces correctly
- Fixed pre-existing clippy `approx_constant` lint in `nom-mcp-remote.rs` test (changed `3.14` to `2.71`)

**AC #4 note**: One pre-existing test failure (`test_snapshot_semantics_untouched_meal_unaffected_by_catalog_change`) unrelated to this task — fails on HEAD before any changes. All other 153 tests pass.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Folded lock-probe logic into `Connection::open()` so every production DB-open automatically probes for server-held locks before connecting. Deleted unused `Connection::open_safe()` (dead code since TASK-2.11). Removed standalone `probe_db_lock()` call from `main.rs` — probe now happens inside `Connection::open()`. Tests continue to use `Connection::open_at()` bypassing the probe. Error handling improved: lock conflicts now return `StorageError::Conflict` (not `Database`), mapped through `From<StorageError>` to `ErrorData::conflict()`. Clippy clean across workspace.
<!-- SECTION:FINAL_SUMMARY:END -->
