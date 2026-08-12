---
id: TASK-7
title: 'Fix: FdcClient/OffClient base_url + path concatenation produces double slashes'
status: To Do
assignee: []
created_date: '2026-08-12 01:06'
updated_date: '2026-08-12 01:12'
labels:
  - review-followup
dependencies:
  - TASK-2.9
priority: high
type: bug
ordinal: 100
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while reviewing TASK-2.9 (nom-core/src/client/usda.rs). FdcClient::new and OffClient::new take base_url: &str and store it as a parsed url::Url. Endpoint methods build request URLs via format!("{}/v1/foods/search", self.base_url) (usda.rs) and format!("{}/api/v2/product/{}...", self.base_url, ...) (off.rs). url::Url::parse normalizes a base URL with an empty path (e.g. "https://world.openfoodfacts.org", or any override like "http://localhost:8080") to have a trailing '/' root path (as_str() == "https://world.openfoodfacts.org/"). Concatenating '/v1/...' or '/api/...' onto that already-slash-terminated string produces a double slash: "https://world.openfoodfacts.org//api/v2/product/123". This is a live bug in OffClient::with_default_base (already shipped) and a latent one in FdcClient (masked today only because USDA's default base URL 'https://api.nal.usda.gov/fdc' happens to already have a path segment, so no trailing slash is added by Url::parse — but any base_url override without a path segment, e.g. a local test/proxy server, hits it). None of the existing wiremock tests catch this because Mock::given(method(...)) matches on HTTP method only, never asserting the request path. This is a Resilient/Correct-axis gap: request URLs are only correct by accident of which base URL happens to be configured, not by construction.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 FdcClient and OffClient build request URLs without a double slash regardless of whether base_url has a trailing slash, an existing path segment, or no path segment at all (e.g. base_url = "http://localhost:8080" produces ".../v1/foods/search", not ".../v1/foods/search" with two leading slashes)
- [ ] #2 At least one wiremock test per client uses the path() matcher (wiremock::matchers::path) to assert the exact request path reaches the server correctly, for a base_url constructed both with and without a trailing path segment
- [ ] #3 nix develop -c cargo test --workspace passes
- [ ] #4 nix develop -c cargo clippy --all-targets --all-features --workspace -- -D warnings passes
- [ ] #5 nix develop -c cargo fmt --all --check passes
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
SETUP (read first): This is a Rust Cargo workspace with two crates: nom-core/ (library) and nom-mcp/ (binaries). ALL commands must run inside the Nix dev shell: either run 'direnv allow' once, or prefix every command with 'nix develop -c'. Work from the repository root unless told otherwise. Do not change pinned dependency versions.

1. Reproduce the bug first to confirm the diagnosis: write a small throwaway test (or use 'cargo test' with eprintln) showing that url::Url::parse("https://world.openfoodfacts.org").unwrap().as_str() == "https://world.openfoodfacts.org/" (trailing slash added because the input had no path), and that format!("{}/api/v2/product/123", that_url) produces a double slash. Delete the throwaway test once confirmed.

2. Fix nom-core/src/client/off.rs: in OffClient::new, after parsing base_url into a url::Url, normalize it so the stored value never ends in '/' when used for concatenation. The simplest correct fix: store base_url as the original Url, but when building request URLs, trim exactly one trailing '/' from its as_str() representation before appending the leading-slash path (e.g. let base = self.base_url.as_str().trim_end_matches('/');  then format!("{base}/api/v2/product/{normalized}?fields={...}")). Apply this in lookup_barcode.

3. Fix nom-core/src/client/usda.rs the same way: apply the trim_end_matches('/') pattern in search_foods, get_food, and get_foods_batch wherever self.base_url is formatted into a request URL.

4. Add a small private helper if it removes duplication across the 3-4 call sites within a single file (e.g. fn endpoint_url(&self, path: &str) -> String on FdcClient/OffClient) — only if it doesn't add an abstraction layer for its own sake; a one-line trim_end_matches inline at each call site is also acceptable given there are few call sites.

5. In nom-core/src/client/off.rs test module, add wiremock::matchers::path to the existing use wiremock::matchers::{header, method}; import, and add .and(path("/api/v2/product/111")) (or equivalent for the barcode used in that test) to test_lookup_barcode_success's Mock::given(...) chain, so it actually asserts the exact request path.

6. In nom-core/src/client/usda.rs test module, add wiremock::matchers::path to the existing import, and add a path assertion to test_search_foods_success (path "/fdc/v1/foods/search" against a base_url like "http://127.0.0.1:PORT/fdc" constructed in the test — or add a new dedicated test using a base_url with no trailing path segment, e.g. FdcClient::new(&format!("{}", server.uri()), "test-key") where server.uri() has no path, to specifically cover the no-path-segment case that today's tests never exercise) to prove no double slash reaches the server.

7. Run: nix develop -c cargo fmt --all -- and then nix develop -c cargo fmt --all --check to confirm no diff.

8. Run: nix develop -c cargo test --workspace and confirm all tests pass, including the new/modified path-asserting tests.

9. Run: nix develop -c cargo clippy --all-targets --all-features --workspace -- -D warnings and confirm it stays clean.

10. Commit the fix.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Independently corroborated by an automated /code-review pass over usda.rs, which also verified via the url crate that Url::parse("http://localhost:1234").as_str() == "http://localhost:1234/" and that format!("{}/v1/...", that) yields a double slash. IMPORTANT caveat for whoever implements this: do NOT naively swap in self.base_url.join("v1/foods/search") as the fix. Url::join follows RFC 3986 relative-reference resolution — since FdcClient's production base_url is "https://api.nal.usda.gov/fdc" (no trailing slash, last path segment is 'fdc'), .join("v1/foods/search") REPLACES that last segment instead of appending to it, producing "https://api.nal.usda.gov/v1/foods/search" (silently dropping /fdc) — a new, worse bug. Any fix must be verified against both a base_url with a path segment (USDA's real default) and one without (OFF's real default and bare-origin test URLs) before trusting it; the trim_end_matches('/') approach in the plan already accounts for this, .join() does not without also appending a trailing slash to base_url first.
<!-- SECTION:NOTES:END -->
