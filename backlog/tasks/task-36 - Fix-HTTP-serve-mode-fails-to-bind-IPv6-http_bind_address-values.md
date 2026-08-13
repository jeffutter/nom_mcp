---
id: TASK-36
title: 'Fix: HTTP serve mode fails to bind IPv6 http_bind_address values'
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-13 12:38'
updated_date: '2026-08-13 13:15'
labels:
  - review-fix
  - planned
dependencies: []
ordinal: 41000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
nom-mcp/src/main.rs:212 (run_serve_http) builds the listen address as format!("{bind_address}:{port}"). This works for IPv4/hostnames but breaks for any IPv6 literal (e.g. NOM_MCP_HTTP_BIND_ADDRESS=::1 or ::), since the colon-joined string is ambiguous and fails std::net::SocketAddr/TcpListener parsing (confirmed: "::1:8000".parse::<SocketAddr>() -> Err(AddrParseError)). TASK-35 is the first code to actually wire config.http_bind_address into a live listener (it was previously unused), so this is a newly-introduced gap, not pre-existing. Fix by parsing bind_address as an IpAddr and constructing the address via SocketAddr::new(ip, port) (or bracket the host when it looks like IPv6) instead of naive string formatting.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Problem
`run_serve_http` (nom-mcp/src/main.rs:212) builds the listen address with
`format!("{bind_address}:{port}")`. For an IPv6 literal (e.g. `::1` or `::`)
this produces an ambiguous string (`::1:8000`) that `SocketAddr`/`TcpListener`
parsing rejects, because a bare IPv6 literal must be bracketed before
appending a port (`[::1]:8000`).

## Fix (single function, nom-mcp/src/main.rs, run_serve_http)
Replace the naive string join with address construction that treats
`bind_address` as an IP literal first, falling back to a host:port string
(for the hostname case, which `config.http_bind_address` as a plain `String`
must still support — e.g. someone setting it to `localhost`):

```rust
let addr: std::net::SocketAddr = match bind_address.parse::<std::net::IpAddr>() {
    Ok(ip) => std::net::SocketAddr::new(ip, port),
    Err(_) => {
        // Not a bare IP literal (e.g. a hostname) — resolve via ToSocketAddrs.
        format!("{bind_address}:{port}")
            .to_socket_addrs()?
            .next()
            .ok_or_else(|| format!("could not resolve bind address: {bind_address}"))?
    }
};
let listener = tokio::net::TcpListener::bind(addr).await?;
```

Notes:
- `std::net::ToSocketAddrs::to_socket_addrs` is sync/blocking DNS resolution;
  it's fine here since this runs once at startup before serving, same as the
  current behavior implicitly did via `TcpListener::bind(&str)`.
- Need `use std::net::ToSocketAddrs;` (or fully qualify) for the fallback
  branch.
- `tracing::info!(%addr, ...)` continues to work since `SocketAddr` implements
  `Display` and already brackets IPv6 correctly (e.g. `[::1]:8000`).

## Verification
- Add/extend a unit test (or a small `#[cfg(test)]` helper if the address
  construction is extracted into a standalone function, e.g.
  `fn resolve_bind_addr(bind_address: &str, port: u16) -> io::Result<SocketAddr>`)
  covering:
  - IPv4 literal (`127.0.0.1`) -> unchanged behavior
  - IPv6 literal (`::1`) -> succeeds, produces `[::1]:<port>`
  - IPv6 unspecified (`::`) -> succeeds
  - hostname (`localhost`) -> resolves via fallback path
  Extracting the parsing into a small pure function makes it directly
  unit-testable without actually binding a socket.
- Run `cargo test -p nom-mcp` (or workspace-wide `cargo test`) to confirm
  existing tests still pass.
- Manually sanity check (optional, not required for CI): 
  `NOM_MCP_HTTP_BIND_ADDRESS=::1 cargo run -p nom-mcp -- serve http --port 0`
  no longer errors with an AddrParseError-derived bind failure.

## Scope
Single function change in nom-mcp/src/main.rs plus a small extracted helper
and unit tests. No sub-tickets needed — this is a small, well-bounded fix.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented the fix from the plan in nom-mcp/src/main.rs:

- Extracted resolve_bind_addr(bind_address: &str, port: u16) -> io::Result<SocketAddr>: tries bind_address as a bare IpAddr literal first (covers IPv4/IPv6 including "::1" and "::"), falling back to ToSocketAddrs resolution of "{bind_address}:{port}" for hostnames (e.g. "localhost").
- run_serve_http now calls resolve_bind_addr(&bind_address, port)? and binds TcpListener to the resulting SocketAddr instead of naively formatting "{bind_address}:{port}" as a string.
- tracing::info!(%addr, ...) unchanged — SocketAddr's Display already brackets IPv6 correctly.
- Added 4 unit tests covering IPv4 literal, IPv6 loopback (::1), IPv6 unspecified (::), and hostname (localhost) via the ToSocketAddrs fallback.

Verified: cargo build -p nom-mcp, cargo test -p nom-mcp (9/9 main.rs tests pass), cargo fmt --check -p nom-mcp (clean), cargo clippy -p nom-mcp --all-targets (clean), cargo test --workspace (all passing).

No acceptance criteria were defined on this ticket; work matches the Implementation Plan's Fix and Verification sections exactly (extracted pure function + unit tests instead of requiring a live socket bind).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Fixed HTTP serve mode's IPv6 bind failure by extracting resolve_bind_addr() in nom-mcp/src/main.rs, which parses bind_address as an IpAddr (bracketing IPv6 correctly via SocketAddr::new) with a ToSocketAddrs hostname fallback, replacing the naive format!("{bind_address}:{port}") string join; added 4 unit tests and verified the full workspace test suite, fmt, and clippy pass.
<!-- SECTION:FINAL_SUMMARY:END -->
