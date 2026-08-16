---
id: TASK-52
title: Remove Claude iOS _meta.ui exclusion from tools/list gating
status: In Progress
assignee: []
created_date: '2026-08-16 21:39'
labels: []
dependencies: []
priority: high
type: bug
ordinal: 58000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
81d32ff hid _meta.ui from clients identifying as claude-ios based on a packet-capture assumption that iOS sends MCP-Protocol-Version: 2026-01-26 (an unknown value rmcp hard-rejects with 400) on MCP-Apps tool calls. Production evidence since v0.4.6 contradicts the premise: during a live iOS widget-load attempt (2026-08-16 21:18 UTC), every request carried MCP-Protocol-Version: 2025-11-25 (known) and zero 400s occurred; instead the flow died silently right after tools/list returned ui_meta_tool_count=0 — the app re-validates cached old-conversation widget references against tools/list and gives up when no tool advertises _meta.ui. Local repro matrix confirms rmcp accepts tools/call + resources/read with header 2025-11-25 (200, full data) and rejects only the unknown 2026-01-26 header value (400). Remove the CLAUDE_IOS_CLIENT_NAME exclusion so iOS sees _meta.ui again and can load widgets; if iOS ever does send the bad header, the still-deployed debug middleware will show clean 400 evidence and the workaround can be revisited with real data. Revert: constant, build_tools_gated param + condition, list_tools context/cache-scope, affected tests/doc comments.
<!-- SECTION:DESCRIPTION:END -->
