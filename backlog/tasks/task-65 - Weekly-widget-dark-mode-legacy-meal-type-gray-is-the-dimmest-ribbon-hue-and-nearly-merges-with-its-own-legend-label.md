---
id: TASK-65
title: >-
  Weekly widget: dark-mode legacy meal-type gray is the dimmest ribbon hue and
  nearly merges with its own legend label
status: To Do
assignee: []
created_date: '2026-09-07 14:36'
labels:
  - review-followup
dependencies: []
priority: medium
ordinal: 75000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found during TASK-64's two-pass screenshot review of nom-core/assets/weekly_progress_widget.html (both fresh-subagent passes flagged it independently, as non-blocking). After TASK-64 widened the minimum ribbon segment, the remaining weak point of the meal-type ribbon in DARK mode is the colour itself, not its width: --meal-none #71717a measures 3.71:1 against --bg #171717 -- above the WCAG 1.4.11 floor that meal_ribbon_colours_meet_non_text_contrast enforces, but by far the dimmest of the four fills (breakfast 3.15:1 is lower still yet chromatic, lunch 6.59:1, dinner 15.10:1) and close in lightness to the --fg-muted legend text next to it, so the "(none)" swatch recedes exactly where it is meant to flag unlabelled data.

Measured candidates, both clearing the floor: #8b8b96 -> 5.32:1 vs page, #a1a1aa -> 7.00:1. The catch is neighbour separation, which today is carried by the page-coloured seam stroke, not by fill-to-fill contrast (lunch|none is only 1.78:1 fill-to-fill now, 1.24:1 at #8b8b96): a lighter gray also moves the legacy bucket toward --meal-lunch #a78bfa in lightness, which matters under colour-vision deficiency, where gray and lavender differ mainly in lightness. So this needs its own evaluation pass (swatches side by side under a deuteranopia/protanopia simulation plus a fresh screenshot review), not a one-hex swap.

Directions to evaluate (not decided): lighten --meal-none's dark half within the 3.5:1-5.5:1 band and re-check the ramp's lightness ordering; or keep the fill and give the legacy segment/legend swatch a distinct treatment (outline, hatch) so it stops competing with muted text on hue-less grounds.
<!-- SECTION:DESCRIPTION:END -->
