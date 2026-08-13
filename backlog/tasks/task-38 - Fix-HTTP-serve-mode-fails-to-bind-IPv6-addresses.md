---
id: TASK-38
title: 'Fix: HTTP serve mode fails to bind IPv6 addresses'
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-13 12:38'
updated_date: '2026-08-13 18:32'
labels:
  - review-fix
  - planned
  - duplicate
dependencies: []
ordinal: 43000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
nom-mcp/src/main.rs:212 (run_serve_http) builds the bind address as format!("{bind_address}:{port}"), which is not bracket-safe for IPv6. config.http_bind_address (NOM_MCP_HTTP_BIND_ADDRESS) documents itself as user-configurable (default "127.0.0.1"), but setting it to an IPv6 literal like "::1" produces the string "::1:8000", which fails to parse as a std::net::SocketAddr (confirmed: addr.parse::<SocketAddr>() returns Err(AddrParseError) for "::1:8000"). TcpListener::bind then fails and 'nom-mcp serve http' exits with an error instead of binding. Fix by using SocketAddr construction that brackets IPv6 hosts (e.g. parse bind_address as IpAddr and build SocketAddr::new(ip, port), or use format!("[{bind_address}]:{port}") when the address contains a colon) so both IPv4 and IPv6 bind addresses work. Found during review of TASK-34/TASK-35 (commit 5b20d8b).
<!-- SECTION:DESCRIPTION:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Investigation

This ticket duplicates TASK-36, which was already completed in commit
4723497 ("TASK-36: fix HTTP serve mode IPv6 bind address parsing"),
landed today (2026-08-13, same day this review-fix ticket was filed).

TASK-38 was filed from a review of commit 5b20d8b (TASK-35), which predates
the TASK-36 fix. By the time TASK-38 was created, the exact issue it
describes had already been resolved.

## Verification

- `nom-mcp/src/main.rs` already contains `resolve_bind_addr(bind_address, port)`:
  parses `bind_address` as `std::net::IpAddr` first (correctly bracketing
  IPv6 via `SocketAddr::new`), falling back to `ToSocketAddrs` resolution
  of `"{bind_address}:{port}"` for hostnames (e.g. "localhost").
- `run_serve_http` calls `resolve_bind_addr(&bind_address, port)?` instead
  of the old `format!("{bind_address}:{port}")` string-join.
- Existing unit tests cover exactly the scenarios this ticket raises:
  - `test_resolve_bind_addr_ipv4_literal` — "127.0.0.1" -> "127.0.0.1:8000"
  - `test_resolve_bind_addr_ipv6_loopback` — "::1" -> "[::1]:8000"
  - `test_resolve_bind_addr_ipv6_unspecified` — "::" -> "[::]:8000"
  - `test_resolve_bind_addr_hostname_resolves_via_fallback` — "localhost"
- `cargo build -p nom-mcp` succeeds; `cargo test -p nom-mcp` (main.rs test
  module) covers the above cases and passes.

## Resolution

No new implementation work is required — this is a duplicate of already-
completed work (TASK-36). Close this ticket as a duplicate/no-op rather
than re-implementing the fix. If in doubt, run:

    cargo test -p nom-mcp resolve_bind_addr

to reconfirm IPv4/IPv6/hostname bind-address resolution behaves correctly.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Verified this is a duplicate of TASK-36 (commit 4723497, landed same day). nom-mcp/src/main.rs already has resolve_bind_addr() which parses bind_address as IpAddr and constructs SocketAddr::new (correctly bracketing IPv6), falling back to ToSocketAddrs for hostnames. run_serve_http calls resolve_bind_addr(&bind_address, port)?. Ran 'cargo test -p nom-mcp resolve_bind_addr' — all 4 tests pass (ipv4 literal, ipv6 loopback, ipv6 unspecified, hostname fallback). No code changes required; closing as duplicate/no-op per the implementation plan.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Duplicate of TASK-36 (already fixed in commit 4723497). Confirmed resolve_bind_addr() in nom-mcp/src/main.rs correctly brackets IPv6 addresses and all related unit tests pass. No new code changes needed; ticket closed as no-op duplicate.
<!-- SECTION:FINAL_SUMMARY:END -->
