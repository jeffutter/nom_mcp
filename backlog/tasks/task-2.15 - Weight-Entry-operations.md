---
id: TASK-2.15
title: Weight Entry operations
status: To Do
assignee: []
created_date: '2026-08-11 13:24'
labels: []
dependencies:
  - TASK-2.5
  - TASK-2.7
  - TASK-2.10
parent_task_id: TASK-2
type: feature
ordinal: 34000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Scope
log_weight(value, logged_at?), update_weight_entry(id, value?, logged_at?), delete_weight_entry(id) — errors on not-found. get_weight_today/by_date/by_date_range — same per-scope query split and optional-logged_at backdating as Meal. Edits are plain field updates with no cascade concerns. All deletes are hard deletes, no soft-delete/undo.

See doc-5 §5, §13.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 log_weight/update_weight_entry/delete_weight_entry implemented; delete errors on not-found
- [ ] #2 get_weight_today/by_date/by_date_range implemented using the Clock's today() for the 'today' variant
- [ ] #3 delete is a hard delete with no soft-delete flag or undo path
<!-- AC:END -->
