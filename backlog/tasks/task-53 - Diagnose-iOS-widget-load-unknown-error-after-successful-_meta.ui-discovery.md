---
id: TASK-53
title: Diagnose iOS widget-load "unknown error" after successful _meta.ui discovery
status: In Progress
assignee: []
created_date: '2026-08-16 23:37'
updated_date: '2026-08-17 01:39'
labels: []
dependencies: []
priority: high
type: bug
ordinal: 59000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
After v0.5.1 (gating removed), a force-quit cold start + new conversation on Claude iOS ("can you show me my daily nutrition stats") produced: text answer correct (stateless tools/call get_goal_progress → 200 via backend relay) AND "Failed to load the MCP app, an unknown error has occured". Progress vs. prior failures: the app now ATTEMPTS the widget (proves it saw _meta.ui in tools/list). But no resources/ui read appears in the captured nom-mcp log window (23:29:51-52, ~1s, ends at the tool call). Two hypotheses: (a) log window truncated before the widget load (host fetches ui:// resource seconds later); (b) the widget loader is a separate app-initiated connection that fails at the auth proxy (WKWebView without Authelia cookie → 401) and never reaches nom-mcp. Distinguish via: full nom-mcp journal from app-open through >1min after the error (look for resources/read:ui://nom-mcp/goal-progress + status), and auth-proxy logs for the same window (look for 401/403). If (a) and resources/read=200: failure is inside the WebView (ui/initialize handshake / CSP / sandbox) — next layer is host-side. If (b): fix is proxy auth passthrough for the widget-loader connection.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
2026-08-16 diagnosis COMPLETE (full proxy log + journalctl windows 23:29:10–23:31:00): (1) App launch 23:29:14–15: stateless server/discover + resources/list + tools/list all 200, ui_meta_tool_count=2 — app received widget metadata. (2) Prompt: stateless tools/call:get_goal_progress 200 at 23:29:52. (3) ONLY subsequent request: session-based ping on 5a42d5e2 (2025-11-25) at 23:30:35 → 200 — iOS session healthy after the failure. (4) ZERO resources/read requests from any channel/stateless or session. (5) Proxy: all requests 200, no auth failures — proxy exonerated. MYSTERY SOLVED: UA 'python-httpx/0.28.1' in proxy logs IS the iOS app's session-based MCP client (same session id + protocol as 22:24 log); the 23:30:35 request was a routine ping. CONCLUSION: Claude iOS 1.260813.0 fails locally between receiving the tool result and issuing resources/read — client-side renderer bug/limitation. All server/proxy layers verified healthy; web renders widgets off identical responses. No further server-side fix possible. Options: report to Anthropic; optionally re-gate _meta.ui for claude-ios so iOS stops attempting (text-only fallback, as pre-v0.5.1) until Anthropic ships a fixed renderer.

2026-08-17 00:15-00:31 UPDATE — regression went cross-surface: after iOS 'Failed to Load' (~00:19) web ALSO stopped rendering widgets ('Unable to reach Nom' banner, then text-only answers). Log evidence: every request reaching nom-mcp AND the proxy returned 200 (zero panics, server up since 22:06); yet client traffic collapsed — 5 user re-runs produced ~no MCP traffic; even a BRAND-NEW web conversation (00:28:34 discover + tools/call:get_weekly_progress) carried NO tools/list and NO resources/read — the Anthropic backend relay drives all widget behavior from its own cached catalog/UI state and never re-polls us (our 5-min ttlMs hints not honored on the stateless inline-lifecycle channel). Web had previously rendered widgets WITHOUT ever sending us a resources/read (served HTML from its own cache). Suppression confirmed per-SERVER (new conversation also text-only). Conclusion: Anthropic-side per-server degraded state / circuit breaker, plausibly triggered by the reported iOS load failure; nothing we deploy can reach that state. Mitigation being tested: toggle the MCP server OFF->ON in Claude settings to force full re-handshake + fresh catalog/resource prefetch. Also implemented (pending release): ui:// resources now listed in resources/list (spec permits omission; some hosts apparently cross-check tool resourceUri against the listing before fetching). NOTE: python-httpx/0.28.1 UA in proxy logs = iOS app's session-based MCP client (routine pings); Claude-User UA = stateless relay channel.

2026-08-17 01:29 DEPLOYED v0.5.2 (tag fb6e51a): ui:// resources now listed in resources/list. CD pipeline ~15min total (build+publish+notify, then homelab-side deploy workflow ~5min). Live at 01:29:13 UTC (new store path lfvs6qmn...); direct production resources/list probe confirms 3 entries incl. both ui:// URIs with text/html;profile=mcp-app. Service restart transparent to existing sessions (McpSessionStore restore working — iOS session pings continued across restart). NEXT: user runs OFF->ON toggle test in Claude settings (web) against v0.5.2 to force fresh relay handshake; watch journal for tools/list burst (ui_meta_tool_count=2) + any resources/read, and whether widgets render.

2026-08-17 01:34-01:40 RESOLVED (web) — toggle test PASSED: user toggled nom-mcp OFF->ON at ~01:34 UTC; journal shows the full fresh-handshake chain, all 200 OK: stateless server/discover x6 + tools/list x3 (ALL ui_meta_tool_count=2) + resources/list x2 (now 3 entries incl. ui:// URIs) -> tools/call:get_weekly_progress -> resources/read:ui://nom-mcp/weekly-progress x3 (once per page refresh). FIRST PRODUCTION resources/read FOR A WIDGET EVER PREVIOUSLY THE RELAY RENDERED FROM ITS OWN CACHE WITHOUT FETCHING. Widget rendered on first request AND survived repeated page refreshes. CONFIRMED: the cross-surface text-only regression was Anthropic-side per-server degraded state; the OFF->ON toggle forces a full re-handshake that clears it. Mitigation documented: if widgets stop rendering with all server responses healthy (200s, no panics, collapsed client traffic), toggle the MCP server off/on in Claude settings. REMAINING: iOS 'Failed to Load' (renderer fails before issuing resources/read) is untested against v0.5.2 — the new resources/list content may or may not matter to it; next step is an iOS cold-start test, then decide report-to-Anthropic vs re-gate claude-ios.
<!-- SECTION:NOTES:END -->
