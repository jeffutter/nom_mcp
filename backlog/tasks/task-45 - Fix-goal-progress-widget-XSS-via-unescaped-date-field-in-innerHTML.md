---
id: TASK-45
title: 'Fix: goal-progress widget XSS via unescaped date field in innerHTML'
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-13 18:49'
updated_date: '2026-08-13 19:19'
labels:
  - review-fix
  - planned
  - duplicate
dependencies:
  - TASK-43
ordinal: 50000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
nom-core/assets/goal_progress_widget.html's render() builds the widget's HTML by string concatenation and assigns it via appEl.innerHTML, including the tool result's date field verbatim: html += '<p class="subtitle">' + (data.date || "") + "</p>";. That date value is an unvalidated pass-through of the caller-supplied date argument to get_goal_progress (nom-core/src/goal/mod.rs:677-679, query_date = req.date.clone() with no format/regex validation) which is echoed back into the JSON response's date field (goal/mod.rs:746) and then rendered raw by the widget.

Since the MCP Apps widget's default CSP (per the code's own comment in mcp_handler.rs) is script-src 'self' 'unsafe-inline', an attacker (or a client that lets user input flow into the date argument) can pass a payload like {"date": "<img src=x onerror=alert(document.domain)>"}. get_goal_progress echoes it back unchanged, and the widget injects it into innerHTML, causing the inline event-handler script to execute inside the widget iframe. weightRowHtml()'s w.status field is server-computed from a closed enum so is not exploitable, but the date field has no such guarantee.

Fix by having the widget escape any interpolated text before inserting into innerHTML (e.g. a small escapeHtml() using textContent/createElement, or building rows via DOM APIs instead of string concatenation) rather than relying on the value always being a safe date string. Consider also validating/parsing the date argument server-side (reject non YYYY-MM-DD input) as defense in depth, though the client-side fix is the one that actually closes the hole.

Found during review of TASK-41 (commit a1442ff).
<!-- SECTION:DESCRIPTION:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Implementation Plan

This is a duplicate of TASK-43 ("Fix: XSS via unvalidated date argument
reflected into goal-progress widget innerHTML"), which is already Done.
TASK-43's own implementation notes explicitly state that its widget-escaping
half fully satisfies this ticket's request.

Verified directly against the current codebase — no code changes are needed:
- nom-core/src/goal/mod.rs: GetGoalProgress::execute_json validates req.date
  via chrono::NaiveDate's strict YYYY-MM-DD FromStr, returning
  ErrorData::validation("date", ...) on malformed input, before the value is
  ever stored in query_date or echoed back in the response.
- nom-core/assets/goal_progress_widget.html: an escapeHtml() helper (line
  ~210) is defined and used at both dynamic-interpolation sites into
  innerHTML — the date subtitle in render() (line 259) and the message in
  renderError() (line 270).

### Direct work for this ticket (execution phase)

There is no code to write. The only remaining action is bookkeeping:
1. Confirm (re-check) that TASK-43 is Done and the escapeHtml()/date
   validation described above is present — it is, as of this planning pass.
2. Close this ticket out as a duplicate: add the `duplicate` label and mark
   status Done. This matches the project's existing convention for
   confirmed-duplicate tickets whose fix already shipped under a sibling
   ticket (see TASK-38, which was closed the same way: labels
   `review-fix, planned, duplicate`, status Done).
3. No tests to run beyond what TASK-43 already ran (cargo test -p nom-core
   goal::, cargo fmt --all --check, cargo clippy -- -D warnings — all
   verified clean under TASK-43).

No sub-tickets are needed; there is no independent work here beyond closing
this ticket as resolved-via-duplicate.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Verified this is a confirmed duplicate of TASK-43 (Done), as identified during TASK-43's own planning pass. Re-checked directly against current codebase:

- nom-core/src/goal/mod.rs:680-684 — GetGoalProgress::execute_json validates req.date via chrono::NaiveDate's strict YYYY-MM-DD FromStr, returning ErrorData::validation("date", ...) on malformed input, before query_date is ever set or echoed back in the response.
- nom-core/assets/goal_progress_widget.html — escapeHtml() helper defined at line 210 and used at both dynamic-interpolation sites into innerHTML: the date subtitle in render() (line 259) and the message in renderError() (line 270).
- cargo test -p nom-core goal:: — 30 passed, including test_get_goal_progress_rejects_malformed_date and test_get_goal_progress_accepts_well_formed_date, which directly cover this vulnerability's fix.

No code changes made in this ticket; the fix already shipped under TASK-43. Closing as resolved-via-duplicate, matching the project's existing convention (see TASK-38).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Confirmed duplicate of TASK-43 (Done); TASK-43's server-side date validation (chrono::NaiveDate strict parsing) and widget-side escapeHtml() already close this XSS hole. No code changes needed; closing as resolved-via-duplicate.
<!-- SECTION:FINAL_SUMMARY:END -->
