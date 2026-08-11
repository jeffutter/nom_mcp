---
id: TASK-2.12
title: Remote-CLI thin binary
status: To Do
assignee: []
created_date: '2026-08-11 13:24'
labels: []
dependencies:
  - TASK-2.2
  - TASK-2.3
  - TASK-2.7
type: feature
ordinal: 31000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Scope
nom-mcp-remote: a thin binary that only makes HTTP calls against a running nom_mcp server, using the [remote] table's server_url from the shared Config type. Deserializes ErrorData from the HTTP response body and feeds it through the exact same render function local-CLI uses, so error output is identical between the two binaries.

See doc-5 §3, §9, and §10.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 nom-mcp-remote issues HTTP requests to server_url for each Operation and contains no direct DB access
- [ ] #2 on an error response, ErrorData is deserialized from the body and rendered via the same function local-CLI uses, producing identical output for the same error
<!-- AC:END -->
