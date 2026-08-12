---
id: TASK-8
title: >-
  Fix: OffClient::lookup_barcode interpolates barcode unescaped into request URL
  (path/query injection)
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-12 01:36'
updated_date: '2026-08-12 03:06'
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
- [x] #1 OffClient::lookup_barcode builds the request URL via a method that percent-encodes the barcode (e.g. url::Url::path_segments_mut()/extend(), which the already-present url crate provides) instead of raw format! string interpolation, so no barcode value can alter the URL's path structure or inject extra query parameters
- [x] #2 A new wiremock test asserts (via wiremock::matchers::path) that a barcode containing URL meta-characters (e.g. "123/456" or "123?evil=1") reaches the mock server as a single correctly-escaped path segment, proving no path/query injection occurs
- [x] #3 Existing test_lookup_barcode_normalizes_barcode test still passes unmodified in behavior (hyphens/spaces/tabs still stripped)
- [x] #4 nix develop -c cargo test --workspace passes
- [x] #5 nix develop -c cargo clippy --all-targets --all-features --workspace -- -D warnings passes
- [x] #6 nix develop -c cargo fmt --all --check passes
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
SINGLE-FILE FIX (nom-core/src/client/off.rs). No sub-tickets — all work ships together.

PREREQUISITE: Confirm TASK-7 is Done (it is — git log shows commit ffb15f2). This builds on TASK-7's trim_end_matches('/') fix.

STEP 1 — Add base-URL validation in OffClient::new()
- After Url::parse(base_url), assert url.can_be_a_base() is true. Since we only accept http(s) URLs, this should always hold, but it guarantees path_segments_mut() won't fail later.
- If false, return a new OffError variant or reuse InvalidUrl with a custom message.

STEP 2 — Replace format! URL construction with structured building
In lookup_barcode(), replace the current:
  let base = self.base_url.as_str().trim_end_matches('/');
  let url = format!("{}/api/v2/product/{}?fields={}", base, normalized, fields.join(","));

With structured URL building using the already-present url crate:
  let mut url = self.base_url.clone();
  url.path_segments_mut()
    .map_err(|_| OffError::InvalidBase)?  // cannot fail due to STEP 1 invariant
    .extend(["api", "v2", "product", &normalized]);
  url.query_pairs_mut()
    .append_pair("fields", &fields.join(","));

This percent-encodes the barcode via path_segments_mut() (RFC 3986 path percent-encode set), so '/' → %2F, '?' → %3F, etc. Query params are also encoded through the structured API.

STEP 3 — Remove the trim_end_matches call
Since we're using url::Url for path building, the double-slash issue from TASK-7 is handled by path_segments_mut() which manages path structure correctly regardless of trailing slashes. The trim_end_matches is no longer needed.

STEP 4 — Add wiremock injection test
New async test test_lookup_barcode_injection_prevented that:
- Creates a wiremock server with Mock matching path("/api/v2/product/123%2F456") (the slash in barcode is percent-encoded)
- Calls lookup_barcode("123/456") 
- Asserts the mock receives exactly one request — proving the slash was encoded into the path segment rather than splitting it
- Also test with "123?evil=1" to verify query parameter injection is blocked

STEP 5 — Verify existing tests still pass
- test_lookup_barcode_normalizes_barcode must still pass (hyphens/spaces/tabs stripped before encoding)
- All other tests must continue to pass

STEP 6 — Quality gates
- cargo fmt --all
- cargo test --workspace
- cargo clippy --all-targets --all-features --workspace -- -D warnings
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Fixup applied post-review: OffClient::new() used assert!(!url.cannot_be_a_base(), ...) instead of returning the OffError::InvalidBase variant the Implementation Plan's Step 1 explicitly called for ('If false, return a new OffError variant'). This left a panic reachable from a config-supplied base_url instead of an error-as-value, violating the project's Resilient axis (no panic!/unwrap/expect outside tests). Replaced with an early Err(OffError::InvalidBase) return and added test_client_new_rejects_non_base_url covering a mailto: base_url. See fixup commit on c6a30ae.
<!-- SECTION:NOTES:END -->
