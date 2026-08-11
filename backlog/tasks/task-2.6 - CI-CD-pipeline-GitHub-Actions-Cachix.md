---
id: TASK-2.6
title: CI/CD pipeline (GitHub Actions + Cachix)
status: To Do
assignee: []
created_date: '2026-08-11 13:23'
labels: []
dependencies:
  - TASK-2.1
parent_task_id: TASK-2
type: chore
ordinal: 25000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Scope
Mirror jeffutter/notectl's .github/workflows/{ci,cd,audit}.yml directly:
- CI: on push to main + PRs — test/rustfmt/clippy/docs jobs, each via 'nix develop .#ci -c cargo ...', using DeterminateSystems/nix-installer-action + magic-nix-cache-action + Swatinem/rust-cache.
- CD: on a semver git tag — cross-builds release binaries (macos-aarch64, linux-x86_64, linux-aarch64), strips+tars+sha256s them, then cachix/install-nix-action + cachix/cachix-action (cache name 'jeffutter') runs nix build .#nom-mcp / .#nom-mcp-remote + nix flake check, pushing to the jeffutter Cachix cache; tarballs+shasums attached to a GitHub Release via softprops/action-gh-release.
- audit: daily cron + Cargo.toml/lock-touching pushes/PRs, cargo-audit via rustsec/audit-check.

See doc-5 §12 (corrected per TASK-1.15).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 ci.yml runs test/rustfmt/clippy/docs jobs on push to main and on PRs
- [ ] #2 cd.yml triggers on a semver tag, builds cross-platform release artifacts, and pushes to the jeffutter Cachix cache
- [ ] #3 audit.yml runs cargo-audit on a daily cron and on Cargo.toml/Cargo.lock changes
<!-- AC:END -->
