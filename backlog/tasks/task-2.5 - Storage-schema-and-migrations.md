---
id: TASK-2.5
title: Storage schema and migrations
status: To Do
assignee: []
created_date: '2026-08-11 13:23'
labels: []
dependencies:
  - TASK-2.1
type: feature
ordinal: 24000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Scope
Turso/libSQL schema for the five domain tables plus settings, single-user (no user_id anywhere):
- foods (source enum + nullable external_id, unique(source, external_id), full nutrient cache)
- meals (logged_at UTC + materialized logged_date, optional raw-macro adjustment as nullable columns)
- portions (meal_id/food_id FKs, quantity_mode grams-or-servings, snapshots Food's per-100g nutrient rate + serving_size_g at log time)
- weight_entries (logged_at/logged_date pair, bare value in configured unit)
- goals (effective_from-versioned, direction column per nutrient target)
- settings (single row, widget_display_enabled BOOLEAN)

Indexes: logged_date (meals, weight_entries), meal_id (portions), effective_from (goals). No shipped migration tooling — raw SQL migrations, BYO.

See doc-5 §2.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 all six tables created via raw SQL migration(s) with the columns/constraints listed above
- [ ] #2 indexes exist on logged_date (meals, weight_entries), meal_id (portions), effective_from (goals)
- [ ] #3 a Portion row's snapshot columns are populated at insert time and never updated by a later Food catalog change
<!-- AC:END -->
