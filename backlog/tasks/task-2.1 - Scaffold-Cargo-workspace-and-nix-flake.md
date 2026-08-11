---
id: TASK-2.1
title: Scaffold Cargo workspace and nix flake
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-11 13:22'
updated_date: '2026-08-11 16:49'
labels: []
dependencies: []
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
- [x] #1 cargo build --workspace succeeds with nom-core lib and two binary stubs (main + nom-mcp-remote)
- [x] #2 nix build .#nom-mcp and nix build .#nom-mcp-remote succeed
- [x] #3 nix develop .#ci provides cargo, rustfmt, and clippy
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
### Files to create/modify

**Root level:**
- `Cargo.toml` — virtual workspace manifest with `[workspace]`, members, shared dependencies
- `flake.nix` — full crane-based build (replaces cookiecutter stub)
- `.gitignore` — add Rust artifacts (`target/`, `**/*.rs.bk`, `.cargo/`) if not present

**Crate: `nom-core` (library):**
- `nom-core/Cargo.toml` — library crate, workspace dependency inheritance
- `nom-core/src/lib.rs` — minimal lib stub with module structure comments

**Crate: `nom-mcp` (binary with two bins):**
- `nom-mcp/Cargo.toml` — binary crate with two `[[bin]]` targets: `nom-mcp` (main) and `nom-mcp-remote` (thin HTTP client)
- `nom-mcp/src/main.rs` — main binary stub (serve + local CLI entry point)
- `nom-mcp/src/bin/nom-mcp-remote.rs` — remote-CLI thin binary stub

### Step-by-step

1. **Create root `Cargo.toml`** — virtual manifest with `members = ["nom-core", "nom-mcp"]`, shared dependencies in `[workspace.dependencies]` (edition 2024, MSRV 1.85). No `[package]` block.

2. **Create `nom-core/Cargo.toml`** — library crate, no dependencies yet (will inherit workspace deps later).

3. **Create `nom-core/src/lib.rs`** — empty lib with doc comment describing future modules (operations, storage, clients, clock).

4. **Create `nom-mcp/Cargo.toml`** — binary crate with two `[[bin]]` entries. Depends on `nom-core`. Minimal dependencies for now.

5. **Create `nom-mcp/src/main.rs`** — `fn main()` stub that prints a placeholder message.

6. **Create `nom-mcp/src/bin/nom-mcp-remote.rs`** — `fn main()` stub that prints a placeholder message.

7. **Verify `cargo build --workspace`** succeeds.

8. **Replace `flake.nix`** — mirror notectl's flake structure:
   - Inputs: nixpkgs-unstable, flake-utils, rust-overlay (oxalica), crane
   - Two crane packages: `nom-mcp` and `nom-mcp-remote`, each built via `craneLib.buildPackage` with `buildDepsOnly` artifacts
   - Mold linker on Linux, clang as linker
   - Split devShells: default (full dev tools) and ci (lean)
   - nixpkgs-fmt formatter
   - Use `fileSetForCrate` for targeted source sets per crate

9. **Verify `nix build .#nom-mcp` and `nix build .#nom-mcp-remote`** succeed.

10. **Verify `nix develop .#ci -c cargo --version`** works.

### Key decisions from research

- **Flat layout** (not `crates/` subdirectory) — matklad's guidance for ≤3 crates
- **Virtual manifest** at root — avoids polluting root with `src/`
- **One crane package per binary** — mirrors notectl exactly
- **No over-engineering** — no cargo-hakari, deny, audit checks yet
- **Crate names match directory names** — single canonical naming convention
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Created two-crate Cargo workspace (nom-core library + nom-mcp binary with two bin targets: nom-mcp and nom-mcp-remote). Replaced cookiecutter flake.nix with crane-based build mirroring notectl — uses nixpkgs-unstable, oxalica rust-overlay, mold linker on Linux, split default/ci devShells, nixpkgs-fmt formatter. Generated Cargo.lock for workspace.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Scaffolded Cargo workspace with nom-core (lib) and nom-mcp (binary crate, two bin targets: nom-mcp + nom-mcp-remote). Replaced cookiecutter flake.nix with crane-based build using nixpkgs-unstable, oxalica rust-overlay, mold linker on Linux, split default/ci devShells, nixpkgs-fmt formatter. All three acceptance criteria verified: cargo build, nix build for both binaries, and ci devShell with cargo/rustfmt/clippy.
<!-- SECTION:FINAL_SUMMARY:END -->
