---
id: TASK-1.16
title: 'Design edit/delete semantics for Meals, Portions, Foods, and Weight Entries'
status: Done
assignee:
  - '@Jeffery Utter'
created_date: '2026-08-11 13:11'
updated_date: '2026-08-11 13:11'
labels:
  - 'wayfinder:grilling'
dependencies: []
parent_task_id: TASK-1
ordinal: 17000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Question

TASK-1.5's schema snapshots a Food's nutrient rate onto each Portion at log time. What happens when a Portion is edited after its Food's catalog data has since changed? What are the deletion/cascade semantics for Meal, Portion, Food, Weight Entry, and Goal?
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Rationale: the snapshot design (TASK-1.5, decision-1-adjacent) already exists specifically so catalog drift can't corrupt historical logs — extending that same principle to edits (recompute from the snapshot, not the catalog) keeps the model consistent rather than introducing a second data-freshness rule. No delete_food avoids an entire class of dangling-reference bugs for near-zero cost, since Custom/OFF/USDA Foods are cheap to leave unused.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Portions are immutable snapshots (TASK-1.5): editing a Portion's amount recomputes its macros from the nutrient rate captured at creation time, never re-fetches current catalog data — there is no 'refresh nutrition data' operation in v1. Deleting a Meal cascades to delete its Portions (they have no existence independent of their Meal). Foods are never hard-deleted — no delete_food operation exists — since Portions already snapshot their own data, keeping unused catalog/Custom Foods around is harmless; hiding/archiving a Custom Food from search is deferred (not needed for v1). Weight Entry and Goal edits are plain field updates with no cascade concerns, since nothing else references them. All deletes (Meal, Weight Entry) are hard deletes with no soft-delete/undo — single-user tool, no audit-trail requirement.
<!-- SECTION:FINAL_SUMMARY:END -->
