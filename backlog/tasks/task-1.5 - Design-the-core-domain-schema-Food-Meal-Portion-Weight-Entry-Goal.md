---
id: TASK-1.5
title: 'Design the core domain schema: Food, Meal, Portion, Weight Entry, Goal'
status: Done
assignee:
  - '@Jeffery Utter'
created_date: '2026-08-11 04:39'
updated_date: '2026-08-11 05:07'
labels:
  - 'wayfinder:grilling'
dependencies:
  - TASK-1.2
parent_task_id: TASK-1
ordinal: 6000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Question

Turn the domain model in /CONTEXT.md (Food, Meal, Portion, Weight Entry, Goal) into a concrete storage schema for libSQL/Turso. Cover: table shapes and relationships, how Food's `source` discriminator (OpenFoodFacts / USDA FDC / Custom) is represented and what's cached locally from each external source vs re-fetched, how a Meal's mixed composition (zero-or-more Portions plus an optional raw-macro adjustment) is stored, identity/versioning concerns (what happens to a Meal's historical macros if the underlying Food's catalog data is later refreshed), and indexing needed for the date-range queries the tool inventory will need (today/by-date/by-date-range for both Meals and Weight Entries).
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Alternatives weighed: Food as separate tables per source (rejected, three-way join complexity for no current benefit) or JSON metadata blob (rejected, less queryable, nothing needs the extra fields yet). Meal macros as always-live-join to Food (rejected — historical totals would silently drift on catalog refresh) or as fully-versioned Food rows (rejected — more history-tracking machinery than a single-user server needs). Meal adjustment as a pseudo-Portion row (rejected — forces every Portion consumer to handle a null-food case) or a separate adjustments table (rejected — extra join for a single nullable set of columns). Portion snapshot as final computed totals (rejected — quantity edits would have nothing to rescale from, forcing delete+recreate). Goal as a single mutable row (rejected — re-evaluating a past day's progress after a goal change would silently use the new target). Turso concurrency: research subagent found no explicit safety confirmation in docs/issues, but core/io/unix.rs uses F_SETLK POSIX advisory locks with a Drop impl calling unlock_file() — process-scoped, not a stale-PID-lockfile scheme, so a crashed or killed process releases the lock at the kernel level and a clean close releases it explicitly. Sources: github.com/tursodatabase/turso docs/manual.md, core/io/unix.rs, issues #769/#1853/#6813/#2267/#7995, PR #2299. Chose 'require explicit clean close' over 'drop local-CLI path' because the evidence lowers the risk enough that the bigger architectural change (folding local-CLI into an HTTP client) isn't warranted for what's a rare debugging workflow; chose it over 'accept as-is' because the crash-before-checkpoint caveat means the invariant needs to be an explicit, documented contract, not an implicit assumption.
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @Jeffery Utter
created: 2026-08-11 04:56
---
Storage crate is now settled as `turso` (pure Rust, not `libsql`) per TASK-1.2's corrected final summary — this ticket needs to confirm whether strictly sequential (non-overlapping) multi-process file access is actually safe on turso today (its docs list 'No multi-process access' as a current limitation, with real cross-process support only via an experimental, not-production-ready flag). If it isn't confirmed safe, this ticket needs a fallback: either the local-CLI direct-DB path gets dropped in favor of always going through the server/HTTP (folding into TASK-1.6), or the schema/access pattern needs to guarantee the DB handle is always fully closed before another process opens it.
---
<!-- COMMENTS:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Concrete libSQL/Turso schema for the five domain entities (single-user, no user_id anywhere): foods (source enum + nullable external_id, full nutrient cache with no auto-refresh, unique(source,external_id)); meals (logged_at UTC + materialized logged_date for range queries, optional raw-macro adjustment as nullable columns directly on the row); portions (meal_id/food_id FKs, dual quantity_mode grams-or-servings, snapshots the Food's per-100g nutrient rate + serving_size_g at log time so totals are computed on read and are immune to later Food catalog refreshes); weight_entries (logged_at/logged_date pair, bare value in the system-wide configured unit, no per-entry unit column); goals (versioned via effective_from, so past-day progress judges against the goal active that day, not today's). Nutrient set is kcal/protein/carbs/fat/fiber, matching what Goal already tracks per CONTEXT.md. Indexes on logged_date (meals, weight_entries), meal_id (portions), effective_from (goals). Resolves the identity/versioning question: a Meal's historical macros never drift when Food's catalog data is refreshed, because Portion holds its own snapshot, not a live reference. Also resolves the turso multi-process follow-up from TASK-1.2: no explicit doc/issue confirms sequential handoff is safe, but turso's locking (POSIX fcntl advisory locks released on close or process exit via a Drop impl, not a stale-lockfile scheme) strongly supports it; the caveat is WAL data loss on crash-before-checkpoint. Decision: keep the local-CLI direct-DB path (don't fold into TASK-1.6/HTTP-only), but require both local-CLI and server code paths to fully close and checkpoint their connection before handoff, documented as a hard invariant in the access layer.
<!-- SECTION:FINAL_SUMMARY:END -->
