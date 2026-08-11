---
id: TASK-2.11
title: Local-CLI direct-DB path and lock probe
status: To Do
assignee: []
created_date: '2026-08-11 13:24'
labels: []
dependencies:
  - TASK-2.5
  - TASK-2.7
parent_task_id: TASK-2
type: feature
ordinal: 30000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Scope
The main binary's local CLI always executes Operations in-process against the local Turso file, first-class and top-level alongside 'serve' — not a runtime decision, no remote fallback (that's nom-mcp-remote's job). Given the clean-close/checkpoint invariant from the storage schema design, local-CLI adds a runtime lock probe (same POSIX advisory lock turso already takes) before opening the DB directly, failing fast rather than risking silent WAL corruption if the server appears to hold it. The lock-probe rejection is an ordinary Conflict error with reason 'local_db_locked', rendered with a CLI-specific message ('server is running — stop it or use the remote-CLI instead').

See doc-5 §2 and §3.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 local CLI subcommands execute in-process against the local DB file with no network path
- [ ] #2 opening the DB directly first probes the advisory lock and fails fast with a Conflict/local_db_locked error if held
- [ ] #3 the CLI-specific message directs the user to stop the server or use nom-mcp-remote
<!-- AC:END -->
