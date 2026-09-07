---
id: TASK-68
title: >-
  Fix: meal-type bucket ordering doc claims one source of truth but the
  weekly-progress widget carries an untested second copy
status: Done
assignee: []
created_date: '2026-09-07 20:36'
updated_date: '2026-09-07 23:25'
labels:
  - review-followup
dependencies:
  - TASK-64
priority: high
ordinal: 110
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while reviewing TASK-64 (nom-core/assets/weekly_progress_widget.html:357, MEAL_TYPE_ORDER/mealTypeRank landed in that ticket). nom-core/src/meal_type.rs:12-17 documents that 'the bucket ordering exist[s] in exactly one place' (bucket_rank, meal_type.rs:144, breakfast=0/lunch=1/dinner=2/else=3), citing TASK-31's cost of duplicated aggregation logic. TASK-64 necessarily added a second, independently-maintained copy of that same ordering in JS (MEAL_TYPE_ORDER = [breakfast, lunch, dinner], weekly_progress_widget.html:357-361) because the client-side widget cannot call into Rust -- a legitimate exception, not a mistake, but the doc comment no longer states the full picture, and nothing in the test suite proves the two orderings actually agree. Clear/Resilient axis: stale documentation plus an untested cross-boundary invariant -- if bucket_rank's order is ever changed server-side, the ribbon and legend would silently render meal types in a different order than the API that feeds them, with no test failing.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 nom-core/src/meal_type.rs's module doc comment (lines 12-17) is updated to name weekly_progress_widget.html's MEAL_TYPE_ORDER as a deliberate second copy of the ordering (JS cannot call bucket_rank), rather than claiming the ordering lives in exactly one place
- [x] #2 A new std-only test in nom-core/src/operation/mcp_handler.rs parses MEAL_TYPE_ORDER out of WEEKLY_PROGRESS_WIDGET_HTML and asserts it equals ["breakfast", "lunch", "dinner"] in that order, matching meal_type::bucket_rank's real ranking (0,1,2, else last)
- [x] #3 The new test is written red first against a deliberately reordered MEAL_TYPE_ORDER and the failure text is quoted in the ticket's implementation notes
- [x] #4 nix develop -c cargo nextest run --all-features --workspace, cargo clippy --all-targets --all-features --workspace -- -D warnings and cargo fmt --all --check all pass
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
SETUP (read first): Plain Rust workspace (nom-core / nom-mcp crates) with widget assets as static HTML+JS under nom-core/assets/. ALL commands must run inside the Nix dev shell: either run 'direnv allow' once, or prefix every command with 'nix develop -c'. Work from the repository root unless told otherwise. Do not change pinned dependency versions.

1. Edit nom-core/src/meal_type.rs lines 12-17: after '...the bucket ordering exist in exactly one place (see TASK-31 ...)', add a sentence naming the one deliberate exception: nom-core/assets/weekly_progress_widget.html's MEAL_TYPE_ORDER mirrors this ranking for client-side rendering because the widget's JS cannot call bucket_rank, and points at the new guard test (step 2) as what keeps the mirror honest.

2. In nom-core/src/operation/mcp_handler.rs, in the ribbon-guard test cluster (near meal_ribbon_ships_a_static_colour_key, ~line 1309), add a small parsing helper in the style of the existing js_number_constant (~1267) -- e.g. js_string_array_constant(source, name) that locates 'var NAME = [' and returns the comma-separated quoted-string contents up to the matching ']' -- then a new test meal_ribbon_legend_order_matches_bucket_rank asserting the parsed MEAL_TYPE_ORDER array equals exactly ["breakfast", "lunch", "dinner"]. Do not hardcode meal_type::bucket_rank's ranks as a second literal in the test beyond this comparison; the point is pinning the JS array against the one true Rust ranking (0/1/2/else-last, meal_type.rs:144-153), so state that correspondence in the test's doc comment.

3. Verify red-first: temporarily reorder MEAL_TYPE_ORDER in the widget asset (e.g. swap lunch and dinner), confirm the new test fails with a clear message naming the mismatch, quote the failure text in this ticket's Implementation Notes, then revert the temporary reorder.

4. Run: nix develop -c cargo nextest run --all-features --workspace, cargo clippy --all-targets --all-features --workspace -- -D warnings, cargo fmt --all --check.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Red-first evidence, temporarily swapping MEAL_TYPE_ORDER to ["breakfast", "dinner", "lunch"] in nom-core/assets/weekly_progress_widget.html and running the new test alone:

```
thread 'operation::mcp_handler::tests::meal_ribbon_legend_order_matches_bucket_rank' panicked at nom-core/src/operation/mcp_handler.rs:1378:9:
assertion `left == right` failed: MEAL_TYPE_ORDER must match meal_type::bucket_rank's ranking (breakfast=0, lunch=1, dinner=2, else last)
  left: ["breakfast", "dinner", "lunch"]
 right: ["breakfast", "lunch", "dinner"]
```

Reverted the swap immediately after capturing this. Implementation: meal_type.rs's module doc comment (lines 12-17) now names weekly_progress_widget.html's MEAL_TYPE_ORDER as the one deliberate client-side mirror of bucket_rank, and points at the new test as what keeps it honest. Added js_string_array_constant (mirrors the existing js_number_constant pattern) and meal_ribbon_legend_order_matches_bucket_rank in nom-core/src/operation/mcp_handler.rs, asserting the parsed MEAL_TYPE_ORDER equals exactly ["breakfast", "lunch", "dinner"]. Full suite green: nextest 385/385, doctests, clippy -D warnings, fmt --check, rustdoc -D warnings.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Closed the untested cross-boundary invariant between meal_type::bucket_rank (server-side ranking) and the widget's MEAL_TYPE_ORDER (client-side mirror): the module doc now names the mirror explicitly instead of claiming a single source of truth, and a new red-first-verified test pins the JS array against the Rust ranking it must agree with.
<!-- SECTION:FINAL_SUMMARY:END -->
