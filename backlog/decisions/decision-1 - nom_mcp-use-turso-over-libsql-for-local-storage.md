---
id: decision-1
title: 'nom_mcp: use turso over libsql for local storage'
date: '2026-08-11 04:56'
status: accepted
---
## Context

nom_mcp needs local-file SQLite-family storage with no Turso cloud account. Two Turso-ecosystem Rust crates fit that shape: `libsql` (github.com/tursodatabase/libsql, a mature fork of SQLite's C source, Rust bindings via `libsql-sys`) and `turso` (github.com/tursodatabase/turso, a from-scratch pure-Rust reimplementation, currently pre-1.0 at v0.7.x). The user's explicit priority was a dependency that doesn't require linking a C library/toolchain.

## Decision

Use `turso`, not `libsql`, despite `turso` being pre-1.0. `libsql`'s repo is C; `turso`'s is pure Rust, matching what the user actually wanted. Accepted trade-off: turso's own docs list "No multi-process access" as a current limitation — true cross-process concurrency requires an experimental, not-production-ready `--experimental-multiprocess-wal` flag — plus missing triggers/views/vacuum/savepoints. The user's local-CLI-hits-DB-directly mode is a low-frequency debugging affordance rather than the primary workflow, which lowers the stakes of the multi-process gap, but whether strictly sequential (non-overlapping) process handoff is safe today is unconfirmed and is a follow-up in TASK-1.5/TASK-1.6.

## Consequences

- If turso's multi-process story proves unworkable even for sequential handoff, the fallback is either dropping local-CLI direct-DB access (all CLI use goes through the server/HTTP) or switching to `libsql`/`rusqlite` and re-accepting the C-toolchain requirement.
- turso's pre-1.0 API may still shift; schema/migration code (BYO raw SQL either way, no first-party migration tooling in either crate) should avoid relying on features turso doesn't support yet (triggers, views, savepoints).

