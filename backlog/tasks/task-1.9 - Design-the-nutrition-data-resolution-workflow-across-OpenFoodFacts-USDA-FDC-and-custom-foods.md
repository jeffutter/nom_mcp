---
id: TASK-1.9
title: >-
  Design the nutrition-data resolution workflow across OpenFoodFacts, USDA FDC,
  and custom foods
status: Done
assignee:
  - '@jeffutter'
created_date: '2026-08-11 04:40'
updated_date: '2026-08-11 11:19'
labels:
  - 'wayfinder:grilling'
dependencies:
  - TASK-1.3
  - TASK-1.4
  - TASK-1.5
parent_task_id: TASK-1
ordinal: 10000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Question

Design how a Food gets resolved during logging: barcode input routes to OpenFoodFacts; free-text/whole-food input routes to USDA FDC search; when neither has a match, the LLM is instructed to break a dish into ingredients and search USDA FDC per-ingredient before falling back to a Custom Food. Cover: what gets cached locally in the Food table vs re-queried live, how search results are surfaced to the LLM for disambiguation (multiple candidate matches), how a Custom Food is created and later reused/searched, and how tool descriptions should instruct the LLM to drive this workflow (including the image-to-text delegation pattern the user specified: 'transcribe the barcode', 'extract the label').
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Alternatives weighed: ingredient-breakdown fallback as always-per-ingredient-Portions (rejected -- leaves no escape hatch when a dish is mostly uncatalogued, producing many throwaway one-off Custom Foods) or always-whole-dish-aggregate (rejected -- loses per-ingredient reuse and forces macro-summation work onto the LLM even when most ingredients DO resolve); chose the hybrid with LLM-judgment collapse, consistent with the rest of this workflow being prose-guided/LLM-orchestrated rather than server-orchestrated. Collapse threshold left as prose guidance rather than a fixed numeric rule for the same reason. Result cap: 5 combined (not per-source) to keep search_food's response small, matching the 'searching IS resolving' single-lightweight-call design from TASK-1.8. Custom food search: substring/case-insensitive chosen over fuzzy/trigram (extra index/threshold machinery not justified yet) and over exact-match (reuse would rarely trigger). Barcode miss falls through to free-text search rather than going straight to Custom Food, so a product OFF simply hasn't catalogued yet still gets a shot at a USDA whole-food match. Label-photo path bypasses search_food entirely rather than trying a name search first, since a photographed nutrition label implies the food isn't in either catalog.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
search_food(query) auto-detects barcode-shaped (all-digit) queries and routes to OpenFoodFacts only; everything else routes to local Custom Foods (case-insensitive substring match on name, searched first) plus USDA FDC (Foundation/SR Legacy/Survey), merged into one ranked list capped at 5 combined candidates, custom-first then USDA's own relevance order preserved. Every returned candidate is upserted into the local foods cache as part of the call (full nutrient snapshot, no live re-query, no auto-refresh) -- search IS resolve, food_id is immediately usable. Each candidate carries food_id, name, source, and its full cached nutrient snapshot (kcal/protein/carbs/fat/fiber + serving basis), since no separate get_food_details tool exists -- the list must be self-sufficient for disambiguation. Fallbacks: a barcode miss (no OFF match) makes the LLM fall through to a free-text search_food retry using the product name; a free-text dish miss makes the LLM decompose into ingredients and resolve each individually via search_food, creating a one-off Custom Food for any ingredient found in neither source, with LLM judgment (guided by tool-description prose, not a hard threshold) collapsing to a single whole-dish Custom Food when most ingredients end up uncatalogued. Custom Foods are created per-serving via create_custom_food with no server-side dedup -- reuse relies entirely on search_food's custom-first substring match, so tool descriptions must explicitly instruct 'search before creating.' Image delegation: a barcode photo has the LLM transcribe the digits itself and call search_food(digits); a nutrition-label photo has the LLM extract the structured nutrients itself and call create_custom_food(...) directly, bypassing search entirely since a photographed label implies no catalog entry exists.
<!-- SECTION:FINAL_SUMMARY:END -->
