---
id: TASK-62.4
title: Document meal_type in README and add the Meal Type term to CONTEXT.md
status: Done
assignee:
  - '@ralph'
created_date: '2026-09-07 01:30'
updated_date: '2026-09-07 02:51'
labels:
  - task
  - planned
dependencies:
  - TASK-62.2
  - TASK-62.3
documentation:
  - CONTEXT.md
parent_task_id: TASK-62
priority: medium
type: docs
ordinal: 73000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Documentation slice of TASK-62 (AC #9): document the shipped meal_type behaviour in README and add the Meal Type term to the CONTEXT.md domain glossary. Prose only — no code changes. Depends on TASK-62.2 (write path) and TASK-62.3 (summary output shape), because the wording describes both.

What has to be written down, beyond "there is a new argument":
- The exact default windows and that they are read in the Clock's timezone, plus that editing a meal's logged_at re-derives the label unless explicitly overridden.
- That the taxonomy has NO snack value: whatever window the clock time falls in wins, so a 15:30 protein bar is a lunch. This is a deliberate decision (research doc-6 §1-2: no accepted time-of-day definition exists; Android Health Connect and LogMeal both need a fourth SNACK/UNKNOWN bucket) and the glossary must say it plainly, since CONTEXT.md defines Meal as including "a snack".
- That meals logged before the field existed carry no meal type and report null, and that nothing is backfilled.
- Naming care: Direction already lists bare "Type" under _Avoid_, so the compound term stays qualified as "Meal Type".
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 CONTEXT.md gains a Meal Type entry in house format covering the default windows, the explicit override, the re-derivation on time edits, the deliberate absence of a snack value, and the legacy null case
- [x] #2 Meal entry mentions that a Meal carries a Meal Type; Direction's _Avoid_ list is not contradicted (term stays qualified as "Meal Type")
- [x] #3 README documents the optional argument for log_meal and update_meal with an example, plus the by_meal_type breakdown in the weekly summary section and meal_type in the REST/remote-CLI examples
- [x] #4 Every documented field name and error shape was verified against real command output, not assumed
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Scope

`CONTEXT.md` + `README.md` only. Verify claims against the running binary rather than trusting this plan's wording.

## Step 1 — `CONTEXT.md`: add **Meal Type** immediately after **Meal**, matching house format exactly (bold `**Term**:` line, indented prose paragraph, `_Avoid_:` line; see the Meal/Direction entries at L7-15).

Content to state:

- Which of breakfast, lunch or dinner a Meal belongs to. Stored on the Meal at write time, never inferred at query time.
- Default: derived from the Meal's logged instant read in the Clock's timezone — breakfast 05:00–10:59, lunch 11:00–15:59, dinner 16:00–04:59 (wraps midnight, so a post-midnight Meal extends the previous evening's dinner while its logged_date still belongs to the new calendar day). An explicit argument always wins; correcting a Meal's logged time re-derives the label unless it was given explicitly.
- No Snack value exists: any eating occasion falls into whichever window its clock time lands in, so a 15:30 protein bar is a lunch. State that the windows are a convenience label rather than ground truth.
- Meals logged before the attribute existed have no meal type and report `null`; nothing is backfilled, because deriving one now would claim a choice the user never made.
- `_Avoid_: Meal Slot, Occasion, Category, Snack (no such value)` — and keep the term always qualified as "Meal Type", since Direction's `_Avoid_` already rejects bare "Type".

Also extend the existing **Meal** entry's first sentence to say a Meal carries a Meal Type, and leave Fasting Window untouched (it reads timestamps, not meal types).

## Step 2 — `README.md`

- Operations table (L46-82): note the optional `--meal_type <breakfast|lunch|dinner>` on the `log_meal` row (L52) and the `update_meal` row (L53).
- Extend the CLI example (L72) with `--meal_type lunch`.
- Weekly-summary section (L114-116): mention that each `daily_totals` entry now carries a `by_meal_type` breakdown, with legacy rows grouped under `null`.
- REST API (L118-120) and `nom-mcp-remote` (L122-134) sections: show `meal_type` as a JSON body key and as a `key=value` pair respectively, so all four surfaces are visibly documented.
- Keep the arg-format paragraph (L44) truthful about how an invalid value is reported (Validation error naming the `meal_type` field, non-zero exit code).

## Step 3 — verify before writing

```sh
nix develop .#ci -c cargo run -p nom-mcp --bin nom-mcp -- log_meal --portions '[{"food_id":1,"quantity":100.0,"quantity_mode":"grams"}]' --meal_type lunch
nix develop .#ci -c cargo run -p nom-mcp --bin nom-mcp -- get_meals_by_date_range --start_date 2026-01-01 --end_date 2030-01-01
nix develop .#ci -c cargo run -p nom-mcp --bin nom-mcp -- log_meal --portions '[{"food_id":1,"quantity":100.0,"quantity_mode":"grams"}]' --meal_type snack   # expect Validation naming meal_type
```

Quote real output shapes in the docs; do not invent field names.

## Done when

A reader can learn the argument, the default windows, the no-snack decision, and the legacy-null behaviour from README alone, and the glossary term matches what shipped.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
CONTEXT.md: new **Meal Type** entry directly after **Meal**, in house format (bold term, indented prose, _Avoid_ line), covering write-time storage, the three windows read in the Clock's timezone (midnight-wrapping), explicit override, re-derivation when logged_at is edited, the deliberate no-snack decision, and the legacy null case; Meal's first sentence now says a Meal carries exactly one Meal Type; Weekly Summary entry notes the per-day split; _Avoid_ keeps the term qualified (bare "Type" stays Direction's). README: operations table rows for log_meal/update_meal/get_meals_by_date_range/get_goal_progress, a new '### Meal type' section (defaults, override + real Validation output, re-derivation, no snack, legacy null, and a real weekly-summary daily_totals object showing by_meal_type), REST section shows meal_type as a body key plus the real 400 error JSON, remote-CLI block adds a meal_type=breakfast example, domain-model summary points at the new section.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Prose only. Every documented name, value list, and error shape was taken from live commands against a seeded throwaway DB (NOM_MCP_DB_PATH=/tmp/nom62/db.sqlite): local CLI log_meal --meal_type lunch and the invalid-value stderr/exit-4 pair, POST /api/log_meal with meal_type plus its HTTP 400 Validation JSON, nom-mcp-remote ... meal_type=breakfast and its invalid case, get_meals_by_date_range showing meal_type per meal with null for a raw-SQL legacy row, get_goal_progress meals_by_type, update_meal re-deriving the label from a corrected logged_at, and the nom://weekly-summary resource JSON whose day entry is quoted verbatim (floats aside).
<!-- SECTION:FINAL_SUMMARY:END -->
