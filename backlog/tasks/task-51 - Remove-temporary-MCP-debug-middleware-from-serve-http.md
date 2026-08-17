---
id: TASK-51
title: >-
  Keep MCP debug middleware; strip iOS-investigation-specific framing from its
  comments
status: Done
assignee: []
created_date: '2026-08-16 19:10'
updated_date: '2026-08-17 18:22'
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

## Comments

<!-- COMMENTS:BEGIN -->
created: 2026-08-17 18:22
---
RESCOPE (2026-08-18): user decided NOT to remove the middleware — 'The extra debugging might be useful elsewhere and it's easy enough to turn off.' Only the issue-specific parts went: the investigation-era doc-comment framing (TEMPORARY / since 81d32ff / Claude-iOS example / Remove-once-done) was replaced with neutral permanent wording documenting the 413/502 behavior notes. Committed in 9e96d27.
---
<!-- COMMENTS:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Rescoped per user decision (2026-08-18): the serve-http /mcp access-log middleware (debug_log_mcp_request + JSON-RPC extraction helpers, added in af30221) is KEPT as general-purpose MCP debugging rather than removed — the extra request/response/method/peer/transport-header visibility is useful for future issues and costs nothing at the default info log level (all output is tracing::debug!/warn!). What WAS done: stripped the iOS-investigation-specific framing from its doc comment — dropped 'TEMPORARY ... investigating widget gating since 81d32ff', the Claude-iOS-specific bogus-protocol-version example, and 'Remove once the investigation is done' — replacing it with neutral permanent wording that documents what it logs plus its two non-standard behaviors (413 on >1 MiB request bodies, 502 on unbufferable POST responses). The three McpHandler debug logs were likewise reworded to permanent observability (see commit 9e96d27). Verified: cargo fmt, clippy -D warnings, 311/311 nextest, doctests, rustdoc -D warnings.
<!-- SECTION:FINAL_SUMMARY:END -->
