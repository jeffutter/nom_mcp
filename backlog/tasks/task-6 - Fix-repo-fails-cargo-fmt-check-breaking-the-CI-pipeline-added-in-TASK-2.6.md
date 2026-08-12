---
id: TASK-6
title: 'Fix: repo fails cargo fmt --check, breaking the CI pipeline added in TASK-2.6'
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-11 23:54'
updated_date: '2026-08-11 23:59'
labels:
  - review-followup
dependencies:
  - TASK-2.6
priority: high
type: bug
ordinal: 90
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while reviewing TASK-2.6 (.github/workflows/ci.yml), TASK-3, and TASK-4. TASK-2.6 wired a required 'rustfmt' job (cargo fmt --all --check) into ci.yml, but the nom-core tree does not currently pass it: nom-core/src/client/off.rs, nom-core/src/operation/cli_router.rs, nom-core/src/operation/http_router.rs, and nom-core/src/operation/registry.rs all have unformatted hunks (manual multi-line closures/match arms collapsed to one line, import ordering, one-line fn bodies not expanded). This predates TASK-2.6 itself (introduced by earlier commits, notably the 'fixup! TASK-2.7: Fix compilation errors' commit) but TASK-2.6's new CI job means the very next push or PR will fail its rustfmt check. This is a Resilient/Correct-axis gap: CI as shipped cannot pass against the tree it was added to.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 #1 nix develop -c cargo fmt --all --check exits 0 with no diffs: Done|#2 nix develop -c cargo test --workspace passes: Done|#3 nix develop -c cargo clippy --all-targets --all-features --workspace -- -D warnings passes: Done
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
SETUP (read first): This is a Rust Cargo workspace with two crates: nom-core/ (library) and nom-mcp/ (binaries). ALL commands must run inside the Nix dev shell: either run 'direnv allow' once, or prefix every command with 'nix develop -c'. Work from the repository root unless told otherwise. Do not change pinned dependency versions.

1. Run: nix develop -c cargo fmt --all --check   to see the current full diff (files: nom-core/src/client/off.rs, nom-core/src/operation/cli_router.rs, nom-core/src/operation/http_router.rs, nom-core/src/operation/registry.rs, and any others reported).
2. Run: nix develop -c cargo fmt --all   to apply rustfmt's fixes. This is purely mechanical formatting — do not hand-edit logic while doing this.
3. Run: nix develop -c cargo fmt --all --check   again and confirm it now exits 0 with no diff output.
4. Run: nix develop -c cargo test --workspace   and confirm all tests still pass (formatting must not change behavior).
5. Run: nix develop -c cargo clippy --all-targets --all-features --workspace -- -D warnings   and confirm it stays clean.
6. Commit the formatting-only diff.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Applied cargo fmt --all to fix formatting in 4 files (off.rs, cli_router.rs, http_router.rs, registry.rs). All 70 tests pass, clippy is clean.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Ran cargo fmt --all to fix formatting in 4 source files, resolving the rustfmt CI check that would fail on every push/PR. All acceptance criteria met: fmt check passes (exit 0), all 70 tests pass, clippy is clean with no warnings.
<!-- SECTION:FINAL_SUMMARY:END -->
