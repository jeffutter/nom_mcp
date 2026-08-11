---
id: TASK-2.8
title: OpenFoodFacts client
status: To Do
assignee: []
created_date: '2026-08-11 13:23'
labels: []
dependencies:
  - TASK-2.3
parent_task_id: TASK-2
type: feature
ordinal: 27000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Scope
Direct reqwest client for the Open Food Facts REST API (barcode lookup for packaged/branded foods) — do not depend on the unmaintained openfoodfacts-rust crate. Hand-scoped serde struct for the response fields nom_mcp needs. Base URL must be a constructor parameter (not baked in) so tests can point it at a local wiremock server. Respect OFF's real rate limits (15 req/min/IP reads, 10 req/min/IP search) and set a real User-Agent from config (default nom_mcp/<version>).

See doc-5 §1 and §11 (testing strategy's base-URL requirement).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 client performs a barcode lookup against a configurable base URL and deserializes kcal/protein/carbs/fat/fiber + serving basis
- [ ] #2 User-Agent header is set from config with a working hardcoded default
- [ ] #3 base URL is a constructor parameter, not a compiled-in constant
<!-- AC:END -->
