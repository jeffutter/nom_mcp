---
id: TASK-16
title: >-
  Fix: deduplicate portion validation/snapshot-lookup logic shared by log_meal
  and update_meal
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-12 20:21'
updated_date: '2026-08-12 21:29'
labels:
  - review-followup
  - planned
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
- [x] #1 A single shared async helper function (e.g. resolve_portions(conn: &Connection, portions: &[PortionInput]) -> Result<(Vec<macros-tuple>, Vec<snapshot-tuple>), ErrorData>) exists in nom-core/src/meal/mod.rs encapsulating the validate-quantity_mode / validate-quantity>0 / lookup_food / compute_portion_macros / accumulate sequence
- [x] #2 Both LogMeal::execute_json and UpdateMeal::execute_json call this shared helper instead of each containing their own copy of the loop
- [x] #3 All existing meal tests pass unchanged (no behavior change, pure extraction)
- [x] #4 nix develop -c cargo test -p nom-core passes
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Implementation Plan

### File: nom-core/src/meal/mod.rs (only file changed)

### Step 1: Create `resolve_portions` helper

Add new async function near existing helpers (after `insert_portion`, around line 360):

```rust
async fn resolve_portions(
    conn: &Connection,
    portions: &[PortionInput],
) -> Result<(Vec<(f64,f64,f64,f64,f64)>, Vec<(i64, String, f64, f64, f64, f64, f64, f64, Option<f64>)>), ErrorData> {
    let mut all_macros: Vec<(f64,f64,f64,f64,f64)> = Vec::new();
    let mut snapshots: Vec<(i64, String, f64, f64, f64, f64, f64, f64, Option<f64>)> = Vec::new();

    for portion in portions {
        // Validate quantity_mode
        if portion.quantity_mode != "grams" && portion.quantity_mode != "servings" {
            return Err(ErrorData::validation(
                "quantity_mode",
                format!("must be 'grams' or 'servings' (got '{}')", portion.quantity_mode),
            ));
        }
        // Validate quantity > 0
        if portion.quantity <= 0.0 {
            return Err(ErrorData::validation("quantity", "must be greater than zero"));
        }
        // Lookup food and capture snapshot
        let (_name, snap_cal, snap_prot, snap_carb, snap_fat, snap_fiber, snap_serving) =
            lookup_food(conn, portion.food_id).await?;
        // Compute macros
        let macros = compute_portion_macros(
            portion.quantity, &portion.quantity_mode, snap_serving,
            snap_cal, snap_prot, snap_carb, snap_fat, snap_fiber,
        );
        all_macros.push(macros);
        // Snapshot with owned String for quantity_mode
        snapshots.push((
            portion.food_id,
            portion.quantity_mode.clone(),
            portion.quantity,
            snap_cal, snap_prot, snap_carb, snap_fat, snap_fiber, snap_serving,
        ));
    }
    Ok((all_macros, snapshots))
}
```

**Key type change**: snapshot tuple uses `String` (not `&str`) for quantity_mode so ownership flows cleanly out of the async fn.

### Step 2: Update LogMeal::execute_json (lines ~658-694)

Replace the inline loop with:
```rust
let (all_macros, snapshots) = resolve_portions(&conn, &req.portions).await?;
```

Remove the old Vec declarations and the entire for-loop block. The subsequent insert_meal/insert_portion calls stay unchanged except:
- In the insert_portion loop, borrow the String: `qty_mode.as_str()` instead of bare `qty_mode` reference

### Step 3: Update UpdateMeal::execute_json (lines ~900-932)

In the `if let Some(new_portions) = &req.portions` branch, replace the inline loop with:
```rust
let (all_macros, snapshots) = resolve_portions(&conn, new_portions).await?;
```

Same adjustments as LogMeal for the insert_portion call site (`.as_str()` on the String field).

**Note**: The empty-portions check (`if !new_portions.is_empty()`) moves outside — if the array is empty, `resolve_portions` returns empty Vecs immediately (no validation errors since the loop doesn't execute), which is correct behavior.

### Step 4: Verify and test

- `nix develop -c cargo fmt -p nom-core`
- `nix develop -c cargo clippy -p nom-core --all-targets` (no new warnings)
- `nix develop -c cargo test -p nom-core -- meal::` (all meal tests pass)
- `nix develop -c cargo test -p nom-core` (full suite passes)

### Expected diff summary
- **Added**: ~35 lines (resolve_portions function)
- **Removed**: ~70 lines (two duplicated loops + Vec declarations)
- **Net change**: ~-35 lines in meal/mod.rs
- **Behavior**: Zero behavior change — pure extraction
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Extracted shared portion-processing logic from LogMeal::execute_json and UpdateMeal::execute_json into a single async resolve_portions() helper. Added MacroTuple and SnapshotTuple type aliases to satisfy clippy's type_complexity lint. Zero behavior change — pure extraction. Net reduction of ~35 lines (removed ~70 duplicated lines, added ~35 helper function + type aliases). All 21 meal tests pass unchanged.

Extracted shared portion-processing logic from LogMeal::execute_json and UpdateMeal::execute_json into a single async resolve_portions() helper. Added MacroTuple and SnapshotTuple type aliases for clippy's type_complexity lint. Zero behavior change — pure extraction. Net reduction of ~35 lines (removed ~70 duplicated lines, added ~35 helper + type aliases). All 21 meal tests pass unchanged.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Extracted duplicated portion validation/snapshot-lookup logic from LogMeal and UpdateMeal into a shared resolve_portions() async helper. Added MacroTuple/SnapshotTuple type aliases for clippy compliance. Net -35 lines, zero behavior change, all 21 meal tests pass.
<!-- SECTION:FINAL_SUMMARY:END -->
