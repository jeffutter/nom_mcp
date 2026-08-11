---
id: TASK-2.1
title: Scaffold Cargo workspace and nix flake
status: To Do
assignee: []
created_date: '2026-08-11 13:22'
labels: []
dependencies: []
parent_task_id: TASK-2
type: chore
ordinal: 20000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Scope
Two-crate workspace: unified nom-core library (Operation trait, capability logic, storage access, external API client modules) plus a binary package with two bin targets — the main binary (serve + local CLI) and nom-mcp-remote (thin HTTP client). Nix flake mirrors jeffutter/notectl's flake.nix: nixpkgs-unstable, oxalica rust-overlay (rust-bin.stable.latest.default), crane for the build (one crane package per binary), mold linker on Linux, split default/ci devShells, nixpkgs-fmt formatter. Replaces the current cookiecutter-stub flake.nix.

See doc-5 §3 (multi-surface architecture) and §12 (build tooling).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 cargo build --workspace succeeds with nom-core lib and two binary stubs (main + nom-mcp-remote)
- [ ] #2 nix build .#nom-mcp and nix build .#nom-mcp-remote succeed
- [ ] #3 nix develop .#ci provides cargo, rustfmt, and clippy
<!-- AC:END -->
