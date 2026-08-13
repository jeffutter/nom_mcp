---
id: TASK-40
title: >-
  Fix: duplicate server-bootstrap logic between run_serve_stdio and
  run_serve_http
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-13 12:39'
updated_date: '2026-08-13 13:58'
labels:
  - review-fix
dependencies: []
ordinal: 45000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
nom-mcp/src/main.rs's run_serve_stdio and run_serve_http (TASK-34/TASK-35) each independently repeat the same four lines to build a serve-mode context: AppConfig::load(), Arc::new(Clock::new(&config)?), build_clients(&config)?, and Arc::new(build_registry(clock.clone(), off_client, fdc_client)). AC #4 on both tickets requires the two transports to share an identical construction path 'so operation behavior never drifts between transports' — today that guarantee is only convention (two call sites that happen to read the same), not structure. Extract a shared helper (e.g. fn build_serve_context(config: &AppConfig) -> Result<(Arc<Clock>, Arc<OperationRegistry>), ErrorData>, or similar) that both run_serve_stdio and run_serve_http call, so future changes to registry/client construction can't accidentally diverge between stdio and HTTP serve modes. Found during review of TASK-34/TASK-35 (commits 681998a, 5b20d8b).
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Extracted build_serve_context(config) -> Result<(Arc<Clock>, Arc<OperationRegistry>), ErrorData> in nom-mcp/src/main.rs, combining the Clock::new/build_clients/build_registry sequence. run_serve_stdio and run_serve_http now both call this single helper instead of repeating the four-line construction independently, so the two transports structurally share one construction path. cargo fmt, cargo build, cargo test -p nom-mcp (16 passed), and cargo clippy -p nom-mcp --all-targets all pass.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Deduplicated serve-mode bootstrap logic: added build_serve_context() helper in nom-mcp/src/main.rs that both run_serve_stdio and run_serve_http call, replacing the previously duplicated Clock::new/build_clients/build_registry sequence. No AC list was defined on the ticket; verified via build, full test suite, and clippy.
<!-- SECTION:FINAL_SUMMARY:END -->
