---
id: TASK-20
title: >-
  Fix: compute_portion_macros/compute_totals return a bare positional f64 tuple,
  reintroducing the transposition hazard TASK-19 fixed on the input side
status: To Do
assignee: []
created_date: '2026-08-12 22:47'
labels:
  - review-followup
dependencies:
  - TASK-19
priority: high
ordinal: 220
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while reviewing TASK-16/TASK-19 (nom-core/src/meal/mod.rs:129-134 compute_portion_macros returns (f64,f64,f64,f64,f64); :155-156 compute_totals takes &[(f64,f64,f64,f64,f64)]). TASK-19 grouped the calories/protein/carbs/fat/fiber *input* params into the NutrientValues struct specifically to close a transposition hazard (swapping fat and fiber, or carbs and protein, compiles silently and corrupts nutrition data), but explicitly left the *return* type as a bare 5-tuple ('minimal ripple, not in scope per ACs'). The same hazard now lives on the output side: three non-test call sites destructure this tuple positionally by pattern -- compute_totals's 'for (cal, prot, carb, fat, fiber) in portions' loop (meal/mod.rs:167), build_meal_summary's 'let (cal, prot, carb, fat, fiber) = compute_portion_macros(...)' (meal/mod.rs:582), and UpdateMeal's adjustment-only recompute pushing directly into all_macros (meal/mod.rs:1063) which compute_totals later destructures. A reordered destructure or a reordered tuple construction at any of these sites would compile and silently swap nutrition fields, identical in kind to the bug class TASK-12 and TASK-19 were filed to close. This is a Correctness/Resilience finding (CLAUDE.md 'General-Purpose Interfaces' / 'Define Errors Out of Existence' -- make the illegal transposition state unrepresentable rather than trusting positional discipline).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 compute_portion_macros returns a named struct (reuse the existing NutrientValues struct from nom-core/src/food/mod.rs, matching the pattern TASK-19 already established for its input parameter) instead of a bare (f64,f64,f64,f64,f64) tuple
- [ ] #2 compute_totals's portions parameter is &[NutrientValues] (or equivalent named type) instead of &[(f64,f64,f64,f64,f64)], and its accumulation loop accesses named fields instead of positional tuple destructuring
- [ ] #3 the three call sites (compute_totals's loop at meal/mod.rs:167, build_meal_summary at meal/mod.rs:582, and UpdateMeal's adjustment-only recompute around meal/mod.rs:1063) are updated accordingly; no positional 5-f64-tuple destructuring of macro values remains in non-test code
- [ ] #4 nix develop -c cargo clippy --workspace --all-targets is clean
- [ ] #5 nix develop -c cargo test -p nom-core passes
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
SETUP (read first): This is a Rust+WebAssembly core (crates/gql-core) with a TypeScript/React web app (web/). ALL commands must run inside the Nix dev shell: either run 'direnv allow' once, or prefix every command with 'nix develop -c'. Work from the repository root unless told otherwise. Do not change pinned dependency versions.

Pure refactor -- zero behavioral change. Reuse the existing NutrientValues struct from nom-core/src/food/mod.rs (already made pub(crate) with pub(crate) fields by TASK-19) instead of creating a new type.

1. Open nom-core/src/meal/mod.rs and change compute_portion_macros's return type (line ~134) from '(f64, f64, f64, f64, f64)' to 'NutrientValues'. Update its body (lines ~144-151) to construct 'NutrientValues { calories: ..., protein_g: ..., carbs_g: ..., fat_g: ..., fiber_g: ... }' instead of a tuple literal.
2. Change compute_totals's signature (line ~155-158) from 'portions: &[(f64, f64, f64, f64, f64)]' to 'portions: &[NutrientValues]'. Update the accumulation loop (line ~167) from 'for (cal, prot, carb, fat, fiber) in portions { totals.total_calories += cal; ... }' to iterate NutrientValues and accumulate via named fields (e.g. 'for n in portions { totals.total_calories += n.calories; totals.total_protein_g += n.protein_g; ... }').
3. Update the MacroTuple type alias (meal/mod.rs:357) -- either remove it and use NutrientValues directly everywhere it's used (resolve_portions's Vec<MacroTuple> return, all_macros accumulation), or repoint the alias to NutrientValues. Prefer removing the alias if NutrientValues is used consistently, since a second name for the same type adds an interaction point without value.
4. Update resolve_portions (meal/mod.rs:363-419ish) so 'all_macros: Vec<NutrientValues>' accumulates struct values from compute_portion_macros directly.
5. Update build_meal_summary (meal/mod.rs:582): replace 'let (cal, prot, carb, fat, fiber) = compute_portion_macros(...)' with 'let macros = compute_portion_macros(...)' and use macros.calories / macros.protein_g / macros.carbs_g / macros.fat_g / macros.fiber_g when constructing PortionSummary.
6. Update UpdateMeal's adjustment-only recompute (meal/mod.rs ~1063): 'all_macros.push(compute_portion_macros(quantity, &qty_mode, ss, snapshot_nutrients))' already pushes the return value directly -- no change needed there beyond the type change flowing through, but confirm all_macros's declared type (search nearby, likely 'Vec<MacroTuple>' or 'Vec<(f64,f64,f64,f64,f64)>') is updated to 'Vec<NutrientValues>'.
7. Update the 3 unit tests for compute_portion_macros (meal/mod.rs ~1528-1573) that currently destructure the tuple return ('let (cal, prot, carb, fat, fiber) = compute_portion_macros(...)') to access named fields on the returned NutrientValues instead.
8. Run: nix develop -c cargo fmt -p nom-core
9. Run: nix develop -c cargo clippy --workspace --all-targets -- confirm clean, no new warnings
10. Run: nix develop -c cargo test -p nom-core -- confirm all 154 tests pass
<!-- SECTION:PLAN:END -->
