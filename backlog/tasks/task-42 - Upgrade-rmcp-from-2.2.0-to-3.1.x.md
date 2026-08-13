---
id: TASK-42
title: Upgrade rmcp from 2.2.0 to 3.1.x
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-13 17:34'
updated_date: '2026-08-13 18:05'
labels:
  - planned
dependencies: []
references:
  - 'https://github.com/modelcontextprotocol/rust-sdk/discussions/969'
  - >-
    https://github.com/modelcontextprotocol/rust-sdk/compare/rmcp-v2.2.0...rmcp-v3.0.0
  - 'https://github.com/modelcontextprotocol/rust-sdk/releases/tag/rmcp-v3.0.0'
  - TASK-41
priority: medium
type: chore
ordinal: 47000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
nom-core and nom-mcp pin `rmcp = "2.2.0"` (released 2026-07-08). The latest release is 3.1.2 (2026-08-07) — a full major version ahead, via a breaking 3.0.0 release plus three more releases on top.

The 3.0.0 breaking changes ("RMCP 3.0 adds support for MCP 2026-07-28") include several that land directly on code this project already has:

- `ServerHandler::call_tool`, `get_prompt`, and `read_resource` now return MRTR-aware (Multi Round-Trip Requests) response enums instead of plain `Result<CallToolResult, ErrorData>` / `Result<ReadResourceResult, ErrorData>`. `nom-core/src/operation/mcp_handler.rs` hand-implements exactly these two `ServerHandler` methods (bypassing rmcp's `#[tool]` macro/ToolRouter by design — see that file's module doc comment), so this is not a no-op version bump.
- `_meta` metadata typing is split from a single `Meta` newtype into `MetaObject`, `RequestMetaObject`, and `NotificationMetaObject`.
- The protocol moves toward a stateless lifecycle (`server/discover` + per-request `_meta` replacing the `initialize`/`notifications/initialized` handshake for the new protocol version), which changes how a server would read client-negotiated capabilities.
- Minimum supported Rust version rises to 1.88 (workspace currently declares `rust-version = "1.85"` in the root `Cargo.toml`).

This surfaced while planning TASK-41 (an MCP Apps UI widget for `get_goal_progress`), whose implementation plan touches the exact two hand-written `ServerHandler` methods affected by the MRTR change, and relies on constructing `_meta` payloads using the type shape that changed. Upgrading first avoids doing that work twice against two different API shapes.

This ticket is a pure dependency upgrade — no new functionality, no behavior change to any existing tool, resource, or CLI/HTTP output.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 `rmcp` (and `rmcp-macros`) are upgraded to the latest available 3.x release in both `nom-core/Cargo.toml` and `nom-mcp/Cargo.toml`, with `Cargo.lock` updated to match.
- [x] #2 nom-core and nom-mcp compile cleanly against the new API, including updated `call_tool`/`read_resource` implementations in `nom-core/src/operation/mcp_handler.rs` for the MRTR-aware return types, and updated `_meta`/`Meta` usage for the new metadata type split.
- [x] #3 `rust-version` in the root `Cargo.toml` is bumped if the upgraded `rmcp` requires a higher MSRV than the current 1.85.
- [x] #4 All existing automated tests pass unchanged (`cargo test` across the workspace) and `cargo clippy` is clean, with no observable behavior change to any existing tool call, resource read, or CLI/HTTP output — this is a dependency upgrade, not a feature change.
- [x] #5 The stdio and HTTP serve modes (`nom-mcp serve`) still start and respond to a basic `list_tools`/`call_tool` round trip.
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Approach

Single-shot dependency upgrade — not split into sub-tickets. The Cargo.toml
version bump and the mcp_handler.rs API updates are tightly coupled (the
crate won't compile with one but not the other), so they ship together in
one commit/session per the planner's guidance against splitting inseparable
changes.

Only two files touch `rmcp` directly in this workspace:
- `nom-core/src/operation/mcp_handler.rs` (hand-written `ServerHandler` impl —
  see its module doc comment for why it bypasses the `#[tool]` macro)
- `nom-mcp/src/main.rs` (stdio serve via `ServiceExt::serve`, HTTP serve via
  `StreamableHttpService`)

No other file in nom-core or nom-mcp references `rmcp::` types, so the blast
radius is small despite the major version jump. Confirmed via
`grep -rn "rmcp::" nom-core nom-mcp` and a check for `ProtocolVersion`,
`ServerResult`/`ClientRequest`/etc. exhaustive matches, and `Meta` usage
(none found outside these two files) — so no `_` wildcard arms or Meta-type
migration are needed anywhere else.

Primary source: the official migration guide at
https://github.com/modelcontextprotocol/rust-sdk/discussions/969 (fetched
and read in full during planning — the TL;DR table and §1 "MRTR" / §8
"Metadata models" sections are the ones that apply here).

## Steps

1. **Bump the dependency version** in both `nom-core/Cargo.toml` and
   `nom-mcp/Cargo.toml`: change `rmcp = { version = "2.2.0", ... }` to
   `rmcp = { version = "3.1.2", ... }` (or whatever the actual latest 3.x
   patch is at execution time — check
   `https://github.com/modelcontextprotocol/rust-sdk/releases` /
   `cargo info rmcp` for the current latest before pinning), keeping the
   existing `features = [...]` lists unchanged (`["server", "client"]` for
   nom-core, `["server", "transport-io", "transport-streamable-http-server"]`
   for nom-mcp). Do not add the new `request-state` feature — it's for MRTR
   `requestState` HMAC sealing, which this project doesn't use.
   Run `cargo update -p rmcp` (or `cargo build`) to regenerate `Cargo.lock`;
   `rmcp-macros` is only a transitive dependency (pulled in by `rmcp`, not
   declared directly in either Cargo.toml) so it updates automatically —
   confirm its resolved version also lands on 3.x in `Cargo.lock`.

2. **Fix `nom-core/src/operation/mcp_handler.rs`** — the two hand-written
   `ServerHandler` methods that return the types rmcp changed:

   - `call_tool`: change the return type from
     `Result<CallToolResult, ErrorData>` to
     `Result<CallToolResponse, ErrorData>` and wrap both `Ok` return paths in
     `.into()` (rmcp provides `From<CallToolResult> for CallToolResponse`).
     Concretely: `Ok(CallToolResult::success(...))` becomes
     `Ok(CallToolResult::success(...).into())`, same for the `.error(...)`
     path.
   - `read_resource`: same pattern — return type becomes
     `Result<ReadResourceResponse, ErrorData>`; the `Ok(ReadResourceResult::new(...))` return in `dispatch_read_resource` either:
     (a) stays returning `ReadResourceResult` if `dispatch_read_resource`'s
     own signature stays `Result<ReadResourceResult, ErrorData>` (preferred —
     it's a private helper with its own tests asserting on
     `ReadResourceResult` fields; only the public `ServerHandler::read_resource`
     trait method needs to widen its return type and wrap the delegated call
     in `.into()`), or (b) widen it too if that turns out cleaner once you're
     looking at the actual compiler errors. Prefer (a): keep
     `dispatch_read_resource` untouched and change only:
     ```rust
     async fn read_resource(
         &self,
         request: rmcp::model::ReadResourceRequestParams,
         _context: RequestContext<RoleServer>,
     ) -> Result<ReadResourceResponse, ErrorData> {
         self.dispatch_read_resource(&request.uri).await.map(Into::into)
     }
     ```
   - Add `CallToolResponse` and `ReadResourceResponse` to the `use rmcp::{...}`
     import block (alongside the existing `CallToolResult`/`ReadResourceResult`
     imports, which stay — `dispatch_read_resource`, the tests, and the
     `Ok(CallToolResult::success(...))` construction still need them).
   - `list_tools`, `list_resources`, `get_info`, `get_tool` are untouched —
     `ListToolsResult`/`ListResourcesResult`/`InitializeResult` return types
     did not change in 3.0.
   - This project implements only `call_tool`/`read_resource` (not
     `get_prompt` — no prompts capability is enabled in `get_info()`), so
     `GetPromptResponse` is not needed.
   - No `_meta`/`Meta` code exists in this file or anywhere else in the
     workspace (confirmed by grep during planning), so the `MetaObject`/
     `RequestMetaObject`/`NotificationMetaObject` split (guide §8) requires
     no code changes — it only matters if a future ticket starts
     constructing `_meta` payloads (this is what TASK-41 will need to touch
     for MRTR/UI-widget metadata, which is *why* this upgrade is happening
     first).

3. **Check `nom-mcp/src/main.rs`** — read both serve paths after the
   `rmcp` bump and confirm they still compile as-is:
   - `run_serve_stdio`: `handler.serve(rmcp::transport::stdio())` via
     `ServiceExt` — the legacy `serve()` lifecycle is unchanged in 3.0 (guide
     TL;DR: "Client startup ... `serve()` unchanged"), so this should need
     no changes.
   - `run_serve_http`: `StreamableHttpService::new(move || Ok(handler.clone()), ...)`
     — 3.0 tightens the service bound from `S: Service<RoleServer>` to
     `S: ServerHandler` (guide §4). `McpHandler` already implements
     `ServerHandler` directly, so the existing closure factory should keep
     compiling unchanged; this is a narrowing of an already-satisfied bound,
     not a new requirement. Verify with a build rather than assuming.
   - `StreamableHttpServerConfig::default().with_cancellation_token(...)` —
     unaffected; `stateful_mode`/`with_stateful_mode` was renamed to
     `legacy_session_mode`/`with_legacy_session_mode` in 3.0, but this code
     never calls that method (it relies on the config default), so no rename
     is needed here. If the compiler disagrees, that's the method to look
     for.

4. **Build and let the compiler drive the remaining fixes**: run
   `cargo build --workspace` and `cargo test --all-features --workspace`
   repeatedly, fixing whatever the compiler flags. The migration guide
   TL;DR table (in the ticket's research, sourced from
   https://github.com/modelcontextprotocol/rust-sdk/discussions/969) is the
   reference for anything unexpected — in particular watch for:
   - Any `#[non_exhaustive]` fallout if clippy/rustc complains about a match
     on `ServerResult`/`ClientRequest`/etc. (none currently exist in this
     codebase, but double check nothing was missed).
   - Test code in `mcp_handler.rs`'s `#[cfg(test)] mod tests` — it
     destructures `ReadResourceResult { contents, .. }` directly from
     `dispatch_read_resource`'s return value (a private helper, not the
     trait method), so per step 2's approach (a) these tests need no changes.

5. **Bump MSRV**: change `rust-version = "1.85"` to `rust-version = "1.88"`
   in the root `Cargo.toml`'s `[workspace.package]` (rmcp 3.0's declared
   MSRV per the release notes). The Nix flake's toolchain
   (`pkgs.rust-bin.stable.latest.default` in `flake.nix`) already tracks
   latest stable and is unaffected — no flake change needed (confirmed
   locally: `rustc --version` reports 1.97.1).

6. **Lint and format**:
   `cargo fmt --all --check` and
   `cargo clippy --all-targets --all-features --workspace -- -D warnings`
   must be clean, matching CI (see `AGENTS.md` "Commands").

7. **Manual smoke test** (AC #5 — stdio and HTTP round trip): the existing
   automated test suite doesn't spin up a real transport end-to-end, so
   confirm manually:
   - `cargo run -p nom-mcp --bin nom-mcp -- serve stdio` and pipe a minimal
     JSON-RPC `initialize` + `tools/list` + one `tools/call` through stdin,
     or use an MCP inspector/client if one is available in the dev shell.
   - `cargo run -p nom-mcp --bin nom-mcp -- serve http --port <port>` then
     hit `/mcp` with an `initialize` request and a `tools/list` call (e.g.
     via `curl` with the streamable-HTTP content-type headers), confirming
     the server responds and doesn't panic.
   - This is exploratory verification, not a new automated test — no new
     test files are expected from this step, per the ticket's "no behavior
     change" framing.

## Verification checklist (maps to Acceptance Criteria)

- [ ] AC1: `rmcp` at latest 3.x in both crates' `Cargo.toml`; `Cargo.lock`
      shows matching `rmcp`/`rmcp-macros` versions.
- [ ] AC2: `cargo build --workspace` succeeds; `call_tool`/`read_resource`
      in `mcp_handler.rs` return the new `CallToolResponse`/
      `ReadResourceResponse` types.
- [ ] AC3: root `Cargo.toml` `rust-version = "1.88"`.
- [ ] AC4: `cargo test --all-features --workspace` passes unchanged;
      `cargo clippy --all-targets --all-features --workspace -- -D warnings`
      clean; `cargo fmt --all --check` clean.
- [ ] AC5: manual stdio and HTTP `list_tools`/`call_tool` round trip
      succeeds against the built binary.

## Risks / things that could go sideways

- The exact latest 3.x patch version may have moved past 3.1.2 by execution
  time — pin to whatever `cargo add rmcp@3` (or checking the GitHub
  releases page) resolves to at that point, not blindly to 3.1.2.
- If `cargo build` surfaces an unexpected breaking change not covered by
  this plan (the guide lists OAuth/task/subscription changes this project
  doesn't touch, but the guide could be incomplete or a further 3.x point
  release could add something new), treat the compiler as the source of
  truth and consult
  https://github.com/modelcontextprotocol/rust-sdk/discussions/969 for the
  specific area affected.
- `turso = "0.8.0-pre.4"` and other pre-release deps in `nom-core/Cargo.toml`
  are unrelated to this upgrade; don't let `cargo update` touch them
  incidentally — prefer `cargo update -p rmcp` (and `-p rmcp-macros` if
  needed) over a blanket `cargo update`.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Bumped rmcp/rmcp-macros to 3.1.2 in nom-core/Cargo.toml and nom-mcp/Cargo.toml (cargo update -p rmcp regenerated Cargo.lock, matching versions for both crates). Fixed the two MRTR-aware ServerHandler signatures in nom-core/src/operation/mcp_handler.rs: call_tool now returns Result<CallToolResponse, ErrorData> (wrapping the existing CallToolResult::success/error constructions in .into()), and read_resource now returns Result<ReadResourceResponse, ErrorData> via .map(Into::into) on the untouched dispatch_read_resource helper (which still returns ReadResourceResult, per the plan's preferred approach (a) — its tests were unaffected). No _meta/Meta code exists anywhere in the workspace, so the MetaObject/RequestMetaObject/NotificationMetaObject split required no changes. nom-mcp/src/main.rs's stdio and StreamableHttpService serve paths compiled unchanged as predicted. Bumped rust-version 1.85 -> 1.88 in root Cargo.toml per rmcp 3.0's MSRV; this unlocked clippy's collapsible_if let-chain suggestion, which then fired on three pre-existing, unrelated if-let/if nestings (nom-core/src/config.rs, goal/mod.rs, weight/mod.rs) — collapsed those to keep 'cargo clippy --all-targets --all-features --workspace -- -D warnings' clean, matching CI. cargo fmt --all --check is clean for every file this ticket touched; a pre-existing unformatted/unrelated diff in nom-core/src/food/mod.rs (already modified before this session started, unrelated to rmcp) was left untouched and excluded from the commit. cargo test --all-features --workspace: 237+ tests pass unchanged. Manual smoke test: stdio serve (initialize -> tools/list -> tools/call get_weight_today) round-tripped correctly; HTTP serve (POST /mcp with initialize, notifications/initialized, tools/list, tools/call) round-tripped correctly over SSE with a session id, no panics.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Upgraded rmcp/rmcp-macros from 2.2.0 to 3.1.2 across nom-core and nom-mcp, updated the two hand-written ServerHandler methods (call_tool, read_resource) in mcp_handler.rs to the new MRTR-aware CallToolResponse/ReadResourceResponse return types via .into(), bumped workspace MSRV to 1.88, fixed three clippy collapsible_if findings the MSRV bump newly unlocked, and verified with cargo build/test/clippy/fmt plus manual stdio and HTTP serve round trips — no behavior change.
<!-- SECTION:FINAL_SUMMARY:END -->
