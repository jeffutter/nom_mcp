---
id: TASK-28
title: >-
  Fix: duplicated logged_at/logged_date parsing and avoidable String clone in
  nom-core/src/weight/mod.rs
status: To Do
assignee: []
created_date: '2026-08-13 02:07'
labels:
  - review-followup
dependencies:
  - TASK-2.15
priority: high
ordinal: 250
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while reviewing TASK-2.15 (nom-core/src/weight/mod.rs). LogWeight::execute_json (:146-163) and UpdateWeightEntry::execute_json (:328-340) both independently parse an optional ISO-8601 'logged_at' string into a DateTime<Utc>, format it back to a fixed-width string, and derive logged_date via 'Clock::format_date(self.clock.logged_date(&dt))' — the same three-step logic, duplicated verbatim within the same file (LogWeight additionally has a default-to-now branch UpdateWeightEntry doesn't need). Concise/Well-organized axis: this is exactly the kind of duplicated policy knowledge CLAUDE.md's Information Hiding section warns about — a future change to the timestamp format or the invalid-datetime error message has to be made in two places and will silently drift if only one is updated. Separately, LogWeight's insert query at :176 clones both computed strings ('stmt.query((logged_at_str.clone(), logged_date_str.clone(), req.value))') purely because they're needed again afterward for the JSON response at :208-209 — turso's query already accepts borrowed &str elsewhere in this same file (e.g. :548, :644, :746), so passing (&logged_at_str, &logged_date_str, req.value) avoids the clone entirely.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A private helper function (e.g. fn parse_logged_at(ts: &str, clock: &Clock) -> Result<(String, String), ErrorData>) in nom-core/src/weight/mod.rs computes the (logged_at_str, logged_date_str) pair from a raw timestamp string, used by both LogWeight and UpdateWeightEntry
- [ ] #2 LogWeight's default-to-now branch remains LogWeight-specific (not forced into the shared helper) since UpdateWeightEntry has no equivalent default case
- [ ] #3 LogWeight's INSERT query no longer clones logged_at_str/logged_date_str — it passes borrowed references and the owned Strings are still used afterward for the JSON response
- [ ] #4 nix develop -c cargo test -p nom-core passes
- [ ] #5 nix develop -c cargo clippy --workspace --all-targets is clean
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
SETUP (read first): This is a Rust workspace (nom-core, nom-mcp, nom-mcp-http; no WASM/web component in this repo). ALL commands must run inside the Nix dev shell: either run 'direnv allow' once, or prefix every command with 'nix develop -c'. Work from the repository root unless told otherwise. Do not change pinned dependency versions.

1. In nom-core/src/weight/mod.rs, add a private helper near build_weight_summary (after :73): 'fn parse_logged_at(ts: &str, clock: &Clock) -> Result<(String, String), ErrorData>' that does exactly what LogWeight's :147-152 branch and UpdateWeightEntry's :329-333 block both currently do: parse ts into DateTime<Utc> (mapping a parse error to ErrorData::validation("logged_at", ...)), format it as '%Y-%m-%dT%H:%M:%SZ', and compute logged_date via Clock::format_date(clock.logged_date(&dt)). Return (logged_at_str, logged_date_str).
2. In LogWeight::execute_json (:146-163), replace the 'if let Some(ref ts) = req.logged_at { ... } else { ... }' block's Some-branch body with a call to parse_logged_at(ts, &self.clock)?; keep the else-branch (default to chrono::Utc::now() + self.clock.today()) inline since it has no counterpart to share.
3. In UpdateWeightEntry::execute_json (:328-340), replace the inline parse+format+logged_date block with 'let (logged_at_str, logged_date_str) = parse_logged_at(ts, &self.clock)?;' followed by the existing UPDATE execute call.
4. In LogWeight::execute_json's INSERT query (:176), change 'stmt.query((logged_at_str.clone(), logged_date_str.clone(), req.value))' to 'stmt.query((&logged_at_str, &logged_date_str, req.value))' (or '(&logged_at_str[..], &logged_date_str[..], req.value)' matching this file's existing borrow style at :548/:644/:746) — verify logged_at_str/logged_date_str are still owned and available for the JSON response block at :206-211 afterward (they are; only the clone is removed).
5. Run: nix develop -c cargo test -p nom-core
6. Run: nix develop -c cargo clippy --workspace --all-targets
7. Run: nix develop -c cargo fmt -p nom-core
<!-- SECTION:PLAN:END -->
