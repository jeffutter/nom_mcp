---
id: TASK-2
title: Build nom_mcp v1
status: To Do
assignee: []
created_date: '2026-08-11 13:22'
updated_date: '2026-08-11 13:53'
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
