---
id: TASK-1.2
title: Research Turso/libSQL rust SDK for local-file-only usage
status: Done
assignee:
  - '@Jeffery Utter'
created_date: '2026-08-11 04:39'
updated_date: '2026-08-11 04:56'
labels:
  - 'wayfinder:research'
dependencies: []
documentation:
  - doc-4
parent_task_id: TASK-1
ordinal: 3000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Question

How does the `libsql` Rust SDK (https://docs.turso.tech/sdk/rust/quickstart) work in local-file-only mode (no Turso cloud account, no embedded-replica sync)? Cover: connection/builder API for a plain local file, schema migration story (does it ship migration tooling, or is that BYO?), transaction API, how it compares to plain `rusqlite` in this mode, and any limitations relevant to a single-user server (concurrent access from CLI + server process touching the same file, etc.).
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
## Correction (post-resolution)

Original recommendation (libsql, for maturity) reversed after the user clarified their priority was specifically avoiding a C-toolchain/linked-library dependency, and confirmed local-CLI direct-DB access is a rare debugging path, not a concurrent-with-server workflow. Verified via GitHub repo metadata that libsql's repo is C and turso's is Rust, and cross-checked turso's docs/manual.md Limitations section and bindings/rust/README.md directly. Open follow-up: confirm turso's sequential (non-overlapping) multi-process file handoff safety in TASK-1.5/TASK-1.6 before this is load-bearing.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Corrected pick after user clarification: use the turso crate (github.com/tursodatabase/turso), not libsql. libsql (github.com/tursodatabase/libsql) is a mature but C-based SQLite fork — Rust bindings via libsql-sys wrap vendored C source, requiring a C toolchain at build time. turso is a from-scratch pure-Rust reimplementation (v0.7.x, pre-1.0) with a near-identical API shape (Builder::new_local(path).build().await, conn.execute/query, transactions with commit/rollback). Given the user's explicit priority on a pure-Rust dependency with no C linking, turso wins despite being pre-1.0. Trade-off accepted: turso's docs list 'No multi-process access' as a current limitation (true cross-process concurrency needs an experimental --experimental-multiprocess-wal flag, not production-ready), plus no triggers/views/vacuum/savepoints yet. User's local-CLI direct-DB-access mode is a low-frequency debugging affordance rather than primary workflow, which lowers the stakes of the multi-process gap — but whether strictly sequential (never-overlapping) process handoff is safe on turso today is NOT confirmed and needs verification in TASK-1.5/TASK-1.6. No first-party migration tooling in either crate — raw SQL migrations either way. WAL is turso's default journal mode (unlike libsql, which needs it set explicitly).
<!-- SECTION:FINAL_SUMMARY:END -->
