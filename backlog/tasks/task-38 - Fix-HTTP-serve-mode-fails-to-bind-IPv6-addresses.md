---
id: TASK-38
title: 'Fix: HTTP serve mode fails to bind IPv6 addresses'
status: To Do
assignee: []
created_date: '2026-08-13 12:38'
labels:
  - review-fix
dependencies: []
ordinal: 43000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
nom-mcp/src/main.rs:212 (run_serve_http) builds the bind address as format!("{bind_address}:{port}"), which is not bracket-safe for IPv6. config.http_bind_address (NOM_MCP_HTTP_BIND_ADDRESS) documents itself as user-configurable (default "127.0.0.1"), but setting it to an IPv6 literal like "::1" produces the string "::1:8000", which fails to parse as a std::net::SocketAddr (confirmed: addr.parse::<SocketAddr>() returns Err(AddrParseError) for "::1:8000"). TcpListener::bind then fails and 'nom-mcp serve http' exits with an error instead of binding. Fix by using SocketAddr construction that brackets IPv6 hosts (e.g. parse bind_address as IpAddr and build SocketAddr::new(ip, port), or use format!("[{bind_address}]:{port}") when the address contains a colon) so both IPv4 and IPv6 bind addresses work. Found during review of TASK-34/TASK-35 (commit 5b20d8b).
<!-- SECTION:DESCRIPTION:END -->
