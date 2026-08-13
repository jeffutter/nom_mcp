---
id: TASK-24
title: >-
  Fix: test_snapshot_semantics_untouched_meal_unaffected_by_catalog_change fails
  because it mixes Clock{UTC} with chrono::Local::now()
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-13 00:28'
updated_date: '2026-08-13 00:55'
labels:
  - review-followup
dependencies:
  - TASK-14
priority: high
ordinal: 195
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Surfaced while reviewing TASK-11: its AC #4 claims 'nix develop -c cargo test -p nom-core passes' and is checked [x], but running that exact command shows 1 failure: meal::tests::test_snapshot_semantics_untouched_meal_unaffected_by_catalog_change panics with "Should have at least one meal for today". TASK-11's Implementation Notes do honestly disclose this as a pre-existing, unrelated failure — verified during review: it fails identically at commits well before TASK-11/19/20/2.12 (e.g. at d5fff96). Root cause traces to TASK-14 (nom-core/src/meal/mod.rs:2000-2060), which added this test: it logs a meal via LogMeal::new(Clock { tz: chrono_tz::UTC }) (~line 2010), so the meal's logged_date is computed in UTC, but then re-queries "today" via chrono::Local::now().format("%Y-%m-%d") (~line 2048) — the system's LOCAL timezone, not UTC. Whenever the test machine's local calendar date differs from the UTC calendar date at run time (any negative-UTC-offset timezone in the evening, or positive-offset timezone in the early morning), the query window misses the just-logged meal and the assertion fails — not because the snapshot-freezing invariant the test exists to prove is violated, but because of a timezone mismatch inside the test's own setup. Resilience-axis finding per CLAUDE.md (determinism, no reliance on real wall-clock/local-timezone state); it currently makes cargo test -p nom-core unreliable/red for everyone, risking masking a real regression from a future ticket.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 test_snapshot_semantics_untouched_meal_unaffected_by_catalog_change (nom-core/src/meal/mod.rs) computes the "today" string it queries GetMealsByDateRange with using the SAME UTC clock it used to log the meal (the test's existing clock variable's .today() method), not chrono::Local::now()
- [x] #2 The test passes deterministically regardless of the machine's local timezone (verify by re-running with TZ=Pacific/Kiritimati and TZ=Etc/GMT+12 in addition to the default environment)
- [x] #3 nix develop -c cargo test -p nom-core passes with zero failures — only check this AC once verified fully green
- [x] #4 nix develop -c cargo clippy --workspace --all-targets is clean
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
SETUP (read first): This is a Rust+WebAssembly core (crates/gql-core) with a
TypeScript/React web app (web/). ALL commands must run inside the Nix dev
shell: either run 'direnv allow' once, or prefix every command with
'nix develop -c'. Work from the repository root unless told otherwise. Do not
change pinned dependency versions.

Note: this repo's actual crate layout is nom-core/ and nom-mcp/ (not crates/gql-core — ignore that path in the preamble; everything else in the preamble still applies).

1. Open nom-core/src/meal/mod.rs and locate test_snapshot_semantics_untouched_meal_unaffected_by_catalog_change (~line 2000). Read it in full, noting the 'let clock = Clock { tz: chrono_tz::UTC };' construction at ~line 2010 and its use to build LogMeal::new(clock).
2. At ~line 2048, replace:
     let today = chrono::Local::now().format("%Y-%m-%d").to_string();
   with:
     let today = clock.today().format("%Y-%m-%d").to_string();
   (Clock::today() in nom-core/src/clock.rs returns a chrono::NaiveDate, which supports .format("%Y-%m-%d") directly — no extra conversion needed. This reuses the same clock the test already constructed to log the meal, so 'today' is computed identically on both the write and read sides.)
3. Run: nix develop -c cargo test -p nom-core test_snapshot_semantics_untouched_meal_unaffected_by_catalog_change -- confirm pass in the default environment.
4. Run: TZ='Pacific/Kiritimati' nix develop -c cargo test -p nom-core test_snapshot_semantics_untouched_meal_unaffected_by_catalog_change -- confirm pass (UTC+14, tests the 'local date is ahead of UTC date' edge).
5. Run: TZ='Etc/GMT+12' nix develop -c cargo test -p nom-core test_snapshot_semantics_untouched_meal_unaffected_by_catalog_change -- confirm pass (UTC-12, tests the 'local date is behind UTC date' edge).
6. Run: nix develop -c cargo test -p nom-core -- confirm the FULL suite is green with zero failures (154/154). Do not check off AC #3 unless this is fully clean.
7. Run: nix develop -c cargo clippy --workspace --all-targets -- confirm clean.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Fixed timezone mismatch in test_snapshot_semantics_untouched_meal_unaffected_by_catalog_change: replaced chrono::Local::now() with clock.today() so the query uses the same UTC date as the meal logging. Verified with TZ=Pacific/Kiritimati (UTC+14) and TZ=Etc/GMT+12 (UTC-12). Full nom-core suite passes (163 tests), clippy clean.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Fixed timezone mismatch in test_snapshot_semantics_untouched_meal_unaffected_by_catalog_change by replacing chrono::Local::now() with clock.today(). One-line change in nom-core/src/meal/mod.rs. Verified deterministically across three timezones (default, UTC+14, UTC-12). Full nom-core suite passes (163 tests), clippy clean.
<!-- SECTION:FINAL_SUMMARY:END -->
