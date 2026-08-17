# Claude iOS: MCP Apps widget fails after fully successful server handshake ("Unable to connect to server")

**Client:** Claude iOS 1.260813.0 (session-based client reports UA `python-httpx/0.28.1`; stateless relay channel reports `Claude-User`)
**Server:** self-hosted `nom-mcp` v0.5.4 (Rust, rmcp 3.1.2), endpoint `https://nom-mcp.home.jeffutter.com/mcp`, streamable-HTTP, behind an auth proxy (all requests in scope returned 200; zero 4xx/5xx)
**Spec conformance:** implemented per `modelcontextprotocol/ext-apps` revision `2026-01-26` (`apps.mdx`): `_meta.ui.resourceUri` on tools, `ui://` resources listed in `resources/list` with `mimeType: text/html;profile=mcp-app`, identical `_meta.ui` on read-result contents entries, `initialize` echoes the requested `2026-01-26` revision.

## Symptom

On Claude iOS, asking for a widget-backed answer (e.g. "show me my daily nutrition stats") produces the correct **text** answer plus:

> Failed to load the MCP app. Unable to connect to server

The **identical server responses render correctly on claude.ai web** (repeatedly, including across redeploys). The failure is iOS-only.

## Server-side evidence (journal excerpts, all 200 OK unless noted)

### Case A — fresh session, loader dies between init and fetch (2026-08-17T10:29Z)

```text
10:29:13.477  create new session 9c8ef666-...   initialize: protocolVersion "2026-01-26",
             capabilities.extensions["io.modelcontextprotocol/ui"] = {mimeTypes:["text/html;profile=mcp-app"]}
             → 200 + Mcp-Session-Id, response echoes "2026-01-26"
10:29:25.015  notifications/initialized → 202 Accepted
              … then nothing. Zero further requests from this session, ever.
```

### Case B — stale session restored, read succeeds, still fails (2026-08-17T02:26Z)

```text
02:26:59      initialize replayed from persistent store (session a2669b36 reused across a
             server restart; protocol 2026-01-26, clientInfo claude-ios 1.260813.0)
02:26:59+0ms  notifications/initialized → 202
02:26:59+1ms  resources/read ui://nom-mcp/goal-progress → 200, full self-contained HTML
              (SSE-framed, ok=true, jsonrpc_errors=None)
              … then nothing. App shows the error banner.
```

### Control — web relay, same bytes (2026-08-17T11:09Z)

Stateless `server/discover` → `resources/list` → `tools/list` → `resources/read:ui://nom-mcp/goal-progress`, all 200; widget renders on web.

A local byte-level replication of Case B's exact sequence (initialize as `claude-ios` at `2026-01-26` → kill/restart server → `resources/read` on the stale session id) returns 200 with correct SSE framing and the full widget HTML.

## What we've already ruled out / fixed (each verified in production)

1. **Protocol-version downgrade abort.** Originally rmcp echoed `2025-11-25` for the requested `2026-01-26`; the relay abandoned the session immediately (no follow-ups) and the app showed the same banner. Fixed by echoing `2026-01-26` (v0.5.3). Handshake now completes; the failure persists one layer later.
2. **Session loss across restarts.** Persistent `McpSessionStore`; stale `Mcp-Session-Id`s restore transparently (Case B proves it).
3. **Auth proxy.** Every request in every window returned 200; no app-initiated connections bypassing the proxy; no GET/SSE stream requests from either surface (web success also never uses GET).
4. **CSP/sandbox content issues** (cf. anthropics/claude-ai-mcp#40, hadarge): the widget HTML is fully self-contained — inline CSS/JS only, zero external URLs, `postMessage(..., "*")` bootstrap — so it passes even the restrictive default CSP from `ext-apps` `docs/csp-cors.md`. No `>150k` char offload (payload ≈10 KB). URIs are stable across versions (no stale-URI case).
5. **`_meta.ui.domain` pinning** (cf. #165, Booyaka A/B probe): we added optional config-driven `domain` emission and tested both the origin Claude itself minted for us on web and the computed `sha256(endpoint)[:32]` form. iOS behavior unchanged (dies between init and fetch); web unaffected. Todoist's known-working server does not set `domain` at all. We reverted to field-omitted.

## What remains

After the relay receives our successful `resources/read` (Case B) — or, more often, after merely completing `notifications/initialized` without ever issuing a read (Case A) — the iOS side stops and reports **"Unable to connect to server"** despite having just received 200/202 from that server over that connection. No request of any kind reaches us afterwards.

## Asks

1. **Relay logs for session `9c8ef666-24cd-4994-a772-3a27cda5866f`** (created 2026-08-17T10:29:13Z; `notifications/initialized` sent 10:29:25Z; no subsequent request reached the server) — what did the relay do with the `resources/read` it presumably issued next, or why didn't it?
2. Does the iOS widget loader require anything beyond the `2026-01-26` apps spec (a specific meta field, a GET/SSE stream, a particular sandbox-bootstrap order)?
3. Is there a known-good minimal iOS test server we could diff against?

Cross-references: anthropics/claude-ai-mcp#40 (iOS sandbox CSP bug; staff comment on host-side `_meta` handling), anthropics/claude-ai-mcp#165 (intermittent blank-render; disconnect/reconnect workaround; domain A/B data), modelcontextprotocol/ext-apps#671.
