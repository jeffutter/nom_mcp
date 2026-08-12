---
id: TASK-16
title: >-
  Fix: deduplicate portion validation/snapshot-lookup logic shared by log_meal
  and update_meal
status: To Do
assignee: []
created_date: '2026-08-12 20:21'
labels:
  - review-followup
dependencies:
  - TASK-2.14
priority: high
ordinal: 180
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while reviewing TASK-2.14 (nom-core/src/meal/mod.rs). LogMeal's portion-processing loop (validate quantity_mode, validate quantity > 0, lookup_food, compute_portion_macros, accumulate into all_macros/snapshots -- around lines 664-681) is nearly byte-for-byte duplicated in UpdateMeal's portion-replacement branch (around lines 908-922). This is a Conciseness axis violation -- exactly the kind of repetition the file's own insert_portion/insert_meal helpers were meant to avoid -- and is a meaningful contributor to the file landing at roughly double its planned line count. A bug fix or validation change applied to one copy (as already nearly happened with the DeleteMeal transaction gap found in the same review) is easy to forget applying to the other.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A single shared async helper function (e.g. resolve_portions(conn: &Connection, portions: &[PortionInput]) -> Result<(Vec<macros-tuple>, Vec<snapshot-tuple>), ErrorData>) exists in nom-core/src/meal/mod.rs encapsulating the validate-quantity_mode / validate-quantity>0 / lookup_food / compute_portion_macros / accumulate sequence
- [ ] #2 Both LogMeal::execute_json and UpdateMeal::execute_json call this shared helper instead of each containing their own copy of the loop
- [ ] #3 All existing meal tests pass unchanged (no behavior change, pure extraction)
- [ ] #4 nix develop -c cargo test -p nom-core passes
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
SETUP (read first): This is a Rust+WebAssembly core (nom-core, nom-mcp) with SQLite storage via the turso crate. ALL commands must run inside the Nix dev shell: either run 'direnv allow' once, or prefix every command with 'nix develop -c'. Work from the repository root unless told otherwise. Do not change pinned dependency versions.

1. Open nom-core/src/meal/mod.rs. Locate LogMeal's portion loop (search for 'if portion.quantity_mode != "grams"', first occurrence, around line 664) and UpdateMeal's near-identical copy (second occurrence of the same search, around line 908). Read both in full including their surrounding all_macros/snapshots Vec declarations and the exact tuple types pushed into each.
2. Extract a new async fn resolve_portions(conn: &Connection, portions: &[PortionInput]) -> Result<(Vec<(f64,f64,f64,f64,f64)>, Vec<(i64,String,f64,f64,f64,f64,f64,f64,Option<f64>)>), ErrorData> (match the exact existing tuple field types/order used today -- do not change the tuple shape, just relocate the logic) placed near the other free functions in this file (lookup_food, compute_portion_macros, compute_totals are good neighbors).
3. Move the validation (quantity_mode check, quantity > 0 check), lookup_food call, compute_portion_macros call, and accumulation-into-two-vecs logic from both call sites into this function body, looping over the input portions slice once.
4. Replace LogMeal's loop with a single call: let (all_macros, snapshots) = resolve_portions(&conn, &req.portions).await?;
5. Replace UpdateMeal's loop the same way, using whatever variable currently holds the new portions array in that branch.
6. Note: if PortionInput's quantity_mode field is a &str/String borrowed from the request in the current tuples (check the exact snapshot tuple's second field type), make sure ownership works cleanly when the helper takes a slice reference and returns owned data -- clone quantity_mode into an owned String in the tuple if needed rather than fighting the borrow checker with lifetimes.
7. Run: nix develop -c cargo test -p nom-core -- meal:: --nocapture and confirm all meal tests pass unchanged, then the full suite: nix develop -c cargo test -p nom-core, and nix develop -c cargo clippy -p nom-core --all-targets
<!-- SECTION:PLAN:END -->
