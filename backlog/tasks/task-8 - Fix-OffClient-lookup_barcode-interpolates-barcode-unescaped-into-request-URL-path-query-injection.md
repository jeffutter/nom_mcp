---
id: TASK-8
title: >-
  Fix: OffClient::lookup_barcode interpolates barcode unescaped into request URL
  (path/query injection)
status: To Do
assignee: []
created_date: '2026-08-12 01:36'
labels:
  - review-followup
dependencies:
  - TASK-2.8
  - TASK-7
priority: high
type: bug
ordinal: 110
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while reviewing TASK-2.8 (nom-core/src/client/off.rs:115-141, lookup_barcode). The barcode argument is only partially normalized (hyphens/spaces/tabs stripped via .replace()) before being interpolated raw into a format! string that builds the request path: format!("{}/api/v2/product/{}?fields={}", self.base_url, normalized, fields.join(",")). No percent-encoding is applied. A barcode value containing '/', '?', '#', or '&' (e.g. supplied by an MCP tool caller, which may ultimately be LLM-controlled input) is inserted verbatim into the URL, letting it alter the request's path structure or inject additional/override query parameters (such as replacing fields=) rather than being treated as opaque path data. This is a Correct/Resilient-axis gap: request URLs are only well-formed by accident of which barcode string happens to be passed, not by construction. Depends on TASK-7 landing first since TASK-7 already modifies this exact URL-building code (to fix the unrelated double-slash bug) — implement this on top of that change, not before it, to avoid two passes touching the same lines.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 OffClient::lookup_barcode builds the request URL via a method that percent-encodes the barcode (e.g. url::Url::path_segments_mut()/extend(), which the already-present url crate provides) instead of raw format! string interpolation, so no barcode value can alter the URL's path structure or inject extra query parameters
- [ ] #2 A new wiremock test asserts (via wiremock::matchers::path) that a barcode containing URL meta-characters (e.g. "123/456" or "123?evil=1") reaches the mock server as a single correctly-escaped path segment, proving no path/query injection occurs
- [ ] #3 Existing test_lookup_barcode_normalizes_barcode test still passes unmodified in behavior (hyphens/spaces/tabs still stripped)
- [ ] #4 nix develop -c cargo test --workspace passes
- [ ] #5 nix develop -c cargo clippy --all-targets --all-features --workspace -- -D warnings passes
- [ ] #6 nix develop -c cargo fmt --all --check passes
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
SETUP (read first): This is a Rust Cargo workspace with two crates: nom-core/ (library) and nom-mcp/ (binaries). ALL commands must run inside the Nix dev shell: either run 'direnv allow' once, or prefix every command with 'nix develop -c'. Work from the repository root unless told otherwise. Do not change pinned dependency versions.

1. Confirm TASK-7 is Done first (backlog task 7 --plain). This ticket builds on TASK-7's fix to the same URL-building code in nom-core/src/client/off.rs's lookup_barcode (the trim_end_matches('/') base_url fix for the double-slash bug). If TASK-7 is not Done, stop and flag it rather than implementing against stale code.

2. In nom-core/src/client/off.rs's lookup_barcode, replace the format!(...) URL construction with structured building via the url crate (already a dependency): clone self.base_url, call .path_segments_mut() and .extend(["api", "v2", "product", &normalized]) to append the barcode as a single, automatically percent-encoded path segment, then use .query_pairs_mut().append_pair("fields", &fields.join(",")) for the fields parameter. path_segments_mut() returns Result<_, ()> for cannot-be-a-base URLs (e.g. mailto:); since base_url is always parsed from an http(s) string in OffClient::new/with_default_base, this case cannot occur in practice for this client, but do not silently unwrap() it — either validate at construction time in OffClient::new that the parsed URL can-be-a-base (returning OffError::InvalidUrl-style on failure) so the later call site can rely on that invariant, or thread a clearly-documented error through OffError. Follow this codebase's existing error style (thiserror enum, see OffError).

3. Add a new wiremock integration test in the tests module (near test_lookup_barcode_normalizes_barcode) that passes a barcode containing meta-characters (e.g. "123/456" or "123?evil=1") and uses wiremock::matchers::path to assert the exact literal path the mock server receives, proving the characters were escaped into a single path segment rather than restructuring the request.

4. Run: nix develop -c cargo fmt --all
5. Run: nix develop -c cargo test --workspace and confirm all tests pass, including the new injection test and the pre-existing normalization test.
6. Run: nix develop -c cargo clippy --all-targets --all-features --workspace -- -D warnings and confirm it stays clean.
7. Commit the fix.
<!-- SECTION:PLAN:END -->
