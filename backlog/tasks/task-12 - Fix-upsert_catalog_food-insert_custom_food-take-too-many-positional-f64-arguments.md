---
id: TASK-12
title: >-
  Fix: upsert_catalog_food/insert_custom_food take too many positional f64
  arguments
status: To Do
assignee: []
created_date: '2026-08-12 05:29'
labels:
  - review-followup
dependencies:
  - TASK-2.13
priority: high
ordinal: 160
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while reviewing TASK-2.13 (nom-core/src/food/mod.rs:86-149 upsert_catalog_food, 10 args; :155-203 insert_custom_food, 8 args). Both functions take calories/protein/carbs/fat/fiber as five consecutive same-typed f64 positional parameters (plus source/external_id/name as consecutive &str params on upsert_catalog_food), flagged by clippy::too_many_arguments (nix develop -c cargo clippy --workspace --all-targets: 'this function has too many arguments (10/7)' and '(8/7)'). All current call sites happen to pass arguments in the correct order, but this shape is a latent transposition hazard per the project's 'General-Purpose Interfaces' design philosophy (CLAUDE.md) — swapping fat and fiber, or carbs and protein, at a future call site would compile silently and corrupt stored nutrition data with no error. FoodCandidate already has the right shape (a struct with named fields) for this data; these two DB-helper functions should take the same kind of grouped value instead of five loose f64s.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 upsert_catalog_food and insert_custom_food each take a single grouped nutrients parameter (e.g. a shared NutrientValues { calories, protein_g, carbs_g, fat_g, fiber_g } struct, or reuse/derive from FoodCandidate's fields) instead of 5 individual f64 parameters
- [ ] #2 nix develop -c cargo clippy --workspace --all-targets no longer reports too_many_arguments for these two functions
- [ ] #3 all existing call sites in food/mod.rs are updated and nix develop -c cargo test -p nom-core passes
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
SETUP (read first): This is a Rust+WebAssembly core (crates/gql-core) with a
TypeScript/React web app (web/). ALL commands must run inside the Nix dev
shell: either run 'direnv allow' once, or prefix every command with
'nix develop -c'. Work from the repository root unless told otherwise. Do not
change pinned dependency versions.

Note: this repo's actual crate layout is nom-core/ and nom-mcp/ (not crates/gql-core — ignore that path in the preamble; everything else in the preamble still applies).

1. Read nom-core/src/food/mod.rs in full, focusing on FoodCandidate (~line 26), upsert_catalog_food (~line 86), insert_custom_food (~line 155), and their three call sites (search_barcode ~line 427, search_free_text's USDA branch ~line 493, CreateCustomFood::execute_json ~line 661).
2. Add a small struct (e.g. 'struct NutrientValues { calories: f64, protein_g: f64, carbs_g: f64, fat_g: f64, fiber_g: f64 }') near FoodCandidate, or evaluate whether FoodCandidate's own fields can be constructed first and passed in instead (may require restructuring so FoodCandidate is built before the DB call rather than after, since currently the id comes back from the DB call and FoodCandidate is built last — a plain NutrientValues struct passed in, with FoodCandidate built after using the returned id, is likely the smaller change).
3. Change upsert_catalog_food and insert_custom_food signatures to take 'nutrients: &NutrientValues' (or by value, whichever avoids unnecessary clones — these are Copy-able f64s so by-value is fine) instead of the 5 individual f64 params.
4. Update all 3 call sites accordingly.
5. Run: nix develop -c cargo clippy --workspace --all-targets (confirm the too_many_arguments warnings for these two functions are gone), nix develop -c cargo test -p nom-core, nix develop -c cargo fmt --check.
<!-- SECTION:PLAN:END -->
