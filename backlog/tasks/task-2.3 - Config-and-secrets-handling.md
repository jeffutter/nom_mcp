---
id: TASK-2.3
title: Config and secrets handling
status: To Do
assignee: []
created_date: '2026-08-11 13:23'
labels: []
dependencies:
  - TASK-2.1
type: feature
ordinal: 22000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Scope
Defaults < optional TOML file ($XDG_CONFIG_HOME/nom_mcp/config.toml) < env vars (NOM_MCP_*, env wins). No CLI flags for values. DB path $XDG_DATA_HOME/nom_mcp/nom.db. USDA FDC API key from env or file, always redacted (no Debug/Display leak), validated lazily per-Operation (not at startup). OpenFoodFacts User-Agent default nom_mcp/<version>, overridable. HTTP binds 127.0.0.1 by default. Remote-CLI shares the same Config type via its own [remote] table (server_url). Timezone key (NOM_MCP_TIMEZONE / timezone TOML key, optional IANA string).

See doc-5 §9.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Config loads with correct precedence: defaults < TOML file < env vars
- [ ] #2 USDA key type never appears in Debug/Display output; missing key does not fail non-USDA operations
- [ ] #3 [remote] table parsed for remote-CLI's server_url, ignored by the main binary
- [ ] #4 timezone key present and read by config, unused until TASK for Clock service consumes it
<!-- AC:END -->
