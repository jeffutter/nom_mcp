---
id: TASK-65.2.1
title: >-
  TASK-65.2.1 Add validated std-only colour math (Lab, CIEDE2000, dichromacy
  simulation) with pinned golden vectors
status: Done
assignee: []
created_date: '2026-09-07 17:51'
updated_date: '2026-09-07 20:18'
labels:
  - planned
dependencies: []
parent_task_id: TASK-65.2
ordinal: 77200
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Build the numerical foundation for TASK-65.2's colour-vision guard: hand-rolled, dependency-free colour primitives inside `nom-core/src/operation/mcp_handler.rs`, proven correct against published known-answer vectors before any palette is judged by them. Read **this ticket's own `## Implementation Plan`** first: it settles every decision (metric, matrix space, thresholds) and carries the corrected constants. The parent's Step 1 mis-transcribes the Machado deutan/tritan matrices, cites `feColorMatrix` constants that do not exist in this repo, and carries a Sharma table that does not reproduce — take constants from this ticket's plan, never from the parent.

**Why a separate ticket.** The guard in the sibling ticket is only as trustworthy as this arithmetic, and these primitives are checkable against external truth in a way that palette assertions are not. Nothing here depends on the widget.

**Where the code goes.** Inside the existing `#[cfg(test)] mod tests` of `nom-core/src/operation/mcp_handler.rs` (which starts at line 705 and runs to EOF), as a nested `mod colour_math { ... }`. Nested keeps the ~200 lines of maths out of the guard's namespace while staying inside the module that already owns the widget asset const and the `light_dark_pair` CSS parser (line 1191) the sibling ticket reuses. A separate file module (`mcp_handler/colour_guard.rs`) was considered and rejected: it cannot see helpers that live in `mod tests`, so it would fork the CSS parser. No new Cargo dependencies — TASK-63 ruled that regex/palette/wcag-contrast stay out, and dev-dependencies are limited to `serial_test`, `tempfile`, `tokio`, `wiremock`, none of which belong here. Pure `std`: string slicing, `u8::from_str_radix`, `f64::powf`.

**What to implement (all four; exact specs and constants are in this ticket's Implementation Plan, which supersedes the parent plan's Step 1):**
1. Shared sRGB parsing: extract the hex -> linear-channel logic currently trapped inside `relative_luminance` (line 1203) into one function both it and the new code call. Keep the existing WCAG coefficients (`0.039_28`, `12.92`, `powf(2.4)`, `0.2126/0.7152/0.0722`) byte-identical so the shipped contrast guard cannot drift.
2. Linear sRGB -> CIELAB (D65), with the D65 white point stated in the doc comment.
3. CIEDE2000 (kL = kC = kH = 1) — the full formula including the `G` chrominance factor, hue-weighting `T`, the `RT` rotation term and the hue-difference wrap rules. The short form without rotation is not acceptable; the rotation term is what the published test vectors exercise.
4. Machado/Oliveira/Fernandes 2009 severity-1.0 simulation for protanomaly, deuteranomaly and tritanomaly, applied in **linear** light with clamping to `[0,1]` after the matrix, matching Chromium's `feColorMatrix` behaviour. Gamma-space application shifts results enormously (measured: dark-scheme `lunch` vs legacy under deuteranopia is dE76 25.0 linear vs 11.4 gamma) so the space is part of the definition, not an implementation detail.

**Correctness is the acceptance test, not an optional extra.** Pin the published tables as data inside the test module and assert agreement to `5e-4`:
- The Sharma et al. (2005) CIEDE2000 test-data set (transcribed as `(L1,a1,b1,L2,a2,b2,expected)` rows; arXiv:1201.2707 reproduces the table). Transcribe all 34 real rows — the verified set is pasted verbatim in this ticket's plan, fetched byte-for-byte from the first author's own machine-readable fixture (`hajim.rochester.edu/ece/sites/gsharma/ciede2000/dataNprograms/ciede2000testdata.txt`), so copy it rather than re-transcribing a paywalled PDF or a third-party transcription (the widely-copied colour-science one silently drops a row). Two traps recorded during planning: feeding the rows through `coloraide`'s default `lab` (D50) instead of `lab-d65` silently breaks every row, and the two Pythagorean-triangle rows at the end of some transcriptions are not colour pairs — skip them rather than "fixing" a real parse bug.
- All three Machado severity-1.0 matrices, digit for digit, plus an assert that the deuteranopia matrix agrees to 5e-4 with Chromium/Blink's published 3-decimal deuteranopia simulation constants, pinned as literal test data here (no such feColorMatrix markup exists anywhere under `nom-core/assets/`, despite what the parent plan claims) — that independent parity is free evidence the transcription is right.
- Round-trip sanity: `#000000` -> L* 0, `#ffffff` -> L* 100 with a*=b*=0, and every channel-inverse identity.

**House conventions.** British "colour" in prose and function names (as in `meal_ribbon_colours_meet_non_text_contrast`); cite standards by name in doc comments, not URL-only ("Sharma et al. 2005", "Machado/Oliveira/Fernandes 2009"); table-driven loops over arrays of tuples, `assert!(x.abs() < tol, "...: {x}")` with the measured value in the message — the pattern used at `mcp_handler.rs:1023`, `meal/mod.rs:2600` and `seed/mod.rs:623`. Clippy runs `--all-targets -D warnings`, so test code must be warning-free.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A CIEDE2000 implementation agrees with every row of the published Sharma et al. (2005) test-data set to within 5e-4, with the expected values committed as test data.
- [x] #2 All three Machado/Oliveira/Fernandes 2009 severity-1.0 matrices are pinned digit-for-digit against the published transcription, and the deuteranopia matrix is additionally asserted to agree with Chromium/Blink's published 3-decimal deuteranopia simulation constants - pinned as test data in this ticket, since no feColorMatrix constants exist anywhere in nom-core/assets - to within 5e-4 per element.
- [x] #3 Simulation applies the matrix in linear light with clamping after the multiply, and the doc comment states that space explicitly.
- [x] #4 The hex-to-channel parsing behind relative_luminance is shared, and meal_ribbon_colours_meet_non_text_contrast still passes unchanged.
- [x] #5 No new Cargo dependencies; cargo fmt --all --check, cargo clippy --all-targets --all-features --workspace -- -D warnings, cargo nextest run --all-features --workspace and cargo test --doc --all-features --workspace all clean.
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Scope decision: no sub-tickets

Single leaf task, ~200 lines inside one file (`nom-core/src/operation/mcp_handler.rs`), one shippable increment (test-only; zero production behaviour change). The four primitives share the sRGB parser and are proven by one table, so splitting them would create tickets that cannot ship independently. Execute as three checkpoints (A/B/C below), each left green and committable.

Everything needed to implement is in this plan: corrected constants, the full golden table, layout, lint hazards, verification commands. **Do not re-derive or re-transcribe any constant — copy from here.**

## Corrections to the inherited spec (measured, not opinion — do not re-litigate)

Four defects in the parent plan (TASK-65.2 Step 1) and in this ticket's description were found while verifying its premises against independent implementations. Each is reproduced by `/tmp/t652_verify.py` (planning throwaway) run against the sources named below.

**C1 — The parent plan's Machado deutan and tritan matrices are mis-transcribed.** Authoritative severity-1.0 tables agree digit-for-digit between two independent transcriptions of the authors' supplementary data: `colorspacious/cvd.py` (`MACHADO_ET_AL_MATRICES[...][100]`) and `coloraide/filters/cvd.py` (`MACHADO_DEUTAN[10]`, `MACHADO_TRITAN[10]`). Use these:

```
protan: [[0.152286, 1.052583, -0.204868],   (parent plan: correct)
         [0.114503, 0.786281,  0.099216],
         [-0.003882, -0.048116, 1.051998]]
deutan: [[0.367322, 0.860646, -0.227968],   (parent plan had .860642 / -.227964 / .047414 / .042939 / .968882 — all wrong digits)
         [0.280085, 0.672501,  0.047413],
         [-0.011820, 0.042940, 0.968881]]
tritan: [[1.255528, -0.076749, -0.178779],  (parent plan had -.076741 / -.178770 — wrong digits)
         [-0.078411, 0.930809,  0.147602],
         [0.004733, 0.691367,  0.303900]]
```

Both libraries apply the matrix to **linear-light** RGB as `matrix × column-vector` (`colorspacious`: the `sRGB1-linear+CVD` edge; `coloraide`: `ALLOWED_SPACES = ('srgb-linear',)`), which matches the space decision already settled in the parent plan. Row-major above, row `i` dotted with the input vector.

**C2 — The `feColorMatrix` parity target does not exist in this repo.** Acceptance criterion 2 asked for an assert against "the feColorMatrix constants already shipped in `nom-core/assets/weight_trend_widget.html` (lines 243-248)". There is no `feColorMatrix`, no `<filter>`, and no dichromacy constant anywhere under `nom-core/assets/` (repo-wide grep; `weight_trend_widget.html` is 371 lines of render code at those line numbers). The constants the parent plan remembered live in the TASK-65 spike harness `/tmp/t65-spike/harness.html:91`, which is throwaway. AC#2 has been rewritten accordingly: pin Chromium/Blink's published 3-decimal deuteranopia constants **as literal test data in this ticket** and assert agreement to 5e-4 per element. Measured worst-case deviation between the Machado deutan matrix and Blink's 3-dp triple is **0.000499**, so 5e-4 is the tightest honest tolerance (Blink's digits are rounded to 3 dp, so 5e-4 is their rounding bound). Blink's constants:

```
[[0.367, 0.861, -0.228], [0.280, 0.673, 0.047], [-0.012, 0.043, 0.969]]
```

**C3 — The parent plan's "all 35 real rows … reproduces to within 5e-4" claim is not reproducible from the data it carried.** Its pinned `SHARMA` rows disagree with two independent, mutually-agreeing CIEDE2000 implementations by up to **1.75**: the reference sample's `b*` for rows 1-6 is **-82.7485**, not `-82.7775`, and four pairs (`(50, 2.49, -0.001)`, `(60.2574, -34.0099, 36.2677)`, `(63.0109, -31.0961, -5.8663)`, `(61.2901, 3.7196, -5.3901)`) were paired against the wrong partners. Rows 4-6 are constructed to return dE00 = 1.0000 exactly and only -82.7485 reproduces that, so three deliberately-calibrated rows cannot break identically from one glyph.

Pin the table below, fetched byte-for-byte from the first author's own machine-readable fixture, `https://hajim.rochester.edu/ece/sites/gsharma/ciede2000/dataNprograms/ciede2000testdata.txt` (**34 rows**) — the same lineage CSS Color 4 names when it says its dE2000 handling was "checked against published test data". Verified during planning: an independent hand-rolled CIEDE2000 and `coloraide`'s `delta_e(method='2000')` on `lab-d65` both reproduce all 34 published expectations, worst error **4.95e-5**, 10× inside this ticket's 5e-4 tolerance. Two cautions for the doc comment: the widely-copied `colour-science` transcription silently omits one real row (row 14, `(50,-0.001,2.49)` vs `(50,0.0010,-2.49)` = 4.8045), so "every row" means these 34; and the journal's printed Table 4 could not be read (paywalled) nor an erratum located, so cite the fixture file as provenance rather than claiming to have transcribed the PDF. Skip the Pythagorean-triangle rows some appended fixtures carry — they are not colour pairs.

**C4 — Sharing the sRGB lineariser between the WCAG guard and the colour math is provably lossless, and must keep the WCAG constant.** WCAG 2.x names threshold `0.039_28`; IEC 61966-2-1 / CSS Color 4 name `0.04045`. For 8-bit channels the two are identical: the gap contains no representable value, since `10/255 = 0.039216 < 0.03928 < 0.04045 < 11/255 = 0.043137`. Measured: `|WCAG luminance − IEC luminance| = 0` for every hex in the widget palettes and the Okabe-Ito reference swatches. So the shared function keeps `0.039_28` verbatim, the shipped contrast guard's output is bit-for-bit unchanged, and the colour math is equally correct. Encode that argument as a test (see T6) so nobody "fixes" it later by forking a second lineariser.

## Layout

Nested module inside the existing test module, unprecedented in this repo but legal (a child module sees all ancestor private items; `pub` items in the private child are reachable upward):

- Declare `mod colour_math { ... }` immediately before the closing `}` of `mod tests` (**line 1699**; line 1698 closes the last test). Keeping maths *and* its golden-vector tests inside `colour_math` leaves `mod tests` holding only widget-facing tests, and gives TASK-65.2.2's guard a single `colour_math::…` entry point.
- Inside `colour_math`, start with `use super::*;` if convenient, otherwise address parents explicitly. `WEEKLY_PROGRESS_WIDGET_HTML` / `WEIGHT_TREND_WIDGET_HTML` are already in scope through `mod tests`' `use super::*;`.
- `relative_luminance` (line 1203) calls down into the child: `let c = colour_math::linear_srgb(hex); 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]`. The three luminance weights stay literally at that call site.
- Mark the child's API `pub` (visibility is capped by the private module anyway) so `tests` can reach it.

**Lint hazard that will actually bite:** CI runs `cargo clippy --all-targets --all-features --workspace -- -D warnings`, so `#[cfg(test)]` code is gated, and there is no `[lints]` section, no `.clippy.toml`, and no `#![allow(...)]` precedent in any test module (only `session_store.rs:263` `#[allow(dead_code)]` on a helper). Consequences:

- **`dead_code`:** every function this ticket adds must be called by at least one test that also lands in this ticket — including all three CVD kinds, not just deuteranopia. Do that instead of adding `#[allow(dead_code)]`.
- **`clippy::too_many_arguments`** (threshold 7): pass Lab as `[f64; 3]`, never six scalars. `delta_e_2000(lab1: [f64;3], lab2: [f64;3])` is 2 args.
- `float_cmp` and `excessive_precision` are pedantic (off by default), so long decimal literals and `(a-b).abs() < eps` are fine. Use iterators, not index loops, for the matrix multiply (`needless_range_loop` is on by default).
- `cargo doc` does not compile `cfg(test)` code, so rustdoc's `-D warnings` will not see these doc comments — but keep them well-formed and cite standards by name ("Sharma et al. 2005", "Machado/Oliveira/Fernandes 2009"), house style.
- Run `cargo fmt --all` after writing; the nested module re-indents automatically.

House conventions: British "colour" in prose and identifiers; table-driven loops over arrays of tuples; `assert!((got - want).abs() < tol, "<what>: {got}")` with the measured value interpolated (pattern at `mcp_handler.rs:1444`, `meal/mod.rs:2600`, `seed/mod.rs:623`).

## Function inventory (write exactly these shapes)

```rust
// colour_math
pub fn srgb_channels(hex: &str) -> [f64; 3]        // "#rrggbb" -> [r,g,b] in 0..=1, u8::from_str_radix
pub fn linearise(channel: f64) -> f64              // 0.039_28 / 12.92 / ((x+0.055)/1.055).powf(2.4) — see C4
pub fn linear_srgb(hex: &str) -> [f64; 3]          // srgb_channels + linearise, one pass
pub fn lab_from_linear(linear: [f64; 3]) -> [f64; 3]           // D65, see constants below
pub fn lab_from_hex(hex: &str) -> [f64; 3]         // convenience: linear_srgb then lab_from_linear
pub fn delta_e_2000(one: [f64; 3], two: [f64; 3]) -> f64       // kL = kC = kH = 1, full RT term
pub enum ColourVisionDeficiency { Protanopia, Deuteranopia, Tritanopia }
pub const MACHADO_PROTANOPIA / MACHADO_DEUTERANOPIA / MACHADO_TRITANOPIA: [[f64; 3]; 3]
pub fn simulate_cvd(hex: &str, deficiency: ColourVisionDeficiency) -> [f64; 3]  // Lab; linear light, clamp after multiply
```

Constants for `lab_from_linear` (identical to the digits the parent plan's measurements were computed with — changing them silently moves TASK-65.2.2's floors):

```
X = 0.4124564r + 0.3575761g + 0.1804375b
Y = 0.2126729r + 0.7151522g + 0.0721750b
Z = 0.0193339r + 0.1191920g + 0.9503041b
white point Xn 0.95047, Yn 1.0, Zn 1.08883 (D65)
f(t) = cbrt(t)                        if t > 216.0/24389.0
     = (903.3 * t + 16.0) / 116.0     otherwise
```

Cross-checked against `coloraide`'s `srgb → lab-d65`: agreement ≤ 0.0063 on L\*a\*b\* across the shipped palettes (their digits differ slightly downstream of XYZ). Immaterial next to separation floors near 6, but state the digits in the doc comment so 65.2.2's numbers are reproducible.

`simulate_cvd` path, and it is part of the definition, not an implementation detail: **hex → 8-bit sRGB → linear → matrix → clamp each channel to [0,1] → sRGB encode → XYZ → Lab.** Clamping mirrors Blink clamping filter output; the space choice is load-bearing — measured dark `lunch #a78bfa` vs legacy `#71717a` under deuteranopia gives dE76 **24.99** in linear light versus **11.40** if the matrix is applied to gamma-encoded values. Attribute the space honestly: the 2009 paper's PDF text was not retrievable, so credit the downstream implementers who state it outright — `colorspacious` (the `sRGB1-linear+CVD` pipeline), R `colorspace::simulate_cvd` (default `linear = TRUE`), and Rust `farg` ("precomputed 3x3 simulation matrices in linear sRGB") — plus the model's cone-fundamental derivation, rather than quoting Machado et al. directly. Doc comment must name the space, the attribution, and why it matters.

CIEDE2000 gotchas (each is where implementations diverge; all are exercised by the table below):
- Normalise `atan2` into `[0°, 360°)`.
- `h' = 0` when both `a'` and `b` are exactly zero (rows 7, 8).
- `dH'` sign from the ±180 wrap rules; `ΔH' = 2·sqrt(C1'C2')·sin(Δh'/2)`.
- `h̄'` mean rules including the `+360`/`−360` branch and the `C1'·C2' = 0` case.
- Keep `G` and `RC` as plain arithmetic — `Cb^7/(Cb^7+25^7)` and `Cbp^7/(Cbp^7+25^7)` never divide by zero, so the `if Cb > 0 else …` branches that Python transcriptions carry are unnecessary. Dropping the `RT` term makes T1 fail on rows 1-6, 18, 26, 28 (worst row 18: 31.9030 becomes 27.0542); row 16 shifts by only 4e-4, which stays inside the 5e-4 tolerance.

## Checkpoints

**A — Shared parsing, no behaviour change.** Add `mod colour_math` with `srgb_channels`/`linearise`/`linear_srgb`; rewrite `relative_luminance` to use it with the luminance weights untouched. Add T6. Gate: `cargo nextest run --all-features -p nom-core meal_ribbon` green, and `meal_ribbon_colours_meet_non_text_contrast` unchanged (provable per C4). Commit.

**B — Lab and CIEDE2000 + the golden table.** Add `lab_from_linear`/`lab_from_hex`/`delta_e_2000`, the `SHARMA_CIEDE2000_ROWS` const, and tests T1, T4. Gate: suite + clippy clean. Commit.

**C — Machado simulation.** Add the enum, the three pinned matrices, `simulate_cvd`, and tests T2, T3, T5. Gate: full CI mirror. Commit.

## Tests to write (all inside `colour_math`)

**T1 `ciede2000_matches_the_sharma_test_data_table`** — table-driven over the 34 rows pasted verbatim below; `assert!((got - expected).abs() < 5e-4, "row {i} …: got {got}, published {expected}")`.

```rust
/// Sharma, Wu & Dalal (2005), CIEDE2000 test data, as published in the
/// first author's machine-readable fixture `ciede2000testdata.txt`
/// (hajim.rochester.edu/ece/sites/gsharma/ciede2000/) — all 34 rows, expected
/// dE00 as printed (4 dp). Some circulating transcriptions drop row 14.
const SHARMA_CIEDE2000_ROWS: [([f64; 3], [f64; 3], f64); 34] = [
    (50.0000, 2.6772, -79.7751, 50.0000, 0.0000, -82.7485, 2.0425), // 1
    (50.0000, 3.1571, -77.2803, 50.0000, 0.0000, -82.7485, 2.8615), // 2
    (50.0000, 2.8361, -74.0200, 50.0000, 0.0000, -82.7485, 3.4412), // 3
    (50.0000, -1.3802, -84.2814, 50.0000, 0.0000, -82.7485, 1.0000), // 4
    (50.0000, -1.1848, -84.8006, 50.0000, 0.0000, -82.7485, 1.0000), // 5
    (50.0000, -0.9009, -85.5211, 50.0000, 0.0000, -82.7485, 1.0000), // 6
    (50.0000, 0.0000, 0.0000, 50.0000, -1.0000, 2.0000, 2.3669), // 7
    (50.0000, -1.0000, 2.0000, 50.0000, 0.0000, 0.0000, 2.3669), // 8
    (50.0000, 2.4900, -0.0010, 50.0000, -2.4900, 0.0009, 7.1792), // 9
    (50.0000, 2.4900, -0.0010, 50.0000, -2.4900, 0.0010, 7.1792), // 10
    (50.0000, 2.4900, -0.0010, 50.0000, -2.4900, 0.0011, 7.2195), // 11
    (50.0000, 2.4900, -0.0010, 50.0000, -2.4900, 0.0012, 7.2195), // 12
    (50.0000, -0.0010, 2.4900, 50.0000, 0.0009, -2.4900, 4.8045), // 13
    (50.0000, -0.0010, 2.4900, 50.0000, 0.0010, -2.4900, 4.8045), // 14
    (50.0000, -0.0010, 2.4900, 50.0000, 0.0011, -2.4900, 4.7461), // 15
    (50.0000, 2.5000, 0.0000, 50.0000, 0.0000, -2.5000, 4.3065), // 16
    (50.0000, 2.5000, 0.0000, 73.0000, 25.0000, -18.0000, 27.1492), // 17
    (50.0000, 2.5000, 0.0000, 61.0000, -5.0000, 29.0000, 22.8977), // 18
    (50.0000, 2.5000, 0.0000, 56.0000, -27.0000, -3.0000, 31.9030), // 19
    (50.0000, 2.5000, 0.0000, 58.0000, 24.0000, 15.0000, 19.4535), // 20
    (50.0000, 2.5000, 0.0000, 50.0000, 3.1736, 0.5854, 1.0000), // 21
    (50.0000, 2.5000, 0.0000, 50.0000, 3.2972, 0.0000, 1.0000), // 22
    (50.0000, 2.5000, 0.0000, 50.0000, 1.8634, 0.5757, 1.0000), // 23
    (50.0000, 2.5000, 0.0000, 50.0000, 3.2592, 0.3350, 1.0000), // 24
    (60.2574, -34.0099, 36.2677, 60.4626, -34.1751, 39.4387, 1.2644), // 25
    (63.0109, -31.0961, -5.8663, 62.8187, -29.7946, -4.0864, 1.2630), // 26
    (61.2901, 3.7196, -5.3901, 61.4292, 2.2480, -4.9620, 1.8731), // 27
    (35.0831, -44.1164, 3.7933, 35.0232, -40.0716, 1.5901, 1.8645), // 28
    (22.7233, 20.0904, -46.6940, 23.0331, 14.9730, -42.5619, 2.0373), // 29
    (36.4612, 47.8580, 18.3852, 36.2715, 50.5065, 21.2231, 1.4146), // 30
    (90.8027, -2.0831, 1.4410, 91.1528, -1.6435, 0.0447, 1.4441), // 31
    (90.9257, -0.5406, -0.9208, 88.6381, -0.8985, -0.7239, 1.5381), // 32
    (6.7747, -0.2908, -2.4247, 5.8714, -0.0985, -2.2286, 0.6377), // 33
    (2.0776, 0.0795, -1.1350, 0.9033, -0.0636, -0.5514, 0.9082), // 34
];
```

**T2 `machado_matrices_match_the_published_transcription`** — assert each of the nine entries of all three matrices equals the C1 digits exactly (`assert_eq!(MACHADO_DEUTERANOPIA[r][c], 0.860646_f64)` style, or a table-driven loop with the element in the message). Two structural assertions belong in the same test, because they pin the multiply's orientation independently of any digit: each **row** sums to 1.0 (measured max deviation 1e-6, so assert `< 1e-5`) while no column does, which is what makes `matrix × column-vector` the only form that preserves neutrals — the authors' own tutorial writes Eq. 1 as `RGB_sim = Γ_CVD · RGB`. This is the "digit-for-digit" pin: a future edit to a constant or a transposed multiply must fail loudly here rather than silently move a floor.

**T3 `machado_deutan_agrees_with_the_blink_simulation_constants`** — pin Blink's 3-dp triple (C2) as a const and assert `|(machado - blink)[i][j]| <= 5e-4` for all nine elements, naming in the doc comment that Blink's digits are rounded to 3 dp so 5e-4 is their rounding bound, and that this is an independent cross-check of the transcription, not a restatement of it.

**T4 `lab_conversion_hits_the_achromatic_extremes`** — `#000000` → L\* 0; `#ffffff` → L\* 100 with a\* = b\* = 0. Use tolerance **1e-4, not exact equality**: the measured float result for white is L\* 100.00000386666655, a\* −1.67e-5, b\* 6.7e-6, because `(903.3·t+16)/116` and the cube-root branch meet at the seam with finite precision. Also assert one mid-grey (`#808080`-class) and the primary corners so every channel of the XYZ matrix is exercised individually.

**T5 `cvd_simulation_runs_in_linear_light_not_gamma`** — pins the space as executable knowledge: assert `simulate_cvd("#a78bfa", Deuteranopia)` vs `simulate_cvd("#71717a", Deuteranopia)` gives dE76 ≈ 24.99 (tolerance ±0.25), and add a sibling assertion that feeding the matrix gamma-encoded would land near 11.40 — either by computing that variant inline in the test or by asserting the linear result differs from a documented gamma-space figure. Message must say that thresholds calibrated in one space are meaningless in the other.

**T6 `wcag_and_iec_srgb_linearisation_agree_on_eight_bit_input`** — for all 256 values `k/255`, assert the `0.039_28` branch chosen by `linearise` produces the same number the `0.04045` form would (per C4), and interpolate the proof (`10/255 < 0.03928 < 0.04045 < 11/255`) into the doc comment. This is what licenses sharing one lineariser between the shipped WCAG guard and the colour math.

## Verification

```sh
nix develop .#ci -c cargo fmt --all
nix develop .#ci -c cargo clippy --all-targets --all-features --workspace -- -D warnings
nix develop .#ci -c cargo nextest run --all-features --workspace
nix develop .#ci -c cargo test --doc --all-features --workspace
```

No new Cargo dependencies (TASK-63 ruling; dev-deps stay `serial_test`, `tempfile`, `tokio`, `wiremock`). Pure `std`: slicing, `u8::from_str_radix`, `f64::powf`/`cbrt`/`atan2`/`sin`/`cos`/`exp`/`sqrt`/`hypot`.

Expected red states, each proving a different test bites: delete the `RT` term → T1 fails on rows 1-6, 18, 26, 28; move the matrix multiply onto gamma-encoded channels → T5 fails; change any Machado digit → T2 fails; swap `-82.7485` for `-82.7775` in the table → T1 fails on rows 1-6. Demonstrate at least one, quote it in Implementation Notes, revert.

## Handoff to TASK-65.2.2 (do that ticket's work here)

Expose only what T1-T6 need; the guard itself, the Okabe-Ito floor derivation (`#E69F00 #56B4E9 #009E73 #F0E442 #0072B2 #D55E00 #CC79A7`, measured floors protan 6.61 / deutan 6.04 / tritan 5.66), the light-scheme `--meal-lunch` retune, and the screenshot playbook all belong there. Two facts it must inherit: the parent plan's Machado digits were wrong (C1) and its shipped-`feColorMatrix` premise was false (C2) — both recorded as comments on TASK-65.2 and TASK-65.2.2 so the next planner does not re-adopt them. Its measured sweep (light protan `lunch`/`dinner` 5.05 worst pair, dark worst 15.61) did reproduce exactly with the corrected matrices, so its conclusions stand.

Out of scope, unchanged from the parent plan: per-user CVD configuration, APCA, retuning the dark scheme, milder-severity interpolation (severity snaps to tenths across tools; 1.0 is the conservative worst case for a gate).
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
## Change
All code lives in `nom-core/src/operation/mcp_handler.rs` inside `mod tests`, as a nested `mod colour_math` (test-only; zero production behaviour change). Three commits: `e010982` shared sRGB parsing, `84630da` Lab + CIEDE2000 + golden table, `b2b6f0f` Machado simulation.

- `srgb_channels` / `linearise` / `linear_srgb`: `relative_luminance` now calls down into them with the WCAG weights (`0.2126/0.7152/0.0722`) still literal at its call site and `0.039_28` verbatim, so the shipped contrast guard is bit-for-bit unchanged (AC#4). `wcag_and_iec_srgb_linearisation_agree_on_eight_bit_input` proves all 256 channels take identical branches under the WCAG and IEC thresholds, which is what licenses one lineariser for both guards (C4/T6).
- `lab_from_linear` / `lab_from_hex` (D65, digits stated in the doc comment) and `delta_e_2000` with the `G` factor, hue weighting `T`, `RT` rotation and the full wrap rules; Lab passed as `[f64; 3]` (no six-scalar signatures).
- `SHARMA_CIEDE2000_ROWS`: all 34 rows from the first author's machine-readable fixture, committed as test data. Reproduced worst error across all rows: 4.95e-5, ~10x inside the 5e-4 tolerance.
- `ColourVisionDeficiency` + `MACHADO_PROTANOPIA`/`_DEUTERANOPIA`/`_TRITANOPIA` (C1 digits), `simulate_cvd` (hex -> linear light -> matrix x column-vector -> clamp each channel to [0,1] -> Lab), Blink's 3-dp deuteranopia triple pinned locally (C2). Eight tests, every added function reached by at least one of them, so no `#[allow(dead_code)]` was needed.

## Deviations from the plan, both forced by measurement
1. **The CIEDE2000 T-term's fourth cosine coefficient is 0.20, not the 0.10 that circulates** (Wikipedia and several transcriptions print 0.10). With 0.10, row 19 of the published table misses by 0.245; with 0.20 the worst of all 34 rows is 4.95e-5. Pinned by `ciede2000_matches_the_sharma_test_data_table`.
2. **The plan's T5 figures were wrong and are corrected in code.** Its "linear dE76 24.99 vs gamma 11.40" came from the planning oracle `/tmp/t652_verify.py`, whose `simulate()` fed *gamma-encoded* values straight into the linear->XYZ matrix -- a gamma-space run labelled linear. Decisive tell: that pipeline moves neutral `#808080` from L\* 53.59 to L\* 76.19, which no row-sums-to-1 matrix may do (neutrals must survive dichromacy simulation). Measured properly, the motivating pair `#a78bfa` vs `#71717a` under deuteranopia sits at **dE76 50.00 / dE00 25.38**, and 24.99 reappears as the *gamma-space* figure (**24.64 measured**). Independently confirmed against coloraide 8.12.1 (its own Machado matrices applied in `srgb-linear`): clamped, it gives 50.0038/25.3786 against my 50.0030/25.3786 -- agreement to ~1e-3, the residual being XYZ-digit differences. `cvd_simulation_runs_in_linear_light_not_gamma` now pins 50.00 and 24.64, and its doc comment records the correction so nobody re-adopts the old pair.

Also added beyond the plan's T1-T6: `simulated_lab_matches_an_independent_implementation` pins twelve simulated Lab cells (four palette colours x three deficiencies) produced by coloraide, tolerance 0.02 (measured worst deviation 0.0107, on the most saturated purple). The transcription was already pinned by T2/T3; this pins the *pipeline* -- space, clamp position, XYZ digits, white point -- which is precisely the thing that went wrong above. `every_deficiency_kind_leaves_neutral_greys_untouched` runs all three kinds end-to-end (which also discharges the dead-code hazard without an allow) and pins that a simulated fill stays neutral, so the guard can never manufacture hue separation out of a lightness difference.

One simplification against the plan's function inventory: `simulate_cvd` does not encode to sRGB and re-linearise on the way to Lab. Those two transfer functions are inverses on [0,1], so the round trip is arithmetic noise dressed as a step; the doc comment states the equivalence instead. Digits and results are unaffected (verified identical before and after removal).

## Red-first evidence (each mutation reverted)
- Delete the `RT` rotation term -> `Sharma row 1: got 1.6080411911996602, published 2.0425`.
- Apply the CVD matrix to gamma-encoded channels -> `linear-light deuteranopic separation of #a78bfa from #71717a measured 24.63952975226814, expected ~50.00`. Note the number it lands on: it reproduces the inherited plan's "linear" figure almost exactly, which is how the mislabelled space was caught rather than merely asserted.
- Expected red during development, before the coefficient fix: `#7c3aed under Protanopia a*: got 28.526988814575326, coloraide 28.5376333` set the 0.02 tolerance honestly rather than loosening it until green.

## Full gate
`cargo fmt --all --check`, `cargo clippy --all-targets --all-features --workspace -- -D warnings`, `cargo nextest run --all-features --workspace` (391 passed, 0 skipped, incl. the 8 new colour_math tests and the untouched `meal_ribbon_colours_meet_non_text_contrast`), `cargo test --doc --all-features --workspace` -- all clean under `nix develop .#ci`. No new Cargo dependencies; pure std (`from_str_radix`, `powf`, `cbrt`, `atan2`, `sin`/`cos`, `hypot`). coloraide was used only as an external oracle in /tmp, never imported by the crate.

## Handoff numbers for TASK-65.2.2 (recomputed with the corrected space -- do not use the parent plan's)
Worst pairwise simulated dE00 over the four shipped ribbon fills per scheme, and the Okabe-Ito reference floor, both measured through the shipped Rust primitives:
- light scheme worst: **7.72** (protanopia, `#7c3aed` lunch vs `#a855f7` dinner); deutan 10.33, tritan 18.77
- dark scheme worst: **14.86** (tritanopia, `#7c3aed` breakfast vs `#71717a` none); protan 17.34, deutan 18.71
- Okabe-Ito floors: protan **12.25** (`#0072B2` vs `#CC79A7`), deutan **11.61** (`#E69F00` vs `#F0E442`), tritan **10.87** (`#E69F00` vs `#CC79A7`)

These replace the parent plan's light 5.05 / dark 15.61 and Okabe-Ito 6.61 / 6.04 / 5.66, which were computed in the mislabelled space and are roughly half the true separations. Consequences for 65.2.2: the calibration ratio changes materially, and the claimed one-hex light-scheme `--meal-lunch` retune must be re-derived rather than inherited -- whether any pair actually fails a defensible floor is now an open question again, since the smallest shipped separation (7.72) is compared against an Okabe-Ito floor of 10.87 rather than 5.66. Recorded as comments on TASK-65.2 and TASK-65.2.2.
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
created: 2026-09-07 19:40
---
Executed. Two corrections to the constants this ticket told implementers to copy, both established by measurement rather than opinion: (1) the CIEDE2000 T-term's fourth cosine coefficient is 0.20, not 0.10 (with 0.10, published row 19 misses by 0.245; with 0.20 all 34 rows land within 4.95e-5); (2) the ticket's T5 figures "linear 24.99 / gamma 11.40" came from an oracle that applied the XYZ matrix to gamma-encoded values, i.e. a gamma-space run mislabelled as linear -- its own neutral-grey test would have failed (L\* 53.59 -> 76.19). Correct linear-light figure for `#a78bfa` vs `#71717a` under deuteranopia is dE76 50.00 (dE00 25.38), confirmed against coloraide 8.12.1 to ~1e-3; the gamma-space figure is 24.64. Everything the plan asked to be pinned (34 Sharma rows, three Machado matrices digit-for-digit, Blink 3-dp parity at 5e-4) is pinned and green; see Implementation Notes for the recomputed separation floors TASK-65.2.2 must calibrate against.
---
<!-- COMMENTS:END -->
