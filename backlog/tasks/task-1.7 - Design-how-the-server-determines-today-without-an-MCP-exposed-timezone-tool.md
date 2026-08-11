---
id: TASK-1.7
title: Design how the server determines 'today' without an MCP-exposed timezone tool
status: Done
assignee:
  - '@Jeffery Utter'
created_date: '2026-08-11 04:39'
updated_date: '2026-08-11 05:41'
labels:
  - 'wayfinder:grilling'
dependencies: []
parent_task_id: TASK-1
ordinal: 8000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Question

The confirmed destination excludes MCP-exposed timezone-setting tools, but date-scoped tools (get_meals_today, goal progress for 'today') still need a notion of the current date. Decide the mechanism: system-local timezone, a config-file/env-var-set timezone read at startup, or something else — and how it's threaded through the CLI/HTTP/MCP surfaces consistently so 'today' means the same day everywhere.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Fresh-per-call computation (not cached at startup) chosen specifically because the server is long-running and a cached 'today' would silently go stale at midnight. Config-with-fallback beat 'system-local only' because deployments to a UTC-default container/NAS would otherwise silently misdate everything, and beat 'config required' because a mandatory setup step is unwarranted friction for a single-user server that usually runs on the user's own machine. Exact config key/format for the tz override is intentionally left to TASK-1.12 (config and secrets handling) rather than decided here.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Timezone resolved once at startup by nom-core: explicit IANA tz name from server config if set (exact key/field format deferred to TASK-1.12), else fallback to host system-local timezone. A single Clock/time-provider owned by nom-core is injected into Operation execution and computes 'today' fresh on every call (never cached), converting current instant using the resolved tz. Since TASK-1.6's Operation registry already drives CLI/HTTP/MCP dispatch, injecting the Clock there makes all three surfaces (plus local CLI, same binary) agree on 'today' by construction; remote-CLI is a thin HTTP client and never computes dates itself. This is also the missing piece for TASK-1.5's logged_date materialization: computed at write time via this same Clock from logged_at (UTC); if the configured tz later changes, historical logged_date values are not retroactively recomputed.
<!-- SECTION:FINAL_SUMMARY:END -->
