---
id: decision-2
title: Goal nutrient targets carry an explicit Direction
date: '2026-08-11 12:13'
status: accepted
---
## Context

CONTEXT.md's Goal was ambiguous between a nutrient target being an exact aim, a floor, or a ceiling ("target or limit"). TASK-1.10 needed to settle what get_goal_progress actually computes and returns per nutrient.

## Decision

Each Goal nutrient target (calories, protein, carbs, fat, fiber) carries an explicit Direction — target / minimum / maximum — set by the caller via set_nutrition_goals. Direction is required the first time a nutrient is targeted (no default, since a bare number alone doesn't say whether hitting it or staying under it is the win); a later partial update that omits Direction carries forward the previously-set value, matching the existing partial-patch merge semantics for the value itself. target_weight is exempt — no Direction field, since comparing latest Weight Entry to target_weight is unambiguous without one.

Considered and rejected:
- Uniform target framing (remaining = target − consumed, no semantic meaning attached) — pushes over/under interpretation onto every caller with no server-side help.
- Fixed per-nutrient semantics (calories/fat/carbs always ceilings, protein/fiber always floors) — doesn't fit a user cutting on protein or bulking on calories.

## Consequences

- Extends TASK-1.5's goals schema with a direction column per nutrient, versioned the same way via effective_from.
- Extends TASK-1.8's set_nutrition_goals signature: each nutrient value's first-time-set call must also include its direction.
- get_goal_progress derives a per-nutrient status (under/met/over, met via exact equality only) using Direction; the weight section uses the same status scheme without a Direction field.

