---
id: TASK-9
title: >-
  Fix: create_custom_food mis-converts non-gram serving units to per-100g
  nutrients
status: To Do
assignee: []
created_date: '2026-08-12 05:27'
labels:
  - review-followup
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
- [ ] #1 create_custom_food either rejects non-gram serving_size.unit values with a clear validation error, or correctly converts the given quantity+unit into a gram-equivalent before calling convert_to_per_100g (document which approach was chosen and why in the task's Implementation Notes)
- [ ] #2 a test asserts the actual calories_per_100g/protein_g_per_100g/etc. numeric output for a non-gram serving is correct (or that the request is rejected), not just that serving_size_g is null in the response
- [ ] #3 nix develop -c cargo test -p nom-core passes
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
SETUP (read first): This is a Rust+WebAssembly core (crates/gql-core) with a
TypeScript/React web app (web/). ALL commands must run inside the Nix dev
shell: either run 'direnv allow' once, or prefix every command with
'nix develop -c'. Work from the repository root unless told otherwise. Do not
change pinned dependency versions.

Note: this repo's actual crate layout is nom-core/ and nom-mcp/ (not crates/gql-core — ignore that path in the preamble, it does not apply to this project; everything else in the preamble still applies).

1. Read nom-core/src/food/mod.rs end to end, specifically: ServingSize struct (~line 555), convert_to_per_100g (~line 57), and CreateCustomFood::execute_json (~line 611-684).
2. Decide the fix scope: the simplest correct fix is to reject any serving_size.unit that is not a recognized gram-equivalent unit (grams/gram/g) with ErrorData::validation, since there is no unit-conversion table in scope for this project yet (see doc-5 for whether grams-only was actually intended — if doc-5 explicitly calls for supporting 'cups'/'pieces' as informational-only units with the numeric quantity representing something other than a gram-convertible amount, then instead: only ever pass a gram-based serving_size_g into convert_to_per_100g when unit is grams/gram, and for all other units, store the nutrients as given per-serving without per-100g conversion — i.e. do NOT call convert_to_per_100g at all for non-gram units, and represent that clearly in the stored/returned FoodCandidate, coordinating with how search_food consumes these rows). Pick the rejection approach unless doc-5 clearly requires non-gram units to be accepted and converted.
3. Implement the chosen fix in CreateCustomFoodRequest validation / execute_json.
4. Fix nom-core/src/food/mod.rs:1152 test_create_custom_food_non_gram_unit — either change it to assert the request is rejected (if you chose rejection) or to assert the mathematically correct converted per-100g values (if you chose gram-equivalent conversion). Remove the comment that currently excuses the wrong math.
5. Run: nix develop -c cargo test -p nom-core (and nix develop -c cargo clippy --workspace --all-targets, nix develop -c cargo fmt --check).
<!-- SECTION:PLAN:END -->
