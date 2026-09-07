---
id: TASK-66
title: >-
  Legacy sliver at dpr1 undersamples the 45-degree hatch into mottled gray
  (worst in light theme)
status: To Do
assignee: []
created_date: '2026-09-07 16:13'
labels:
  - review-followup
dependencies: []
priority: medium
ordinal: 76200
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found by both TASK-65.1 screenshot passes (non-blocking; every pass still rated SHIP and judged the segment deliberate-present). The ~2.5-CSS-px minimum legacy segment at devicePixelRatio 1 carries barely one stripe period of the 1.5-CSS-px perpendicular pitch, so the texture undersamples into mottled/dithered gray rather than resolved stripes - most visible in light theme (dark stripes on #8f8f99 over white). Rendered luminance sd stays above the 0.02 floor even there (measured 0.0976 dark sliver / 0.0976-0.0651 light), so the non-colour cue technically survives; the complaint is aesthetic ('slightly dirty/moth-eaten', reviewer wording) at exactly the worst-case host (non-retina, minimum-width bucket). Directions to evaluate, not decided: clamp/skip the hatch below a segment-width threshold (must not re-open the hue-only defect - e.g. fall back to a denser sub-pitch pattern rather than flat fill), or a finer pitch that resolves >=2 periods inside the MIN_SEG_WIDTH sliver at dpr1. Guard interplay: meal_ribbon_legacy_bucket_is_not_hue_only asserts swatch-pattern pitch parity within 0.1 CSS px, so any pitch change must move both surfaces together.
<!-- SECTION:DESCRIPTION:END -->
