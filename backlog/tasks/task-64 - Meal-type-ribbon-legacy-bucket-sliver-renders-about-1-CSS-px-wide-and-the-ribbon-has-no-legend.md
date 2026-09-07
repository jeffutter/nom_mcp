---
id: TASK-64
title: >-
  Meal-type ribbon: legacy-bucket sliver renders about 1 CSS px wide and the
  ribbon has no legend
status: Done
assignee:
  - '@ralph'
created_date: '2026-09-07 14:05'
updated_date: '2026-09-07 14:38'
labels:
  - review-followup
dependencies: []
priority: medium
ordinal: 74000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found during TASK-63's throwaway visual pass over nom-core/assets/weekly_progress_widget.html (two independent fresh-subagent screenshot passes agreed). Two related legibility gaps in the per-day meal-type composition ribbon, both pre-existing and untouched by TASK-63: (1) mealRibbon() clamps a small segment to minSliver = 1.5 user units, which at the shipped 320px-wide layout renders the legacy/unrecognized meal_type bucket as roughly 1 CSS px of solid colour (~2-3 device px at 3x) - present in the data but effectively unreadable as a band, worst in dark mode where --meal-none #71717a also sits near the 3:1 floor against --bg #171717; (2) the ribbon carries no legend, so its four hues are decodable only via each rect's <title> tooltip, which is invisible on touch and print. Suggested directions to evaluate (not decided): raise the minimum segment width or collapse sub-threshold buckets into an explicit remainder, and/or add a compact static legend row.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Ribbon geometry (nom-core/assets/weekly_progress_widget.html):
- MIN_SEG_WIDTH 1.5 -> 3.5 user units, stated next to the seam stroke it must survive. Visible fill = (MIN_SEG_WIDTH - stroke-width) * (300/320 content-box/viewBox scale): 0.70 CSS px before, 2.58 CSS px now (~8 device px at 3x). This is what the ticket's "about 1 CSS px" measurement was.
- Allocator extracted as ribbonWidths(caloriesList, barW) and made iterative. The old version clamped small buckets once and redistributed a single time, which could leave a neighbour that had been *just* above the floor sitting below it afterwards (constructible: [100,120,290,1590] at barW 26.06 yields 2.94 units for the third segment under the old single pass; the new loop reaches it in 3 passes and pins all three at 3.5). Added the missing fallback for the case where the bar cannot hold a legible segment per type: even shares, so every logged type stays visible instead of some vanishing.
- Verified with a throwaway node harness against the function as extracted from the HTML: sum(widths) == barW and every width >= min(MIN_SEG_WIDTH, barW/count) across 50k random bucket sets (1-4 buckets, barW 4-60), widths non-decreasing in calories (a smaller meal never draws wider), no NaN paths. Degenerate zero-total/empty inputs return zeros/[] and are unreachable anyway because mealBuckets() already filters calories > 0.

Legend:
- Static colour key under the Calories chart (.meal-legend), listing only meal types with calories somewhere in the week, in server order (breakfast, lunch, dinner, legacy/unrecognised last). Empty string stands in for the legacy null bucket, matching the seg-none fallback.
- Hue rules were de-duplicated to one per hue setting both fill (SVG segments) and background (HTML swatches), so the key cannot drift from what it keys. Naming goes through one mealBucketLabel() used by both the legend and the tooltips (tooltip text now capitalised: "Breakfast" rather than "breakfast"), so the two can never disagree about what a swatch means.
- Renders nothing when no day carries by_meal_type, so a week of pre-attribute data does not grow a key for an absent encoding (checked in the DOM: legendPresent false, 0 segments, 7 bars still drawn).

Tests (nom-core/src/operation/mcp_handler.rs, std-only, following TASK-63's precedent of parsing the widget asset):
- meal_ribbon_segments_survive_their_seam_stroke ties MIN_SEG_WIDTH to .meal-seg stroke-width and asserts >= 2 CSS px of surviving fill. Written red first: reverting the constant to 1.5 fails it with "keeps 0.70 CSS px of fill".
- meal_ribbon_ships_a_static_colour_key asserts each of the four hue rules paints both surfaces (fill AND background -- a fill-only rule leaves every swatch transparent while the ribbon still looks right), that the legend renderer exists and is called from render(), and that swatches have their own sizing rule.

Visual verification (screenshots kept out of the parent session per the 4-image limit; each review was a fresh subagent):
- Pass 1: before/after x light/dark at viewport 340 dpr3 with fixture data carrying a 50-cal legacy bucket on Wed and a 500-cal one on Fri. Wednesday's grey band went from invisible/illegible-hairline to a readable short band in both themes; other segments' proportions read correctly (the 2.4% bucket inflates to ~3%, judged not misleading); no regressions in bars, goal line, labels, weight section. Its one must-change nit -- the legend crowded the WEIGHT divider and read as ambiguously attached to Weight -- was applied (margin: 5px 0 9px) and re-shot.
- Pass 2 (after-light/after-dark v2): legend now clearly belongs to Calories (roughly 2:1 nearer to the day labels than to the WEIGHT heading), no excess blank space, nothing else broken. Verdict ship in both themes.
- Both passes independently flagged the same residual: dark-mode --meal-none is the dimmest element. Filed as TASK-65 rather than fixed here, because brightening it moves the legacy bucket toward --meal-lunch in lightness (fill-to-fill lunch|none drops 1.78:1 -> 1.24:1 at #8b8b96) and needs a colour-vision comparison of its own.

Full suite: 382 tests pass, doctests and rustdoc (-D warnings) clean, clippy -D warnings clean, fmt clean.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Closed both legibility gaps in the weekly-progress meal-type ribbon. The minimum segment width rose from 1.5 to 3.5 user units and the allocator behind it became an iterative pin-and-redistribute with an even-split fallback, taking the legacy bucket's surviving fill from ~0.7 CSS px (a hairline) to ~2.6 CSS px (a band) at the shipped layout; screenshot review confirms a 50-cal legacy bucket that was previously invisible now reads as a deliberate segment in both themes. The ribbon also gained a static colour key under the Calories chart -- present types only, server order, sharing one naming function with the tooltips and one rule per hue across SVG fill and HTML swatch -- so the four hues are decodable without hover, on touch and in print. Guarded by two new std-only tests over the widget asset, one of which was written red against the old constant. No acceptance criteria were defined on this ticket; its description's two findings plus the suggested directions are what was implemented, and the third item it raised in passing (dark-mode --meal-none sitting near the 3:1 floor) is deliberately deferred to TASK-65 because lightening it trades away neighbour separation.
<!-- SECTION:FINAL_SUMMARY:END -->
