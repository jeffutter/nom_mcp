---
id: TASK-10
title: >-
  Fix: lock_probe's locked-file test always silently no-ops instead of verifying
  lock detection
status: To Do
assignee: []
created_date: '2026-08-12 05:28'
labels:
  - review-followup
dependencies:
  - TASK-2.11
priority: high
ordinal: 140
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while reviewing TASK-2.11 (nom-core/src/storage/lock_probe.rs:69-136, test_probe_locked_file). AC #2 for TASK-2.11 requires 'opening the DB directly first probes the advisory lock and fails fast... if held'. The only test exercising the held-lock branch spawns a helper process by writing Rust source to a temp file and compiling it with a bare 'rustc <path> -o <path>' invocation that references 'libc::...' without ever linking the libc crate (no --extern flag, no Cargo-managed rlib path). This compilation fails every time (confirmed: 'error[E0433]: cannot find module or crate libc' x7), the test's own fallback catches the failed compile and does 'eprintln!(...); return;' instead of failing, and cargo reports the test as passing ('test storage::lock_probe::tests::test_probe_locked_file ... ok') without ever having called probe_db_lock against an actually-locked file. So the core positive-detection behavior probe_db_lock is supposed to provide — and that AC #2 requires — has zero real test coverage, while the test suite reports green. This is a Correctness/Resilience axis violation: a test that silently passes without asserting the behavior it claims to verify is worse than no test, because it hides the gap from future readers and from CI.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 test_probe_locked_file (or its replacement) actually exercises the branch where another process holds the write lock, without relying on 'rustc' being available on PATH or compiling ad-hoc source at test time
- [ ] #2 the test fails loudly (not silently skips) if it cannot set up the locked-file scenario for any reason
- [ ] #3 nix develop -c cargo test -p nom-core -- --nocapture storage::lock_probe shows the locked-file assertion actually executing (not the 'skipping locked-file test' eprintln path)
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

1. Read nom-core/src/storage/lock_probe.rs in full, especially test_probe_locked_file (lines 69-136).
2. Replace the rustc-compile-a-helper-from-a-string approach with a reliable same-binary re-exec pattern: re-invoke the current test binary itself as the child process via std::env::current_exe(), gated behind an environment variable (e.g. NOM_LOCK_PROBE_HELPER=1) checked at the very top of the test binary's entry point, OR add a small #[test] -annotated function that is invoked by name via 'cargo test --exact <name> -- --ignored' from the parent test as a subprocess (std::process::Command::new(std::env::current_exe().unwrap()).arg("lock_probe::tests::hold_lock_helper").arg("--exact").arg("--ignored").arg("--nocapture")). Either approach avoids depending on rustc/libc linking at test time since it reuses the already-compiled, already-linked test binary which has libc available.
3. In the child path, open the same file, take an F_SETLKW write lock via libc::fcntl exactly as the current helper source attempted, then sleep long enough for the parent to probe (e.g. 2 seconds) — no need for a full 30s sleep.
4. In the parent test, spawn the child via step 2's mechanism, wait briefly for it to acquire the lock, assert probe_db_lock returns Ok(true), then kill the child, wait for exit, and assert probe_db_lock returns Ok(false).
5. Make sure any setup failure (spawn failure, unexpected exit code) causes the test to panic/fail rather than silently return.
6. Run: nix develop -c cargo test -p nom-core -- --nocapture storage::lock_probe and confirm the locked-file assertions actually execute (no 'skipping' message). Also run the full suite: nix develop -c cargo test -p nom-core.
<!-- SECTION:PLAN:END -->
