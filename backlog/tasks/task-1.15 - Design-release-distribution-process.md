---
id: TASK-1.15
title: Design release/distribution process
status: Done
assignee:
  - '@Jeffery Utter'
created_date: '2026-08-11 13:11'
updated_date: '2026-08-11 13:14'
labels:
  - 'wayfinder:grilling'
dependencies: []
parent_task_id: TASK-1
ordinal: 16000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Question

Beyond nix flake packaging (already noted), what versioning scheme applies and how does a built binary actually reach and run on the machine that hosts nom_mcp?
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Rationale: matches the single-user, self-hosted scope already settled for auth/multi-user (Out of scope) and config (TASK-1.12). Building CI/CD or a release pipeline for a server one person runs on their own machine is unjustified complexity — nix's existing build/rollback story is sufficient.

## Correction (post-resolution)

Original recommendation (no CI/CD, build ad hoc on/near the host) reversed after the user asked to mirror notectl's existing pipeline instead. Read notectl's actual .github/workflows/{ci,cd,audit}.yml directly (not the flake.nix already cited in this map's Notes) to source the concrete job/action list above, rather than guessing at a plausible one.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Corrected: mirror notectl's GitHub Actions setup directly rather than skipping CI/CD. Three workflows: (1) CI — on push to main + PRs, four jobs (test, rustfmt, clippy, docs) each run inside 'nix develop .#ci -c cargo ...', using DeterminateSystems/nix-installer-action + magic-nix-cache-action + Swatinem/rust-cache for fast nix/cargo caching. (2) CD — triggered on a semver git tag push; cross-compiles release binaries for macos-aarch64, linux-x86_64, and linux-aarch64, strips debug symbols, tars + sha256s them, then uses cachix/install-nix-action + cachix/cachix-action (cache name 'jeffutter') to run 'nix build .#nom-mcp' and '.#nom-mcp-remote' (TASK-1.6's two crane packages) plus 'nix flake check', pushing those build closures into the jeffutter Cachix binary cache; tarballs+shasums are attached to a GitHub Release via softprops/action-gh-release. (3) Security audit — daily cron plus Cargo.toml/Cargo.lock-touching pushes/PRs, cargo-audit via rustsec/audit-check. Versioning stays single-workspace semver, but now the version tag IS the CD trigger. Deploy on the host machine becomes 'nix build --accept-flake-config ...', which pulls the prebuilt closure from the jeffutter Cachix cache instead of rebuilding from source — same self-hosted, no-package-registry model as before, just with CI verification and a warm binary cache.
<!-- SECTION:FINAL_SUMMARY:END -->
