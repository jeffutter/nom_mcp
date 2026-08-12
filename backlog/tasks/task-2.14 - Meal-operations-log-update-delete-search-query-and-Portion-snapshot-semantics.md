---
id: TASK-2.14
title: >-
  Meal operations (log/update/delete/search/query) and Portion snapshot
  semantics
status: To Do
assignee: []
created_date: '2026-08-11 13:24'
labels: []
dependencies:
  - TASK-2.5
  - TASK-2.7
  - TASK-2.10
  - TASK-2.13
  - TASK-9
type: feature
ordinal: 33000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Scope
log_meal(portions:[{food_id,quantity,quantity_mode}], adjustment?, logged_at?) — two-step flow, food_id must already exist (via search_food/create_custom_food). update_meal(meal_id, portions?, adjustment?, logged_at?) — partial patch; portions when present replaces the whole array (no granular add/remove-portion tools). delete_meal(meal_id) — errors on not-found. search_meals(query, date_range?) — plain keyword search over linked Food names, recency-ordered, not recurring-variation grouping. get_meals_today/by_date/by_date_range.

Portion editing recomputes macros from its own snapshot (captured at creation), never re-fetches current Food catalog data — there is no 'refresh nutrition data' operation. Deleting a Meal cascades to delete its Portions. All deletes are hard deletes.

See doc-5 §5, §13.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 log_meal requires pre-resolved food_id per portion and stores each Portion's nutrient-rate snapshot at insert time
- [ ] #2 update_meal treats a present portions array as a full replacement, not an incremental patch
- [ ] #3 delete_meal errors on a not-found id and cascades to delete the meal's portions
- [ ] #4 editing a portion's quantity recomputes from its own snapshot, not from the current foods table row
- [ ] #5 search_meals matches linked Food names only, recency-ordered, with no recurring-variation grouping
<!-- AC:END -->
