---
id: TASK-67
title: >-
  Fix: legacy meal-type bucket detection duplicated across three functions in
  the weekly-progress widget
status: Done
assignee: []
created_date: '2026-09-07 20:36'
updated_date: '2026-09-07 23:25'
labels:
  - review-followup
dependencies:
  - TASK-64
priority: high
ordinal: 100
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while reviewing TASK-64 (nom-core/assets/weekly_progress_widget.html: mealSegClass ~369, mealBucketLabel ~376, mealLegendHtml's dedup key ~506, all landed in TASK-64). Whether a meal-type bucket counts as 'legacy/unrecognized' is checked three separate times with three slightly different ad-hoc expressions (an object-lookup fallback, a typeof+truthiness check, and a typeof-only check), kept in sync only by a code comment cross-reference rather than one shared predicate. Concise/Organized axis: the same policy knowledge (what counts as legacy) is duplicated instead of living in one place, so a future change to the rule made in one spot can silently desync the ribbon fill class, the legend swatch and the tooltip label from each other.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A single isLegacyMealBucket(mealType) predicate exists in nom-core/assets/weekly_progress_widget.html and is the only thing mealSegClass, mealBucketLabel and mealLegendHtml's dedup key use to decide legacy-ness
- [x] #2 For the four real inputs (breakfast, lunch, dinner, null/absent), rendered output (segment class, tooltip label, legend label and dedup grouping) is unchanged from before the refactor
- [x] #3 A widget guard test in nom-core/src/operation/mcp_handler.rs asserts isLegacyMealBucket( is referenced at all three call sites (mirroring the existing mealLegendHtml(/mealNoneHatchDefs( >= 2 occurrence pattern), so the sharing cannot silently regress
- [x] #4 nix develop -c cargo nextest run --all-features --workspace, cargo clippy --all-targets --all-features --workspace -- -D warnings and cargo fmt --all --check all pass
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
SETUP (read first): This is a Rust+WebAssembly-adjacent core (crates/gql-core convention does not apply here -- this is nom-core/nom-mcp, a plain Rust workspace) with widget assets as static HTML+JS under nom-core/assets/. ALL commands must run inside the Nix dev shell: either run 'direnv allow' once, or prefix every command with 'nix develop -c'. Work from the repository root unless told otherwise. Do not change pinned dependency versions.

1. In nom-core/assets/weekly_progress_widget.html, near MEAL_SEG_CLASS/MEAL_TYPE_ORDER (~line 357), add:
   function isLegacyMealBucket(mealType) {
     return typeof mealType !== "string" || !mealType || !(mealType in MEAL_SEG_CLASS);
   }
   Comment it as the single source of truth for what counts as the legacy/unrecognized bucket -- note explicitly that this also folds any future out-of-enum meal_type string into the legacy bucket (previously mealBucketLabel would have shown such a string as its own capitalized label while mealSegClass painted it seg-none grey; the API only ever emits breakfast/lunch/dinner/null today so this is currently unreachable, but the two should agree if it ever isn't).

2. Update the three call sites to use it:
   - mealSegClass (~369): return isLegacyMealBucket(mealType) ? "seg-none" : MEAL_SEG_CLASS[mealType];
   - mealBucketLabel (~376): if (isLegacyMealBucket(mealType)) return "(none)"; return mealType.charAt(0).toUpperCase() + mealType.slice(1);
   - mealLegendHtml's dedup key (~506): var key = isLegacyMealBucket(b.meal_type) ? "" : b.meal_type;
   Rewrite the comment at ~504-505 (currently 'matching the unrecognised-type fallback in mealSegClass') to say these three now share isLegacyMealBucket instead of agreeing by cross-referenced comment.

3. In nom-core/src/operation/mcp_handler.rs, extend meal_ribbon_ships_a_static_colour_key (or add a new test next to it, same file, ~line 1309) asserting WEEKLY_PROGRESS_WIDGET_HTML.matches("isLegacyMealBucket(").count() >= 4 (one definition + three call sites), following the existing mealLegendHtml(/mealNoneHatchDefs( >= 2 pattern in the same test file.

4. Run: nix develop -c cargo nextest run --all-features --workspace (confirm meal_ribbon_segments_survive_their_seam_stroke, meal_ribbon_ships_a_static_colour_key and meal_ribbon_legacy_bucket_is_not_hue_only all still pass unmodified -- they guard the CSS/pattern side, which this ticket does not touch), cargo clippy --all-targets --all-features --workspace -- -D warnings, cargo fmt --all --check.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented as planned: isLegacyMealBucket(mealType) added beside MEAL_TYPE_ORDER (nom-core/assets/weekly_progress_widget.html), and mealSegClass, mealBucketLabel, and mealLegendHtml's dedup key all now defer to it instead of three separate ad-hoc checks. Rendered output for breakfast/lunch/dinner/null is unchanged by construction (each old check's true/false split is preserved for those four inputs); the only behavioural change is a hypothetical out-of-enum string like "snack", which used to get its own capitalized legend label from mealBucketLabel while mealSegClass painted it seg-none grey -- now both agree it is legacy. That path is unreachable today (migration_v2.sql's CHECK constraint only allows breakfast/lunch/dinner or NULL). Guarded by meal_ribbon_legacy_bucket_check_is_not_duplicated (nom-core/src/operation/mcp_handler.rs), asserting isLegacyMealBucket( appears >= 4 times (1 definition + 3 call sites). Full suite green: nextest 385/385, doctests, clippy -D warnings, fmt --check, rustdoc -D warnings.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Deduplicated legacy/unrecognized meal-type-bucket detection into one isLegacyMealBucket(mealType) predicate shared by the ribbon fill class, the tooltip/legend label, and the legend's dedup key, guarded by a new occurrence-count test so the sharing cannot silently regress.
<!-- SECTION:FINAL_SUMMARY:END -->
