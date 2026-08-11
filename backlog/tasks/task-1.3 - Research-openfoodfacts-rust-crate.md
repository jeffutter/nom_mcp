---
id: TASK-1.3
title: Research openfoodfacts-rust crate
status: Done
assignee:
  - '@Jeffery Utter'
created_date: '2026-08-11 04:39'
updated_date: '2026-08-11 04:47'
labels:
  - 'wayfinder:research'
dependencies: []
documentation:
  - doc-3
parent_task_id: TASK-1
ordinal: 4000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Question

What does the `openfoodfacts-rust` crate (https://github.com/openfoodfacts/openfoodfacts-rust) actually support? Cover: barcode lookup API surface, response shape (what nutrition fields are available per product), auth/User-Agent requirements, rate limits, crate maturity (maintenance status, version), and whether it supports anything beyond barcode lookup (e.g. text search) that might overlap with USDA FDC's role.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
openfoodfacts-rust is a thin, unversioned HTTP wrapper (no typed Product/Nutriments structs — callers deserialize raw JSON themselves) around the Open Food Facts REST API; it does auto-set a generic User-Agent by default (override recommended per OFF policy) and correctly leaves rate limiting (OFF's real limits: 15 req/min/IP for product reads, 10 req/min/IP for search) entirely up to the caller. It is hosted under the official openfoodfacts GitHub org but has had no substantive code changes since March 2022 (only Dependabot CI bumps since), is not published on crates.io or docs.rs (git-dependency only, tracked as an open issue since 2022), and an outside contributor's own open PR calls it 'an unmaintained repository.' Given its thinness, the recommendation is to skip the crate and call the OFF REST API directly with reqwest and a hand-scoped serde struct for barcode lookups of packaged/branded foods, keeping USDA FDC for whole/raw foods.
<!-- SECTION:FINAL_SUMMARY:END -->
