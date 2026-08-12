---
id: TASK-18
title: >-
  Fix: eliminate nested cargo build and racy synchronization in lock_probe's
  locked-file test
status: To Do
assignee: []
created_date: '2026-08-12 20:22'
updated_date: '2026-08-12 21:36'
labels:
  - review-followup
dependencies:
  - TASK-10
priority: high
ordinal: 200
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while reviewing TASK-10 (nom-core/src/storage/lock_probe.rs). TASK-10 replaced a silently-broken test with one that genuinely spawns a child process holding an fcntl lock, but independent verification (running it repeatedly, not just reading the diff) found three real, coupled defects in that new harness, all stemming from how the child process is located and synchronized -- fixing them together via one redesign is more coherent than three separate patches to the same function: (1) find_lock_holder() (lines 51-58) shells out to 'cargo build --bin lock_holder' synchronously from inside the test on every single run -- verified reproducible: three consecutive runs of just this test, with zero source changes between them, each recompiled ring/rustls/rustls-webpki/tokio-rustls/hyper-rustls/reqwest/nom-core from scratch, and one run printed 'Blocking waiting for file lock on build directory', proving real build-lock contention with the outer cargo test/nextest invocation. This directly undercuts the ticket's own AC1 ('without compiling ad-hoc source at test time') -- it still compiles at test time, just via cargo build instead of rustc. (2) The parent's only signal that the child has acquired the fcntl lock is a fixed 200ms sleep (line 126) before calling probe_db_lock -- no ack pipe, stdout marker, or file signal confirms the child actually reached F_SETLKW. On a throttled/noisy CI runner this is a latent flake: a lost race fails the assertion with a message that looks like a probe_db_lock bug rather than a test-timing issue. (3) The spawned Child (created ~line 120, killed ~line 142) is never wrapped in a Drop guard or catch_unwind -- any panic between spawn and kill (including one caused by defect #2's race) leaks an orphaned lock_holder process holding an exclusive lock on the about-to-be-deleted TempDir file for up to its 300s self-timeout.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 test_probe_locked_file no longer invokes 'cargo build' (or any Command::new("cargo")) at test-run time; the lock_holder binary is located via a mechanism that does not trigger recompilation on a cached build (e.g. std::env::var("CARGO_BIN_EXE_lock_holder") if the test is moved under nom-core/tests/ as an integration test, since CARGO_BIN_EXE_<name> is only populated for integration tests -- or another approach that avoids invoking cargo build from within the test process; document whichever approach is chosen and why)
- [ ] #2 The parent test no longer relies on a fixed sleep to infer the child has acquired the lock -- it waits on an explicit signal from the child (e.g. the child writes a byte to its stdout, or to a dedicated pipe/fd, immediately after a successful F_SETLKW, and the parent blocks reading that signal with a bounded timeout before calling probe_db_lock)
- [ ] #3 The spawned child process is guaranteed to be killed even if an assertion panics between spawn and kill (e.g. via a scope guard / Drop-based wrapper type that kills the child in its Drop impl, or an explicit catch_unwind around the assertions with a kill-then-resume-unwind pattern)
- [ ] #4 nix develop -c cargo test -p nom-core -- --nocapture storage::lock_probe run 3 times consecutively all show the locked-file assertion executing (not a 'skipping' message) and none of the 3 runs recompile any dependency crate (verify by checking for 'Compiling' lines in the test output on the 2nd and 3rd runs -- there should be none)
- [ ] #5 nix develop -c cargo test -p nom-core passes
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
SETUP (read first): This is a Rust+WebAssembly core (nom-core, nom-mcp). ALL commands must run inside the Nix dev shell: either run 'direnv allow' once, or prefix every command with 'nix develop -c'. Work from the repository root unless told otherwise. Do not change pinned dependency versions.

1. Read nom-core/src/storage/lock_probe.rs in full, especially find_lock_holder() (~lines 51-75) and test_probe_locked_file (~lines 110-148), and nom-core/src/lock_holder.rs in full, and the [[bin]] section in nom-core/Cargo.toml.
2. Decide on binary location strategy: the cleanest fix is moving test_probe_locked_file out of the #[cfg(test)] unit-test module in src/storage/lock_probe.rs into a new integration test file nom-core/tests/lock_probe_integration.rs, because Cargo only populates the CARGO_BIN_EXE_<binname> environment variable (here CARGO_BIN_EXE_lock_holder) for integration tests under tests/, not for unit tests under src/. This lets you delete find_lock_holder()'s entire directory-walking/cargo-build logic and replace it with std::env::var("CARGO_BIN_EXE_lock_holder").expect(...) (expect is fine here -- this is test setup, not the production probe_db_lock path). Whatever remains callable from probe_db_lock's own unit tests in lock_probe.rs (if any) should stay; only test_probe_locked_file and its helper move.
3. In lock_holder.rs (or a small addition to it), after the process successfully acquires the fcntl lock (after the F_SETLKW call succeeds), write a single byte to stdout and flush it, before starting the sleep. This is the ack signal.
4. In the new/moved test_probe_locked_file, after spawning the child with Command::new(...).stdout(Stdio::piped()).spawn(), read exactly one byte from the child's stdout handle (with a bounded timeout -- e.g. spawn a thread to do the blocking read and join it with a timeout, or use a non-blocking read loop with a deadline) before calling probe_db_lock. Treat a timeout or read error as a loud test failure (panic with a clear message), not a silent skip.
5. Wrap the Child in a small RAII guard type (e.g. struct KillOnDrop(std::process::Child); impl Drop for KillOnDrop { fn drop(&mut self) { let _ = self.0.kill(); let _ = self.0.wait(); } }) so any panic between spawn and the explicit kill still cleans up the process. Use the explicit kill()+wait() in the normal success path as before (the Drop guard is the safety net for the panic path, not a replacement for the deliberate kill-and-verify-unlocked assertions).
6. Update nom-core/Cargo.toml's [[bin]] section if its path needs adjusting for the new test location (it likely doesn't, since [[bin]] targets are independent of where tests reference them).
7. Run: nix develop -c cargo test -p nom-core -- --nocapture storage::lock_probe (or the new integration test's path) three times in a row and confirm no 'Compiling' output appears on runs 2 and 3, and the locked-file assertions visibly execute each time. Then run the full suite: nix develop -c cargo test -p nom-core, and nix develop -c cargo fmt --check -p nom-core on the touched files.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
AC #1 satisfied as part of a review-round fixup to TASK-10's commit (unpushed, folded via git commit --fixup): find_lock_holder() no longer shells out to 'cargo build' or walks directories manually. Replaced with std::env::current_exe() sibling-path resolution (test binary and lock_holder binary are both under target/<profile>/, one level below/above 'deps'), since CARGO_BIN_EXE_lock_holder is unavailable here (unit test module, not an integration test under tests/ -- moving the test wasn't done, so that path wasn't taken). Verified via three consecutive 'nix develop -c cargo test -p nom-core -- --nocapture storage::lock_probe' runs: no 'Compiling' output on runs 2/3, locked-file assertions execute every run. AC #2 (racy fixed-sleep synchronization) and AC #3 (no Drop-guard/panic safety for the spawned child) are UNCHANGED and still need implementing.
<!-- SECTION:NOTES:END -->
