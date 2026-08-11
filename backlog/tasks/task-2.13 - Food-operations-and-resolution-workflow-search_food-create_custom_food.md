---
id: TASK-2.13
title: 'Food operations and resolution workflow (search_food, create_custom_food)'
status: To Do
assignee: []
created_date: '2026-08-11 13:24'
labels: []
dependencies:
  - TASK-2.5
  - TASK-2.7
  - TASK-2.8
  - TASK-2.9
type: feature
ordinal: 32000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Scope
search_food(query): barcode-shaped (all-digit) queries route to OpenFoodFacts only; everything else routes to local Custom Foods (case-insensitive substring match, searched first) + USDA FDC, merged into one list capped at 5 combined candidates. Every candidate is upserted into the local foods cache as part of the call (full nutrient snapshot, no auto-refresh) — searching IS resolving, food_id is immediately usable. Each candidate carries food_id, name, source, and its full cached nutrient snapshot (no separate get_food_details tool).

create_custom_food(name, serving_size:{quantity,unit}, nutrients): nutrients given per one serving. No server-side dedup — reuse relies entirely on search_food's substring match; tool descriptions must instruct 'search before creating'.

Fallback workflow for tool descriptions (LLM-orchestrated, not server logic): barcode miss falls through to a free-text search_food retry; free-text/dish miss triggers per-ingredient decomposition with LLM-judgment collapse to a whole-dish Custom Food when mostly uncatalogued; barcode/label photos are transcribed/extracted by the LLM itself, not a tool.

Edit/delete: no delete_food operation exists in v1 — Foods are never hard-deleted.

See doc-5 §5, §6, §13.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 search_food routes barcode-shaped queries to OFF only and free-text queries to Custom+USDA merged/capped at 5
- [ ] #2 every returned candidate is upserted into the foods table as part of the search_food call
- [ ] #3 create_custom_food stores nutrients on a per-serving basis
- [ ] #4 no delete_food Operation exists
<!-- AC:END -->
