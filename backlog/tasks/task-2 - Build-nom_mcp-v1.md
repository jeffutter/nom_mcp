---
id: TASK-2
title: Build nom_mcp v1
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-11 13:22'
updated_date: '2026-08-13 13:09'
labels: []
dependencies:
  - TASK-2.1
  - TASK-2.2
  - TASK-2.3
  - TASK-2.4
  - TASK-2.5
  - TASK-2.6
  - TASK-2.7
  - TASK-2.8
  - TASK-2.9
  - TASK-2.10
  - TASK-2.11
  - TASK-2.12
  - TASK-2.13
  - TASK-2.14
  - TASK-2.15
  - TASK-2.16
  - TASK-2.17
  - TASK-2.18
documentation:
  - doc-5
type: feature
ordinal: 19000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Goal

Implement nom_mcp v1 per the consolidated implementation spec (doc-5), synthesized from the wayfinder map TASK-1. This is execution, not decision-making — every architectural question doc-5 answers is settled; these subtasks build it.

## Reference

- Spec: doc-5 ("nom_mcp v1 implementation spec")
- Decision log / rationale: TASK-1 and its subtasks TASK-1.1 through TASK-1.17
- Domain vocabulary: /CONTEXT.md
- Architecture reference: jeffutter/notectl (local checkout at /home/jeffutter/src/notectl)

## Sequencing

Subtasks are ordered and dependency-wired to reflect a buildable order: workspace scaffold first, then cross-cutting infra (errors, config, logging, storage), then the Operation/transport core, then per-entity domain operations (Food before Meal, since Meal logging needs resolved food_id), then Goals (needs Meal+Weight data), then the MCP Resource (needs Goals), then testing and CI/CD.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Orchestration Plan

TASK-2 is a pure tracking/epic ticket: its scope (build nom_mcp v1 per doc-5) was fully delegated to 18 ordered, dependency-wired sub-tickets (TASK-2.1 through TASK-2.18), covering:

- Workspace/infra scaffolding: TASK-2.1 (Cargo workspace + nix flake), TASK-2.2 (error taxonomy), TASK-2.3 (config/secrets), TASK-2.4 (observability/logging), TASK-2.5 (storage schema/migrations), TASK-2.6 (CI/CD)
- Operation/transport core: TASK-2.7 (Operation trait + multi-surface registry), TASK-2.10 (Clock/today service), TASK-2.11 (local-CLI direct-DB path), TASK-2.12 (remote-CLI thin binary)
- External API clients: TASK-2.8 (OpenFoodFacts), TASK-2.9 (USDA FDC)
- Domain operations (ordered Food -> Meal -> Weight -> Goals): TASK-2.13 (Food ops), TASK-2.14 (Meal ops), TASK-2.15 (Weight Entry ops), TASK-2.16 (Goal ops + daily progress)
- MCP surface extras: TASK-2.17 (weekly-summary Resource + Widget Display tools)
- Testing: TASK-2.18 (wiremock fixtures, temp-DB integration tests, unit tests)

All 18 sub-tickets are Done (verified via `backlog task list` and per-ticket status check on 2026-08-13). `cargo check --workspace` passes cleanly against the current tree. No unplanned children remain, and no other To-Do ticket references TASK-2 as a parent — the five open TASK-36..40 tickets are independent review-followup fixes, not TASK-2 children.

### Integration/verification already covered by sub-tickets
- End-to-end build/test/lint via TASK-2.6 (CI) and TASK-2.18 (test harness) — both Done.
- Cross-surface (CLI/HTTP/MCP) consistency guaranteed by construction via the TASK-2.7 registry design, exercised by each domain-operation ticket's own tests.

### Remaining work for this ticket
None directly. TASK-2 has no implementation of its own beyond what its sub-tickets delivered — it exists solely to track and sequence them. Every dependency is now Done.

### Status note
This project's backlog config (`backlog/config.yml`) does not define a "Blocked" status — valid statuses are Backlog, To Do, Needs Plan, Dev Ready, In Progress, Done. Since TASK-2 has no direct implementation work, "Dev Ready" is used as the closest available status so a future backlog-execute pass picks this ticket up, re-confirms all 18 dependencies are Done (and the whole-system build/tests still pass), and closes it as Done — mirroring how the directly analogous pure-tracking epic TASK-1 (wayfinder map, all subtasks done) was itself ultimately marked Done.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Verification pass (2026-08-13): re-confirmed all 18 dependency sub-tickets (TASK-2.1 through TASK-2.18) are Status: Done via individual `backlog task <id> --plain` checks. Confirmed no unplanned children reference TASK-2 as a parent, and no other To-Do/Dev-Ready ticket lists TASK-2 as a dependency requiring this ticket to stay open. `cargo check --workspace` and `cargo test --workspace` both pass cleanly (237+ tests, 0 failures) against the current tree, confirming the whole-system build/test bar this tracking epic exists to gate is met. TASK-2 itself has no acceptance criteria and no direct implementation — it is a pure orchestration/tracking ticket per its Implementation Plan, closed out now that every delegated sub-ticket is Done.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
nom_mcp v1 is complete: all 18 delegated sub-tickets (TASK-2.1..TASK-2.18) are Done, cargo check/test pass cleanly across the workspace, and no unplanned follow-on work blocks closure of this tracking epic.
<!-- SECTION:FINAL_SUMMARY:END -->
