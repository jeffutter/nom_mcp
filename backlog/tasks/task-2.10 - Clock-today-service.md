---
id: TASK-2.10
title: Clock / today service
status: To Do
assignee: []
created_date: '2026-08-11 13:23'
labels: []
dependencies:
  - TASK-2.3
  - TASK-2.7
type: feature
ordinal: 29000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Scope
Timezone resolved once at startup: explicit IANA tz from config if set, else host system-local. A single Clock owned by nom-core is injected into Operation execution and computes 'today' fresh on every call (never cached). Since the Operation registry (TASK-2.7) drives CLI/HTTP/MCP dispatch, injecting the Clock there makes all three surfaces agree on 'today' by construction. Also used at write time to materialize meals.logged_date / weight_entries.logged_date from logged_at (UTC).

See doc-5 §4.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Clock resolves tz from config, falling back to system-local when unset
- [ ] #2 today() is computed fresh per call, not cached at startup, and is injected into every Operation execution path (CLI, HTTP, MCP)
- [ ] #3 logged_date materialization at write time uses this Clock
<!-- AC:END -->
