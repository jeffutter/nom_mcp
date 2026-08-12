---
id: TASK-10
title: >-
  Fix: lock_probe's locked-file test always silently no-ops instead of verifying
  lock detection
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-12 05:28'
updated_date: '2026-08-12 19:14'
labels:
  - review-followup
  - planned
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
- [x] #1 test_probe_locked_file (or its replacement) actually exercises the branch where another process holds the write lock, without relying on 'rustc' being available on PATH or compiling ad-hoc source at test time
- [x] #2 the test fails loudly (not silently skips) if it cannot set up the locked-file scenario for any reason
- [x] #3 nix develop -c cargo test -p nom-core -- --nocapture storage::lock_probe shows the locked-file assertion actually executing (not the 'skipping locked-file test' eprintln path)
- [x] #4 nix develop -c cargo test -p nom-core passes
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Fix test_probe_locked_file by replacing the rustc compile-from-string approach with same-binary re-exec pattern.

## Problem
test_probe_locked_file writes Rust source to a temp file and compiles with bare `rustc`, which never links libc (no `--extern`). Compilation fails silently, the test catches it and returns early, reporting 'ok' without ever exercising probe_db_lock against an actually-locked file.

## Solution: Re-exec current test binary

Replace the rustc helper with std::process::Command::new(std::env::current_exe()) spawning the compiled test binary itself as the child process. The test binary already has libc linked (it's a Cargo build), so fcntl calls work immediately.

### Implementation Steps

**Step 1: Add hold-lock helper function in lock_probe.rs**

Add a standalone function (not #[test]) that acts as the child-process entry point:

**Step 2: Rewrite test_probe_locked_file using cargo test subcommand**

Use the cargo-test-as-subprocess pattern (Nimbus Runtime style): spawn `cargo test` targeting only the hold-lock helper. But since we can't easily invoke a non-test function from `cargo test`, use an env-var gate instead:

Better approach — add a second #[test] function that serves as the child:

Then in test_probe_locked_file, spawn the test binary:

Mark `hold_lock_child` with `#[ignore]` so it only runs when explicitly invoked.

**Step 3: Assert setup failures loudly**

- If spawn fails → `panic!", not silent return
- If child exits before assertions complete → verify via `child.try_wait()`
- After kill, assert child exited successfully

**Step 4: Verify**

Run: \`nix develop -c cargo test -p nom-core -- --nocapture storage::lock_probe\`
Confirm: locked-file assertions execute (no 'skipping' message), both assertions pass (is_locked=true during child, is_locked=false after kill).

Also run full suite: \`nix develop -c cargo test -p nom-core\`

### Files Changed
- nom-core/src/storage/lock_probe.rs (test module only — no production code changes)
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Replaced broken rustc compile-from-string approach with dedicated lock_holder binary target. The old test compiled ad-hoc Rust source without libc linking (always failed silently). New approach: spawned lock_holder binary (properly linked by Cargo) holds an exclusive write lock via fcntl/F_SETLKW; parent verifies probe_db_lock detects it (is_locked=true), kills child, then verifies lock release (is_locked=false). All 132 tests pass.

Fixup applied post-review: lock_probe.rs and lock_holder.rs failed cargo fmt --all --check as committed, which would fail the project's CI fmt gate. Ran rustfmt on both files (formatting only, no logic change).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Replaced silently-failing test_probe_locked_file with working lock detection test. Created dedicated lock_holder binary (nom-core/src/lock_holder.rs) that acquires an exclusive fcntl write lock and sleeps. Test spawns this binary, verifies probe_db_lock detects the held lock (true), kills child, then verifies release (false). All 4 acceptance criteria verified: no rustc dependency, loud failures on setup problems, assertions actually execute, full nom-core suite passes (132 tests).
<!-- SECTION:FINAL_SUMMARY:END -->
