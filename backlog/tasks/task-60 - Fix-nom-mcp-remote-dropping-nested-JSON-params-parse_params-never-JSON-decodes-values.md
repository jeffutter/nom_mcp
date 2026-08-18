---
id: TASK-60
title: >-
  Fix nom-mcp-remote dropping nested JSON params (parse_params never
  JSON-decodes values)
status: To Do
assignee: []
created_date: '2026-08-18 02:29'
labels:
  - bug
dependencies: []
priority: high
ordinal: 66000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
nom-mcp-remote cannot invoke any operation with a nested-JSON argument (log_meal/update_meal portions, create_custom_food serving_size/nutrients). Root cause: nom-core/src/cli.rs::parse_params -> parse_value only auto-types bare i64/f64/true/false and leaves everything else as a string; the value is POSTed to /api/{op} as a JSON *string*, and the operation's request deserialization fails (verified empirically: 'nom-mcp-remote log_meal portions=[{...}]' -> 400 validation 'invalid type: string "[...]", expected a sequence'). The local CLI does not have this problem because cli_router.rs::parse_value tries serde_json::from_str first, so inline JSON works there. Fix direction: make cli.rs::parse_value try serde_json::from_str before falling back to plain-string inference (parity with cli_router::parse_value), keeping the existing bare-number/bool behavior as a subset of that; add unit tests covering array/object/null passthrough plus the existing cases, and a binary-spawning e2e test (seed_data throwaway DB + serve http + nom-mcp-remote log_meal) mirroring nom-mcp/tests/seed_e2e.rs. Also remove the interim README note added by TASK-59 stating the remote CLI cannot pass nested JSON once this lands.
<!-- SECTION:DESCRIPTION:END -->
