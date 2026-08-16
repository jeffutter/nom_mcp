---
id: TASK-51
title: Remove temporary MCP debug middleware from serve-http
status: To Do
assignee: []
created_date: '2026-08-16 19:10'
labels: []
dependencies:
  - TASK-50
priority: low
type: chore
ordinal: 57000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
After the user confirms the iOS widget loads successfully against v0.4.6 in production, remove the temporary debug instrumentation added in af30221 (v0.4.4): debug_log_mcp_request middleware plus extract_jsonrpc_methods, extract_jsonrpc_errors, and describe_jsonrpc_message helpers in nom-mcp/src/main.rs, and its .layer(axum::middleware::from_fn(debug_log_mcp_request)) registration. Keep the Mcp-Method/Mcp-Name header extraction only if it proves useful long-term; otherwise drop it all. Verify with fmt/clippy/nextest/docs, then release a patch via cargo release patch --workspace --no-publish --no-confirm --execute. Depends on TASK-50's production verification (deploy v0.4.6, reproduce the iOS widget tap, confirm resources/read returns 200 in the log).
<!-- SECTION:DESCRIPTION:END -->
