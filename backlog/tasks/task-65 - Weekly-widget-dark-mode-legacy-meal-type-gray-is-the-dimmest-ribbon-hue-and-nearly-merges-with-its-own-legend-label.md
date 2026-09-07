---
id: TASK-65
title: >-
  Weekly widget: dark-mode legacy meal-type gray is the dimmest ribbon hue and
  nearly merges with its own legend label
status: Dev Ready
assignee: []
created_date: '2026-09-07 14:36'
updated_date: '2026-09-07 15:38'
labels:
  - review-followup
  - planned
dependencies:
  - TASK-65.1
  - TASK-65.2
priority: medium
ordinal: 75000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found during TASK-64's two-pass screenshot review of nom-core/assets/weekly_progress_widget.html (both fresh-subagent passes flagged it independently, as non-blocking). After TASK-64 widened the minimum ribbon segment, the remaining weak point of the meal-type ribbon in DARK mode is the colour itself, not its width: --meal-none #71717a measures 3.71:1 against --bg #171717 -- above the WCAG 1.4.11 floor that meal_ribbon_colours_meet_non_text_contrast enforces, but by far the dimmest of the four fills (breakfast 3.15:1 is lower still yet chromatic, lunch 6.59:1, dinner 15.10:1) and close in lightness to the --fg-muted legend text next to it, so the "(none)" swatch recedes exactly where it is meant to flag unlabelled data.

Measured candidates, both clearing the floor: #8b8b96 -> 5.32:1 vs page, #a1a1aa -> 7.00:1. The catch is neighbour separation, which today is carried by the page-coloured seam stroke, not by fill-to-fill contrast (lunch|none is only 1.78:1 fill-to-fill now, 1.24:1 at #8b8b96): a lighter gray also moves the legacy bucket toward --meal-lunch #a78bfa in lightness, which matters under colour-vision deficiency, where gray and lavender differ mainly in lightness. So this needs its own evaluation pass (swatches side by side under a deuteranopia/protanopia simulation plus a fresh screenshot review), not a one-hex swap.

Directions to evaluate (not decided): lighten --meal-none's dark half within the 3.5:1-5.5:1 band and re-check the ramp's lightness ordering; or keep the fill and give the legacy segment/legend swatch a distinct treatment (outline, hatch) so it stops competing with muted text on hue-less grounds.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Legacy/unrecognised meal-type segments are identifiable without relying on hue at all, in both themes, verified on rendered pixels rather than declared hexes
- [ ] #2 Legacy band keeps >=3:1 against the widget background with margin at dpr 1, 2 and 3 in both themes, and stays visually distinct from the muted legend label printed beside it
- [ ] #3 Proportion remains honest: no segment becomes hollow or page-coloured, and breakfast/lunch/dinner render byte-identically to before
- [ ] #4 A deterministic, std-only CI guard fails if the legacy encoding loses its non-colour cue, and a second deterministic guard covers palette collapse under simulated dichromacy
- [ ] #5 Full workspace suite green: nextest --all-features, doctests, clippy -D warnings, fmt --check, rustdoc -D warnings
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## What planning established

The ticket's premise — "pick a lighter legacy gray" — is arithmetically impossible, and the ticket's own 3:1 criterion contradicts its "distinct from muted text" criterion. Measured on the shipped palette:

| | legacy vs page bg (needs >=3) | ceiling while staying 3:1 from --fg-muted |
|---|---|---|
| dark | #71717a = 3.71:1 | <=2.37:1 vs bg |
| light | #8f8f99 = 3.20:1 | <=1.78:1 vs bg |

No color clears both, in either theme. The real defect is WCAG 1.4.1: hue is the legacy bucket's *only* cue, so it must stop being hue-only. A foreground-tinted texture fixes both halves at once because `--fg` is bright in dark mode and dark in light mode, whereas any lightness change helps one half and always moves the fill toward the muted label text.

Spike evidence (harness + scripts kept at `~/.local/share/nom_mcp-spikes/t65/`, nothing committed): the legacy band is a 5-user-unit ribbon segment rendered at 4.69 CSS px tall, and at `MIN_SEG_WIDTH` only ~2.3 CSS px of it is visible fill after the page-coloured seam stroke eats ~0.7 user units. Candidates were scored by pixel statistics at dpr 1/2/3 plus two independent screenshot reviews (normal and Chromium deuteranopia emulation, both themes):

- **45-degree hatch over the existing fill — ship.** Effective contrast vs page 6.02:1 dark / 4.99:1 light at dpr1; 5.87 / 4.91 at dpr2; texture amplitude survives downscaling (rendered luminance sd 0.04-0.11 vs 0.00 flat).
- outline/ring — close second, rejected as primary because the ring nearly consumes the sliver's fill and drifts toward "hollow"; keep as fallback if the hatch ever reads as an artifact.
- hatch + lighter gray — endorsed by one reviewer as marginally clearer; strictly worse against the ticket's own "distinct from muted text" clause, so not chosen.
- vertical stripes — aliases into one line inside the sliver.
- hollow (fill = page colour) — rejected outright: rendered mean equals the page exactly (1.00:1), i.e. it reads as missing data, and it lies about proportion.
- lighter gray alone — rejected per the table above.
- dashed border — redundant with hollow.

Measured but deliberately not acted on here: under Machado 2009 severity-1.0 deuteranopia in linear sRGB, `--meal-lunch` #a78bfa and `--meal-none` #71717a are CIE76 dE 3.0-3.7 apart today, far below the dE>=10 separability floor used by colour-vision tooling. No lightness change recovers that pair, and CI cannot see it at all — that is TASK-65.2, which needs its own planning pass.

## Children and order

- **TASK-65.1** (planned) — texture encoding in the asset + its std-only guards. This is what actually closes the user-visible complaint.
- **TASK-65.2** (unplanned; needs its own planning pass before execution) — std-only CVD collapse guard for the palette. Depends on 65.1 because the assertion only makes sense once a non-colour cue exists.

## Integration verification when both land

Re-run the TASK-62.5/63/64 widget playbook end to end against the deployed-format asset (seed_data -> serve http -> real `get_weekly_progress` payload -> 320px host iframe), light and dark, dpr 1/2/3, normal and deuteranopia, confirming: legacy bucket reads deliberate-present in both themes; legend swatch reads as its own object beside the muted label; breakfast/lunch/dinner pixels unchanged from before this work; no console errors; graceful degradation when the host overrides `--bg`/`--fg` through `applyHostContext`. Teardown `rm -rf /tmp/nom-dev /tmp/widget-harness`. Keep every image batch at <=4 images per subagent prompt (AGENTS.md, vLLM cap).

Out of scope, recorded so nobody re-opens it here: raising `--meal-none` toward `--fg-muted` (moves it *into* the label), any `prefers-color-scheme` or `--color-scheme` change (the single-source-of-truth `light-dark()` model from TASK-62.5 stands), and changing MIN_SEG_WIDTH or the seam stroke contract.
<!-- SECTION:PLAN:END -->
