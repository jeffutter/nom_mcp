---
id: TASK-9
title: >-
  Fix: create_custom_food mis-converts non-gram serving units to per-100g
  nutrients
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-12 05:27'
updated_date: '2026-08-12 18:58'
labels:
  - review-followup
  - planned
dependencies:
  - TASK-2.13
priority: high
ordinal: 130
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while reviewing TASK-2.13 (nom-core/src/food/mod.rs:645, convert_to_per_100g at :57-67). create_custom_food's CreateCustomFoodRequest.serving_size accepts {quantity, unit} where unit can be 'grams', 'cups', 'pieces', etc. (per the task's own Implementation Plan). But execute_json does 'let serving_size_g = req.serving_size.quantity;' and feeds that straight into convert_to_per_100g's '(nutrient_at_serving * 100.0) / serving_size_g' formula, treating the raw quantity number as grams regardless of the actual unit. For a non-gram serving (e.g. quantity=1.0, unit='cups', 100 kcal/serving), this computes 100*100/1 = 10000 kcal per 100g — a 100x-order-of-magnitude-wrong value that gets upserted into the foods table and returned as the food's permanent per-100g snapshot. Every downstream consumer (search_food re-reads the same row, and the not-yet-built TASK-2.14 meal-logging math scales per-100g values by portion size) will silently compute wrong calorie/macro totals for any custom food not specified in grams. This violates the Correctness axis and the doc-5 'all nutrient storage normalized to per-100g invariant' design decision. The existing test (test_create_custom_food_non_gram_unit, food/mod.rs:1152) documents the bug in a code comment ('100 * 100 / 1 = 10000... but that is mathematically correct per the formula') and asserts only that serving_size_g is null in the response — it never checks the resulting calories_per_100g value, so it passes despite the corrupted math.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 create_custom_food either rejects non-gram serving_size.unit values with a clear validation error, or correctly converts the given quantity+unit into a gram-equivalent before calling convert_to_per_100g (document which approach was chosen and why in the task's Implementation Notes)
- [x] #2 a test asserts the actual calories_per_100g/protein_g_per_100g/etc. numeric output for a non-gram serving is correct (or that the request is rejected), not just that serving_size_g is null in the response
- [x] #3 nix develop -c cargo test -p nom-core passes
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Fix Approach: Reject Non-Gram Units (Validation Error)

**Chosen approach:** Reject any `serving_size.unit` that is not a recognized gram unit with `ErrorData::validation`. Volume-to-weight conversion requires ingredient-specific density tables (1 cup flour ~120g vs 1 cup water ~237g), which v1 has no data source for. Industry apps (MyFitnessPal, etc.) do not auto-convert household measures.

### Step 1: Add Unit Validation in execute_json

In `CreateCustomFood::execute_json` (~line 620), immediately after deserializing the request, add validation that rejects non-gram units. Place this right after the existing quantity > 0 check (~line 623):

```rust
// Validate serving size unit — only grams accepted
let unit_lower = req.serving_size.unit.to_lowercase();
if unit_lower != "grams" && unit_lower != "gram" && unit_lower != "g" {
    return Err(ErrorData::validation(
        "serving_size.unit",
        format!(
            "only gram-based units are supported (got '{}'); volume units like cups/pieces cannot be converted without ingredient-specific density data",
            req.serving_size.unit
        ),
    ));
}
```

### Step 2: Simplify effective_serving_size Logic

After validation guarantees only gram units reach the conversion step, the conditional at lines ~648-653 (`if unit == grams...`) becomes redundant. The `serving_size_g` variable now correctly represents grams. Keep the block for documentation clarity, or simplify to `Some(serving_size_g)`.

### Step 3: Fix test_create_custom_food_non_gram_unit

Replace the existing test (`nom-core/src/food/mod.rs:1152`) to assert rejection instead of silently accepting corrupted data. Remove the misleading comment that said "but that is mathematically correct per the formula."

New test asserts:
- Error category is Validation
- Error field is "serving_size.unit"
- Error reason mentions the invalid unit

### Step 4: Add Positive Test for Gram Aliases

Add a test that verifies `"g"` and `"gram"` are accepted (not just `"grams"`). Iterates over all three aliases, confirms each produces correct per-100g conversion and stores `serving_size_g`.

### Step 5: Run Quality Checks

```bash
nix develop -c cargo test -p nom-core
nix develop -c cargo clippy --workspace --all-targets
nix develop -c cargo fmt --check
```

### Files Modified
- `nom-core/src/food/mod.rs` — validation logic + test fixes (~20 lines net change)

### Risk Assessment
- **Breaking change?** Yes — callers using non-gram units will get validation errors instead of silently wrong data. This is the correct behavior; previously the data was corrupted.
- **Downstream impact:** search_food reads from foods table — existing corrupted rows remain but new ones won't be created. No migration needed.
- **Performance:** No impact — adds one string comparison.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Chosen approach: reject non-gram serving_size.unit values with validation error. Volume-to-weight conversion requires ingredient-specific density tables (1 cup flour ~120g vs 1 cup water ~237g), which v1 has no data source for. Added unit validation in execute_json that accepts only 'grams', 'gram', 'g'. Simplified effective_serving_size logic since only gram units pass validation — always stores Some(serving_size_g). Fixed test to assert rejection instead of accepting corrupted data. Added positive test for gram aliases.
<!-- SECTION:NOTES:END -->
