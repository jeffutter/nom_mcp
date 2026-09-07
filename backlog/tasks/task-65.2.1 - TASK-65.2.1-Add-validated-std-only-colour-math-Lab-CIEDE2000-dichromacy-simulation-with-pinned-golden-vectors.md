---
id: TASK-65.2.1
title: >-
  TASK-65.2.1 Add validated std-only colour math (Lab, CIEDE2000, dichromacy
  simulation) with pinned golden vectors
status: Needs Plan
assignee: []
created_date: '2026-09-07 17:51'
updated_date: '2026-09-07 18:00'
labels: []
dependencies: []
parent_task_id: TASK-65.2
ordinal: 77200
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Build the numerical foundation for TASK-65.2's colour-vision guard: hand-rolled, dependency-free colour primitives inside `nom-core/src/operation/mcp_handler.rs`, proven correct against published known-answer vectors before any palette is judged by them. Read the parent ticket's `## Implementation Plan` first — it records every decision (metric, matrix space, thresholds) and the verified measurements, so this ticket needs no further research.

**Why a separate ticket.** The guard in the sibling ticket is only as trustworthy as this arithmetic, and these primitives are checkable against external truth in a way that palette assertions are not. Nothing here depends on the widget.

**Where the code goes.** Inside the existing `#[cfg(test)] mod tests` of `nom-core/src/operation/mcp_handler.rs` (which starts at line 705 and runs to EOF), as a nested `mod colour_math { ... }`. Nested keeps the ~200 lines of maths out of the guard's namespace while staying inside the module that already owns the widget asset const and the `light_dark_pair` CSS parser (line 1191) the sibling ticket reuses. A separate file module (`mcp_handler/colour_guard.rs`) was considered and rejected: it cannot see helpers that live in `mod tests`, so it would fork the CSS parser. No new Cargo dependencies — TASK-63 ruled that regex/palette/wcag-contrast stay out, and dev-dependencies are limited to `serial_test`, `tempfile`, `tokio`, `wiremock`, none of which belong here. Pure `std`: string slicing, `u8::from_str_radix`, `f64::powf`.

**What to implement (all four, exact specs and constants are in the parent plan's Step 1):**
1. Shared sRGB parsing: extract the hex -> linear-channel logic currently trapped inside `relative_luminance` (line 1203) into one function both it and the new code call. Keep the existing WCAG coefficients (`0.039_28`, `12.92`, `powf(2.4)`, `0.2126/0.7152/0.0722`) byte-identical so the shipped contrast guard cannot drift.
2. Linear sRGB -> CIELAB (D65), with the D65 white point stated in the doc comment.
3. CIEDE2000 (kL = kC = kH = 1) — the full formula including the `G` chrominance factor, hue-weighting `T`, the `RT` rotation term and the hue-difference wrap rules. The short form without rotation is not acceptable; the rotation term is what the published test vectors exercise.
4. Machado/Oliveira/Fernandes 2009 severity-1.0 simulation for protanomaly, deuteranomaly and tritanomaly, applied in **linear** light with clamping to `[0,1]` after the matrix, matching Chromium's `feColorMatrix` behaviour. Gamma-space application shifts results enormously (measured: dark-scheme `lunch` vs legacy under deuteranopia is dE76 25.0 linear vs 11.4 gamma) so the space is part of the definition, not an implementation detail.

**Correctness is the acceptance test, not an optional extra.** Pin the published tables as data inside the test module and assert agreement to `5e-4`:
- The Sharma et al. (2005) CIEDE2000 test-data set (transcribed as `(L1,a1,b1,L2,a2,b2,expected)` rows; arXiv:1201.2707 reproduces the table). Transcribe all 35 real rows. Two traps recorded during planning: feeding the rows through `coloraide`'s default `lab` (D50) instead of `lab-d65` silently breaks every row, and the two Pythagorean-triangle rows at the end of some transcriptions are not colour pairs — skip them rather than "fixing" a real parse bug.
- All three Machado severity-1.0 matrices, digit for digit, plus an assert that the deuteranopia matrix equals the `feColorMatrix` constants already shipped in `nom-core/assets/weight_trend_widget.html` (lines 243-248) — that parity is free evidence the transcription is right.
- Round-trip sanity: `#000000` -> L* 0, `#ffffff` -> L* 100 with a*=b*=0, and every channel-inverse identity.

**House conventions.** British "colour" in prose and function names (as in `meal_ribbon_colours_meet_non_text_contrast`); cite standards by name in doc comments, not URL-only ("Sharma et al. 2005", "Machado/Oliveira/Fernandes 2009"); table-driven loops over arrays of tuples, `assert!(x.abs() < tol, "...: {x}")` with the measured value in the message — the pattern used at `mcp_handler.rs:1023`, `meal/mod.rs:2600` and `seed/mod.rs:623`. Clippy runs `--all-targets -D warnings`, so test code must be warning-free.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A CIEDE2000 implementation agrees with every row of the published Sharma et al. (2005) test-data set to within 5e-4, with the expected values committed as test data.
- [ ] #2 All three Machado/Oliveira/Fernandes 2009 severity-1.0 matrices are pinned digit-for-digit, and the deuteranopia matrix is asserted equal to the feColorMatrix constants already shipped in weight_trend_widget.html.
- [ ] #3 Simulation applies the matrix in linear light with clamping after the multiply, and the doc comment states that space explicitly.
- [ ] #4 The hex-to-channel parsing behind relative_luminance is shared, and meal_ribbon_colours_meet_non_text_contrast still passes unchanged.
- [ ] #5 No new Cargo dependencies; cargo fmt --all --check, cargo clippy --all-targets --all-features --workspace -- -D warnings, cargo nextest run --all-features --workspace and cargo test --doc --all-features --workspace all clean.
<!-- AC:END -->
