---
id: TASK-43
title: >-
  Fix: XSS via unvalidated date argument reflected into goal-progress widget
  innerHTML
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-13 18:48'
updated_date: '2026-08-13 19:02'
labels:
  - review-fix
  - planned
dependencies: []
ordinal: 48000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
get_goal_progress accepts an arbitrary, unvalidated 'date' string (nom-core/src/goal/mod.rs, GetGoalProgressRequest.date is a plain String with no format/regex validation) and echoes it back verbatim as the response's 'date' field (~line 746, query_date = req.date.clone()). The MCP Apps widget (nom-core/assets/goal_progress_widget.html, render(), ~line 250) inserts that field directly into innerHTML: html += '<p class="subtitle">' + (data.date || '') + '</p>'; appEl.innerHTML = html. Because the widget's resource declares no CSP domains, the MCP Apps host applies the default script-src 'self' 'unsafe-inline' CSP (per the comment in mcp_handler.rs), so an argument like {"date": "<img src=x onerror=alert(document.domain)>"} executes inline JS inside the widget iframe when a host renders the tool result. Fix by validating 'date' server-side against YYYY-MM-DD (reject non-conforming input with ErrorData::validation) and/or by escaping all interpolated values in the widget's render()/renderError() functions before inserting into innerHTML (e.g. build DOM nodes with textContent, or an HTML-escape helper) rather than string-concatenating into innerHTML.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Implementation Plan

Small, tightly-scoped fix touching two files. No sub-tickets needed (well under a
focused session, both halves are inherently coupled to the same vulnerability).

**Duplicate note:** TASK-45 ("Fix: goal-progress widget XSS via unescaped date
field in innerHTML") describes the same root cause from an independent review
pass of the same commit. This plan's widget-escaping half fully satisfies
TASK-45's fix too. TASK-45 will be marked as a duplicate depending on this
ticket so the loop doesn't redo the work.

### 1. Server-side validation — nom-core/src/goal/mod.rs

In `GetGoalProgress::execute_json` (~line 676-680), where `query_date` is
resolved from `req.date`, validate the caller-supplied string before it is
used or ever echoed back:

```rust
let query_date = match &req.date {
    Some(d) => {
        d.parse::<chrono::NaiveDate>().map_err(|_| {
            ErrorData::validation("date", format!("date must be in YYYY-MM-DD format, got: {d}"))
        })?;
        d.clone()
    }
    None => Clock::format_date(self.clock.today()),
};
```

- Mirrors the existing `end_date.parse::<chrono::NaiveDate>()` pattern already
  used in `nom-core/src/weekly/mod.rs::rolling_start_date` — `chrono::NaiveDate`'s
  `FromStr` only accepts strict ISO `YYYY-MM-DD`, so no custom regex is needed.
- Uses the existing `ErrorData::validation(field, reason)` constructor (see
  `nom-core/src/error.rs`), same convention as the request-deserialize error a
  few lines above.
- Add a unit test near the existing `get_goal_progress` tests (~line 1090+)
  that passes an XSS-shaped payload, e.g. `"date": "<img src=x onerror=alert(1)>"`,
  and asserts the call returns an `Err` with `ErrorData` category `Validation`
  and field `"date"` (check whatever accessor the existing tests use — grep
  `ErrorData::validation` usages in other module tests for the assertion
  pattern already in use).
- Also add a positive test confirming a well-formed date (e.g.
  `"2025-01-15"`) still succeeds, to guard against over-tightening the regex
  equivalent.

### 2. Widget-side escaping (defense in depth) — nom-core/assets/goal_progress_widget.html

Even with server-side validation, the widget must not trust that every caller
of `get_goal_progress` (including any future client of the JSON API) will
enforce the same rule. Add HTML-escaping before any dynamic string is
concatenated into `innerHTML`:

- Add a small helper near `fmt()` (~line 202):
  ```js
  function escapeHtml(str) {
    return String(str).replace(/[&<>"']/g, function (c) {
      return { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c];
    });
  }
  ```
- In `render()` (~line 245-253), escape the date before interpolating:
  `html += '<p class="subtitle">' + escapeHtml(data.date || "") + "</p>";`
- In `renderError()` (~line 255-257), escape the message parameter:
  `appEl.innerHTML = '<p class="placeholder">' + escapeHtml(message) + "</p>";`

Leave `nutrientRowHtml()`/`weightRowHtml()` numeric and status interpolations
as-is: `consumed`/`target`/`pct` are numbers produced by `fmt()`, and
`status`/`w.status` are serialized from the closed Rust `ProgressStatus` enum
("under"/"met"/"over") — not attacker-controlled, consistent with the existing
analysis already recorded on TASK-45. `label`/`unit` come from the hardcoded
`NUTRIENTS` array, not from request data.

### 3. Verification

- `cargo test -p nom-core goal::` — run the goal module's test suite,
  including the two new tests.
- `cargo fmt --all --check` and `cargo clippy --all-targets --all-features --workspace -- -D warnings`
  per AGENTS.md conventions.
- Manual read-through of the widget file's `render`/`renderError` diff — no
  build step exists for the static HTML asset, so there's nothing to compile,
  just confirm `escapeHtml` is referenced correctly at both call sites.

### 4. Housekeeping

- Mark TASK-45 as a duplicate of this ticket (add dependency / note) once this
  ticket's fix lands, since the widget-escaping half of this plan is the exact
  fix TASK-45 asks for.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Planning marked this Dev Ready, but its dependencies are not all Done, so it was moved back to To Do. It should be re-checked once its dependencies are Done.

Validated GetGoalProgressRequest.date server-side in nom-core/src/goal/mod.rs (GetGoalProgress::execute_json): parses the caller-supplied date with chrono::NaiveDate's strict ISO YYYY-MM-DD FromStr and returns ErrorData::validation("date", ...) on failure, before the value is ever stored in query_date or echoed back. Added two unit tests: test_get_goal_progress_rejects_malformed_date (XSS-shaped payload asserted to error with category Validation and field "date") and test_get_goal_progress_accepts_well_formed_date (well-formed date still succeeds and round-trips). Also hardened nom-core/assets/goal_progress_widget.html as defense in depth: added an escapeHtml() helper and used it for both the date subtitle in render() and the message in renderError() before interpolating into innerHTML, since the widget must not assume every future caller of get_goal_progress enforces the same server-side format. Left nutrientRowHtml()/weightRowHtml() numeric and enum-derived interpolations unescaped as they are not attacker-controlled (per existing TASK-45 analysis). Verified with cargo test -p nom-core goal:: (30 passed), cargo fmt --all --check (no diff in touched files), and cargo clippy -p nom-core --all-targets --all-features -- -D warnings (clean). TASK-45 already carries Dependencies: TASK-43 from planning, so no further duplicate-marking housekeeping was needed.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Fixed the XSS by validating GetGoalProgressRequest.date server-side against strict YYYY-MM-DD (chrono::NaiveDate parse, ErrorData::validation on failure) before it can be echoed back or reach the widget, and added HTML-escaping (escapeHtml helper) around the date and error-message values interpolated into the goal-progress widget's innerHTML as defense in depth. Added two unit tests covering the rejection of a malformed/XSS-shaped date and acceptance of a well-formed one; full goal:: test suite, cargo fmt, and clippy all pass.
<!-- SECTION:FINAL_SUMMARY:END -->
