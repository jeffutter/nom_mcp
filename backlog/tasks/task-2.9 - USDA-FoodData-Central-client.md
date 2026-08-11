---
id: TASK-2.9
title: USDA FoodData Central client
status: To Do
assignee: []
created_date: '2026-08-11 13:23'
labels: []
dependencies:
  - TASK-2.3
type: feature
ordinal: 28000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Scope
Bespoke reqwest client for the USDA FDC API (no existing Rust crate). Search endpoint (/fdc/v1/foods/search) and detail/batch endpoints (/fdc/v1/food/{fdcId}, /fdc/v1/foods). Query only Foundation + SR Legacy + Survey (FNDDS) data types via the dataType filter; exclude Branded. Nutrients per 100g, with household/serving portions surfaced alongside. API key from config (env or file), free api.data.gov key, 1,000 req/hr. Base URL must be a constructor parameter for the same testing reason as the OFF client.

See doc-5 §1 and §11.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 search queries filter to Foundation, SR Legacy, and Survey (FNDDS) data types only
- [ ] #2 detail/batch responses are parsed into kcal/protein/carbs/fat/fiber per 100g plus household portions
- [ ] #3 API key is read from config and never logged; base URL is a constructor parameter
<!-- AC:END -->
