---
id: TASK-29
title: >-
  Fix: TestGuard::drop() never actually restores a pre-existing XDG_CONFIG_HOME,
  duplicated verbatim across two crates
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-13 02:07'
updated_date: '2026-08-13 05:25'
labels:
  - review-followup
dependencies:
  - TASK-22
priority: high
ordinal: 260
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while reviewing TASK-22 (nom-mcp/src/bin/nom-mcp-remote.rs:100-144), which copy-pasted the TestGuard struct/impl/Drop verbatim from nom-core/src/config.rs:298-343 (an older pattern from TASK-2.3, predating this review round) into a second crate. Both copies share the same real bug: Drop::drop() first restores a saved XDG_CONFIG_HOME via 'if let Some(saved) = &self.saved_xdg { ... set_var("XDG_CONFIG_HOME", saved) ... }', then unconditionally loops over 'self.cleared_vars' and calls remove_var on every entry — but every test calls 'guard.set("XDG_CONFIG_HOME", ...)' first, which pushes "XDG_CONFIG_HOME" into cleared_vars via TestGuard::set(). The second loop therefore immediately un-does the restore the first block just performed, unconditionally removing XDG_CONFIG_HOME on every drop regardless of whether it had a real prior value. Verified directly by reading both files — the two Drop impls are byte-identical and both exhibit this. Resilience-axis finding: in the common case (no XDG_CONFIG_HOME set before the test run, e.g. most CI environments) the bug is invisible because the end state (unset) happens to match the intended restore target, which is presumably why it has gone unnoticed through two rounds of tests being added against this fixture. But on any machine/CI where XDG_CONFIG_HOME is legitimately set beforehand (common on Linux dev machines, some CI runners), every test using this guard leaves it permanently unset for the remainder of the test binary's process, silently changing behavior for any other test in the same binary that reads config from XDG_CONFIG_HOME. Organization-axis finding: the ~40-line TestGuard block is duplicated verbatim across nom-core and nom-mcp instead of being a single shared piece of test-support knowledge, so this same bug now has to be fixed in two places (and will drift again the next time either copy is touched).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 TestGuard::drop() in nom-core/src/config.rs no longer removes XDG_CONFIG_HOME as part of the generic cleared_vars loop when a prior value was successfully restored — e.g. by excluding "XDG_CONFIG_HOME" from the generic remove loop (the earlier restore branch already owns that variable's final state)
- [x] #2 A new test in nom-core/src/config.rs's #[cfg(test)] mod tests proves the bug is fixed: set XDG_CONFIG_HOME to a known value before constructing TestGuard, use the guard to point at a different temp dir, drop the guard, and assert XDG_CONFIG_HOME is back to the original known value (not unset)
- [x] #3 nom-mcp/src/bin/nom-mcp-remote.rs's TestGuard is no longer a separate verbatim copy — either it re-uses a shared implementation exposed from nom-core (e.g. via a #[cfg(test)] pub test_support module gated behind a dev-dependency on nom-core with its test-support feature enabled) or, if cross-crate sharing proves impractical within this ticket's scope, both copies are fixed identically and a comment on each references the other as the sibling implementation that must stay in sync
- [x] #4 nix develop -c cargo test -p nom-core passes
- [x] #5 nix develop -c cargo test -p nom-mcp passes
- [x] #6 nix develop -c cargo clippy --workspace --all-targets is clean
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
SETUP (read first): This is a Rust workspace (nom-core, nom-mcp, nom-mcp-http; no WASM/web component in this repo). ALL commands must run inside the Nix dev shell: either run 'direnv allow' once, or prefix every command with 'nix develop -c'. Work from the repository root unless told otherwise. Do not change pinned dependency versions.

1. In nom-core/src/config.rs, find TestGuard::drop() (around :324-343). Change the second loop from:
   'for var in &self.cleared_vars { unsafe { std::env::remove_var(var) }; }'
   to skip "XDG_CONFIG_HOME" specifically, since the preceding block already restored (or correctly left unset) that variable:
   'for var in &self.cleared_vars { if var == "XDG_CONFIG_HOME" { continue; } unsafe { std::env::remove_var(var) }; }'
2. Add a new #[serial_test::serial] test in nom-core/src/config.rs's mod tests (near the existing TestGuard-using tests, e.g. after :422): set_var("XDG_CONFIG_HOME", "/tmp/nom_mcp_original_value_marker") directly (not via the guard) before constructing TestGuard, then 'let mut guard = TestGuard::new(); guard.set("XDG_CONFIG_HOME", "/tmp/nom_mcp_test_scratch");', do nothing else, explicitly 'drop(guard);', then assert 'std::env::var("XDG_CONFIG_HOME").unwrap() == "/tmp/nom_mcp_original_value_marker"'. Clean up the marker var afterward.
3. Decide the cross-crate sharing approach: the simplest option within scope is to apply the identical one-line fix from step 1 to nom-mcp/src/bin/nom-mcp-remote.rs's TestGuard::drop() (around :128-144) and add a one-line comment on each copy noting the other file as the sibling that must be kept in sync ('// Keep in sync with the identical TestGuard in nom-core/src/config.rs'). If a shared nom-core test-support module can be exposed to nom-mcp's dev-dependencies without disrupting the existing #[cfg(test)] boundaries, prefer that instead — but do not let that design exploration block landing the correctness fix in both places.
4. Run: nix develop -c cargo test -p nom-core
5. Run: nix develop -c cargo test -p nom-mcp
6. Run: nix develop -c cargo clippy --workspace --all-targets
7. Run: nix develop -c cargo fmt --check (both crates)
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Fixup applied post-review (2nd review round): the Drop impl's 'if let Some(saved)' only handled the case where XDG_CONFIG_HOME had a prior value; when there was no prior value (the common case per this ticket's own bug report — most CI environments), the branch did nothing AND the cleared_vars loop was told to skip XDG_CONFIG_HOME unconditionally, so the guard's own set() value leaked past drop() instead of being removed. Fixed by replacing the if-let with an exhaustive match: Some(non-empty) restores, everything else (None or empty) removes. Applied identically to nom-mcp/src/bin/nom-mcp-remote.rs's sibling copy. Added test_guard_leaves_xdg_config_home_unset_when_no_prior_value to nom-core/src/config.rs proving the fix. nix develop -c cargo test -p nom-core -p nom-mcp passes (185+7), clippy clean, fmt clean.
<!-- SECTION:NOTES:END -->
