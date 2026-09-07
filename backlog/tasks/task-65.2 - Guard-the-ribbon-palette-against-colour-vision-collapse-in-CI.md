---
id: TASK-65.2
title: Guard the ribbon palette against colour-vision collapse in CI
status: To Do
assignee: []
created_date: '2026-09-07 15:29'
updated_date: '2026-09-07 15:29'
labels:
  - task
dependencies:
  - TASK-65.1
parent_task_id: TASK-65
priority: medium
ordinal: 75200
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Nothing in CI measures what the meal-type ribbon looks like to a colour-blind user. `meal_ribbon_colours_meet_non_text_contrast` (nom-core/src/operation/mcp_handler.rs:1228) checks each declared hex against --bg with hand-rolled WCAG 2.x luminance, which is the only automated colour gate that exists — axe-core has no 1.4.11 rule and CI never renders a widget. TASK-65's spike showed why that is not enough: under a Machado 2009 severity-1.0 deuteranopia matrix applied in linear sRGB, the lunch fill #a78bfa and the legacy fill #71717a sit only CIE76 dE 3.0-3.7 apart, far below the dE>=10 "distinguishable" floor that colour-vision tooling (Colorally, opticquiz-cvd's collapse=10) treats as separable, so those two buckets are already collapsed for dichromats today and no lightness change recovers them. That is tolerable only because TASK-65.1 adds a non-colour cue; today it is unmeasured and therefore invisible to whoever changes the palette next.

Build the missing measurement as a std-only test module in the same inline `mod tests` of nom-core/src/operation/mcp_handler.rs, reusing the existing `light_dark_pair` helper (line 1191) to read the four `--meal-*` values and adding whatever pure-Rust colour math the guard needs. No new dependencies: TASK-63 established that regex/palette/wcag-contrast stay out, and the whole computation is a 3x3 matrix multiply plus a Lab conversion, comfortably hand-rolled in the style of the existing `relative_luminance`/`contrast_ratio` helpers.

What the guard must express: for each scheme (light half and dark half separately), simulate deuteranopia, protanopia and tritanopia, compute pairwise separation for all six hue pairs, and fail when a pair falls below a stated threshold UNLESS the collapsed pair involves the legacy bucket, whose legibility after TASK-65.1 rests on texture rather than colour and is asserted separately by `meal_ribbon_legacy_bucket_is_not_hue_only`. A guard that simply asserts every pair clears some dE would be red on day one and get weakened into uselessness.

Decisions this ticket must make and record (they were not settled during planning, which is why it is its own ticket):
- Which CVD model and severity. The spike used Machado/Oliveira/Fernandes 2009 at severity 1.0 (dichromacy); milder forms need interpolation. Chromium's own feColorMatrix constants match those matrices exactly.
- Matrix space. Applying the matrix in linear sRGB (what Blink does, and what the spike numbers assume) versus gamma-encoded space shifts absolute dE noticeably while preserving ordering; pick one, state it in the doc comment, and keep the test honest about which it is.
- Separation metric. CIE76 was enough to rank candidates; CIEDE2000 is defensible for an absolute threshold but costs more code for little discrimination here.
- Threshold provenance. Cite where 10 (and any second, stricter band) comes from instead of inventing a number.

Acceptance is not "the test passes" but "reverting TASK-65.1's texture, or swapping --meal-lunch to a lightness adjacent to --meal-none, makes this test fail for the right reason" — demonstrate both red states in the implementation notes.
<!-- SECTION:DESCRIPTION:END -->
