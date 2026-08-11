---
id: TASK-1.12
title: Design config and secrets handling
status: Done
assignee:
  - Jeffery Utter
created_date: '2026-08-11 04:40'
updated_date: '2026-08-11 12:57'
labels:
  - 'wayfinder:grilling'
dependencies:
  - TASK-1.6
  - TASK-1.7
parent_task_id: TASK-1
ordinal: 13000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Question

Decide how runtime configuration is supplied: USDA FDC API key, OpenFoodFacts User-Agent string, local Turso/libSQL file path, HTTP port, and (per the 'today' ticket) the timezone source. Cover env vars vs a config file vs CLI flags, precedence between them, and where defaults live — consistent with notectl's `Config` pattern (notectl-core/src/config.rs) if that fits.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Alternatives weighed: config sources — env-vars-only (rejected, a TOML file is worth it for non-secret settings even with few of them) or full notectl stack with per-invocation CLI flags for every value (rejected, unnecessary surface for a single-user server). Secrets — env-var-only (rejected in favor of also allowing config-file storage for a set-once-and-forget desktop server; convenience won over on-disk-plaintext risk, which is documented rather than solved). Missing-key behavior — the earlier 'fail fast' framing (round 1) was narrowed once TASK-1.6's shared Operation registry made per-Operation lazy validation straightforward: eager whole-process fail-fast (rejected, would break weight/meal logging for users who haven't set up USDA yet) vs lazy-per-Operation (chosen). Paths — single ~/.nom_mcp/ dot-directory (rejected in favor of XDG split, more idiomatic and matches notectl). Remote-CLI config — standalone env-var/flag-only mechanism (rejected in favor of reusing the same Config type via its own [remote] table, one mechanism to learn).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Defaults < optional TOML config file < env vars (env wins); no CLI flags for values. Config at $XDG_CONFIG_HOME/nom_mcp/config.toml, DB at $XDG_DATA_HOME/nom_mcp/nom.db (~/.config, ~/.local/share fallbacks), env vars prefixed NOM_MCP_*, mirroring notectl's Config pattern minus its vault tier (no vault concept here). USDA FDC API key: accepted from env var or config file (env wins, plaintext-on-disk risk accepted/documented, no keychain integration); validated lazily per-Operation via TASK-1.6's registry (missing key only errors when a USDA-touching Operation runs, not at process startup) and always redacted — its type never plain-Debug/Display's the value. OpenFoodFacts User-Agent ships a working hardcoded default (nom_mcp/<version>), overridable. HTTP transport binds 127.0.0.1 by default (no-auth is a loopback-trust assumption, not open-network). Remote-CLI binary shares the same Config type/file/env mechanism via its own [remote] table (server_url) that the main binary ignores. Timezone (deferred from TASK-1.7): a timezone TOML key / NOM_MCP_TIMEZONE env var, optional IANA string, same layering as everything else, falling back to host system-local per TASK-1.7.
<!-- SECTION:FINAL_SUMMARY:END -->
