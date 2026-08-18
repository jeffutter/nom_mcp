---
id: TASK-59
title: Fix stale local-CLI key=value docs (clap uses --key value)
status: To Do
assignee: []
created_date: '2026-08-18 02:08'
labels:
  - docs
  - cli
dependencies: []
priority: medium
type: task
ordinal: 65000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
TASK-54 execution found that README.md and AGENTS.md document the local CLI as 'nom-mcp <operation> key=value ...', but since TASK-30 the local CLI is clap-backed and only accepts '--key value' (verified empirically: 'nom-mcp log_weight value=79.9' fails with 'unexpected argument'; 'nom-mcp log_weight --value 79.9' works). nom-core/src/cli.rs::parse_params (true key=value) is only used by nom-mcp-remote, which genuinely is key=value. Affected doc sites: README.md 'The four surfaces' table (local CLI + remote CLI rows), 'Usage: local CLI' intro + all operation examples, 'REST API' section ('matching the operation key=value arguments'), 'Usage: nom-mcp-remote' (correct there), and AGENTS.md Commands section ('cargo run -p nom-mcp --bin nom-mcp -- <operation> key=value ...'). Decide whether to also make the local CLI accept key=value for parity with the remote CLI (would need cli_router to fall back to parse_params when no subcommand matches, or per-arg handling) — if not, fix the docs to show --key value.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 README.md local-CLI sections show the actual working invocation syntax (--key value) for every documented example
- [ ] #2 AGENTS.md Commands section shows the actual working local-CLI invocation syntax
- [ ] #3 Either the local CLI accepts key=value like nom-mcp-remote (with tests), or the docs explicitly note the two CLIs differ in arg syntax
<!-- AC:END -->
