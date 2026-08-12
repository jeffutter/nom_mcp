---
id: TASK-11
title: >-
  Fix: Connection::open_safe() lock-probe wrapper is dead code, duplicated
  instead of used
status: To Do
assignee: []
created_date: '2026-08-12 05:28'
labels:
  - review-followup
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
- [ ] #1 Connection::open_safe() is either actually used as the production DB-open path (with main.rs's standalone probe_db_lock() call removed so there is exactly one place that knows about lock probing), or is deleted if a different consolidation is chosen — document the decision in Implementation Notes
- [ ] #2 nix develop -c cargo clippy --workspace --all-targets shows no dead_code warning for lock-probe-related code
- [ ] #3 the CLI's existing behavior is unchanged: opening the DB while the server holds the lock still returns Conflict/local_db_locked with the same user-facing message
- [ ] #4 nix develop -c cargo test -p nom-core passes
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
SETUP (read first): This is a Rust+WebAssembly core (crates/gql-core) with a
TypeScript/React web app (web/). ALL commands must run inside the Nix dev
shell: either run 'direnv allow' once, or prefix every command with
'nix develop -c'. Work from the repository root unless told otherwise. Do not
change pinned dependency versions.

Note: this repo's actual crate layout is nom-core/ and nom-mcp/ (not crates/gql-core — ignore that path in the preamble; everything else in the preamble still applies).

1. Read nom-core/src/storage/connection.rs (open, open_safe, open_at) and nom-mcp/src/main.rs execute_from_args() in full.
2. Decide the consolidation: the simplest fix consistent with 'pull complexity downward' is to delete the standalone lock_probe::probe_db_lock() call in main.rs (lines ~29-37) and instead have execute_from_args() open its connection via Connection::open_safe() at the point registry/operations actually need a connection — but note operations currently call Connection::open() themselves inside food/mod.rs rather than main.rs passing a connection in, so open_safe() alone at the top of main.rs would not change what food/mod.rs does. The cleanest fix given the current architecture (main.rs probes once, up front, before any operation runs) is: keep the single up-front probe in main.rs exactly as it is today, but change Connection::open() itself to call probe_db_lock() internally (folding open_safe's behavior into open(), since there is currently no call site that wants unprobed production access), then delete Connection::open_safe() as redundant, and delete the now-redundant explicit probe_db_lock() call in main.rs (the probe now happens inside whichever Connection::open() call happens first). Keep Connection::open_at() as the test-only unprobed escape hatch, unchanged.
3. Update food/mod.rs's #[cfg(test)] conn-opening blocks if needed (they already use open_at() directly in test mode, so they should be unaffected).
4. Remove the now-unused 'use nom_core::storage::lock_probe;' and 'use nom_core::config::db_path' imports from main.rs if they become unused.
5. Run: nix develop -c cargo clippy --workspace --all-targets (confirm no dead_code warnings), nix develop -c cargo test -p nom-core, nix develop -c cargo test -p nom-mcp.
<!-- SECTION:PLAN:END -->
