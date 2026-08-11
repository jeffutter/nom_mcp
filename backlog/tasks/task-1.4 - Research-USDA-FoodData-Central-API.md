---
id: TASK-1.4
title: Research USDA FoodData Central API
status: Done
assignee:
  - '@Jeffery Utter'
created_date: '2026-08-11 04:39'
updated_date: '2026-08-11 04:46'
labels:
  - 'wayfinder:research'
dependencies: []
documentation:
  - doc-2
parent_task_id: TASK-1
ordinal: 5000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Question

What does the USDA FoodData Central API (https://fdc.nal.usda.gov/api-guide#bkmk-1) provide for whole/raw food lookups? Cover: auth (API key signup process, rate limits under the free key), search endpoint behavior (text search quality/ranking), the shape of nutrient data returned per food (which macros/nutrients, units, per what serving basis), the different FDC data types (Foundation, SR Legacy, Survey, Branded) and which are relevant here given OpenFoodFacts already covers branded/packaged foods, and whether an existing Rust crate wraps this API or it needs a bespoke client.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
USDA FoodData Central provides a free api.data.gov-keyed REST API (1,000 req/hr default; DEMO_KEY 30/hr, 50/day) with a /fdc/v1/foods/search endpoint (query, dataType, pageSize, pageNumber, sortBy, brandOwner) and /fdc/v1/food/{fdcId} plus /fdc/v1/foods batch detail endpoints; nutrient values (energy, macros, fiber, sodium, vitamins/minerals in kcal/g/mg/µg/IU) are reported per 100g across all data types, with household/serving portions available alongside via foodPortions (Foundation/SR Legacy/Survey) or servingSize/servingSizeUnit (Branded). Of the data types (Foundation, SR Legacy, Survey/FNDDS, Branded, Experimental), nom_mcp should query only Foundation + SR Legacy + Survey (FNDDS) and exclude Branded via the dataType filter since Open Food Facts already covers packaged/branded foods. No usable Rust crate exists for this API on crates.io or lib.rs, so nom_mcp should build a small bespoke reqwest-based client.
<!-- SECTION:FINAL_SUMMARY:END -->
