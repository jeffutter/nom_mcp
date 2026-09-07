---
id: TASK-65.2
title: Guard the ribbon palette against colour-vision collapse in CI
status: Blocked
assignee: []
created_date: '2026-09-07 15:29'
updated_date: '2026-09-07 21:44'
labels:
  - task
  - planned
dependencies:
  - TASK-65.2.1
  - TASK-65.2.2
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

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Both sub-tickets (TASK-65.2.1, TASK-65.2.2) are Done with their own criteria met and the full CI mirror is green at HEAD.
- [ ] #2 A committed std-only test measures every meal-fill pair, legacy bucket included, under protanopia, deuteranopia and tritanopia in both schemes, with the metric, matrix space and threshold provenance stated in its doc comment.
- [ ] #3 Reverting TASK-65.1's hatch texture, and separately repainting --meal-lunch light to a lightness adjacent to --meal-none, each make that test fail for the right reason; both red states are quoted in Implementation Notes and reverted.
- [ ] #4 The shipped palette clears every calibrated floor with margin, and the ramp rationale comment in weekly_progress_widget.html describes the palette that actually shipped.
- [ ] #5 TASK-65's acceptance criterion 4 is recorded as satisfied here, so the parent epic can move out of Blocked.
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Planning findings that change the ticket

Planning measured the premise with an implementation validated against published truth, and three of the ticket's assumptions need correcting before anyone codes against them.

**1. The motivating number does not reproduce.** The dark-scheme pair that motivated this work (`--meal-lunch #a78bfa` vs `--meal-none #71717a`, quoted as dE 3.0-3.7 under deuteranopia) measures **dE00 15.61 / dE76 25.04** under a linear-light Machado severity-1.0 simulation — comfortably separable by every convention surveyed. dE76 only inflates it further, so no metric choice rescues the 3.0-3.7 claim; it appears to have come from comparing the wrong pair or the wrong scheme.

**2. There is a real collapse, but it is in light mode, and it is protanopia.** Worst surviving separation per condition, on the shipped hexes:

| scheme | normal | protanopia | deuteranopia | tritanopia |
|---|---|---|---|---|
| light | 10.65 (lunch/dinner) | **5.05 lunch/dinner** | 6.33 lunch/dinner | 12.41 (lunch/legacy) |
| dark | 22.20 (breakfast/lunch) | 15.79 (breakfast/lunch) | 15.61 (lunch/legacy) | 9.15 (lunch/legacy) |

Light-scheme `lunch`/`dinner` fails under protanopia and clears its floor by 0.29 under deuteranopia. That single pair is the production defect. Everything else passes.

**3. The legacy-bucket exemption as written would be an escape hatch, and its proposed fallback cannot pass.** Judging `.seg-none` on its declared hex is achievable today (worst case 9.15 against a 5.66 floor), so no exemption is needed — and dropping it is what makes the acceptance criterion work, because repainting `--meal-lunch` onto the legacy grey then trips the guard instead of slipping through. The alternative sketched in the ticket (compare luminance only, since texture carries the cue) is impossible on the current palette: the effective rendered legacy fill is `#6f6f76` light / `#96969c` dark and sits just **1.08-1.26:1** from its chromatic neighbours, so any minimum-luminance rule fails immediately. Model note: the effective fill comes from stripe coverage 0.66/1.6 = 41.25% at 65% alpha over the base, which reproduces the spike's rendered contrasts (predicted 4.99:1 light / 6.09:1 dark against the recorded 4.99:1 / 6.02:1). Under that harsher pixel-level reading one more pair tightens — dark `lunch`/legacy tritan drops to 5.81 against the 5.66 floor — record that in the doc comment as a known tighter reading that is deliberately not gated, because there the texture, not the hue, is the cue being relied on.

## Decisions to implement (settled here so nobody re-decides mid-code)

- **Metric: CIEDE2000**, kL = kC = kH = 1, D65 Lab. dE76 is inflated roughly 2x on chroma-heavy pairs (measured on this palette's normal-vision pairs: dE76 45.9-104.2 where dE00 is 22.2-46.5), so a dE76 floor would systematically under-protect. CIEDE2000 also has a published known-answer table we can pin.
- **Simulation: Machado/Oliveira/Fernandes 2009, severity 1.0** (full dichromacy) for all three types. Severity snaps to tenths across tools (culori rounds to .1/.2/.3/.4/.5/.6/.8/1.0); severity 1.0 is the worst case and anomalous trichromats sit between normal and it, so it is conservative for a gate. Constants verified two ways during planning: digit-for-digit against colorspacious' transcription of the paper's supplementary tables, and the deuteranopia row is identical to the `feColorMatrix` constants already shipped in `nom-core/assets/weight_trend_widget.html:243-248`.
- **Matrix space: linear-light sRGB**, clamped to [0,1] after the multiply (Chromium clamps post-filter). This is not cosmetic: gamma-space application moves dark `lunch`/legacy deutan dE76 from 25.04 to 11.40. Thresholds are only meaningful together with the space they were calibrated in — say so in the doc comment.
- **Threshold provenance: derive it in-test from a pinned CVD-safe reference palette** (Wong/Okabe-Ito, Nature Methods 2011), taking the minimum pairwise simulated dE00 among the seven non-black swatches as the floor per condition. Measured floors: **protan 6.61, deutan 6.04, tritan 5.66**. They land at the low edge of the Sharma/CVRL "mid-range" band (2-10) and next to the ~6 dichromatic JND figure, which is the cross-check. Do **not** copy PaletteGuard's published 9.79-13.38 into our pipeline: their method (reference-palette calibration) is right, their numbers are tied to their own conventions, and importing them would silently redefine the threshold. Same reason "dE >= 10" is unusable as a borrowed constant — Colorally uses dE00>=10/15, rgblind flags collapse in CIE76, PaletteGuard uses dE00; the label hides three different measurements.
- **Scope: all six pairs, both schemes, three deficiencies, declared hexes = 36 comparisons**, plus one structural assert that `.seg-none` still paints SVG via `url(#meal-none-hatch)` so a TASK-65.1 texture revert also fails this test.
- **Location: the existing inline `mod tests`** of `nom-core/src/operation/mcp_handler.rs` (starts line 705), colour primitives nested in a `mod colour_math` inside it, guard placed after `meal_ribbon_legacy_bucket_is_not_hue_only` (ends line 1449). A separate file module was rejected: it could not reach `light_dark_pair` (line 1191) and would fork the CSS parser. No new dependencies (TASK-63's ruling; dev-deps are `serial_test`, `tempfile`, `tokio`, `wiremock`, none needed).

## Sub-tickets

Two leaves, planned separately by their own planner runs; the descriptions carry the concrete specs.

- **TASK-65.2.1** — std-only colour primitives (sRGB parse shared with `relative_luminance` at line 1203, linear sRGB -> Lab D65, CIEDE2000, Machado severity-1.0 in linear light) proven against pinned golden vectors. Independently shippable and independently testable: the published tables *are* its tests.
- **TASK-65.2.2** (depends on 65.2.1) — the guard itself, the derived floors, the single-hex retune, both red-state demonstrations, screenshot verification.

## Step 1: Colour primitives with published known-answer vectors

**Owner:** TASK-65.2.1 | **Depends:** none

SETUP: confirm the workspace builds clean first (`cargo nextest run --all-features --workspace`) so any later red is yours.

IMPLEMENTATION in `nom-core/src/operation/mcp_handler.rs`, inside `mod tests`:

- Extract the hex -> linear-channel logic trapped in `relative_luminance` (line 1203) into one shared function; leave its WCAG coefficients (`0.039_28`, `12.92`, `powf(2.4)`, `0.2126/0.7152/0.0722`) byte-identical so the shipped contrast guard cannot drift.
- Linear sRGB -> XYZ (D65): `X = 0.4124564r + 0.3575761g + 0.1804375b`, `Y = 0.2126729r + 0.7151522g + 0.0721750b`, `Z = 0.0193339r + 0.1191920g + 0.9503041b`; white point `Xn 0.95047, Yn 1.0, Zn 1.08883`; cube-root branch on `t > 216/24389`, else `(903.3t + 16)/116`.
- Full CIEDE2000 including the `G` factor, the `T` hue weighting, the `RT` rotation term and the hue-wrap rules for `dH'`. Skip-the-rotation shortcuts are not acceptable — the rotation is exactly what the published vectors exercise.
- Machado severity-1.0 matrices (verified transcription):
  - protan `[[0.152286, 1.052583, -0.204868], [0.114503, 0.786281, 0.099216], [-0.003882, -0.048116, 1.051998]]`
  - deutan `[[0.367322, 0.860642, -0.227964], [0.280085, 0.672501, 0.047414], [-0.011820, 0.042939, 0.968882]]`
  - tritan `[[1.255528, -0.076741, -0.178770], [-0.078411, 0.930809, 0.147602], [0.004733, 0.691367, 0.303900]]`
  Path: hex -> 8-bit sRGB -> linear -> matrix -> clamp [0,1] -> encode -> Lab.

TESTING. Pin the full Sharma et al. (2005) CIEDE2000 data set as `(L1,a1,b1,L2,a2,b2,expected)` rows and assert agreement to `5e-4`; pin all three Machado matrices digit-for-digit; assert the deutan matrix equals the `weight_trend_widget.html` `feColorMatrix` constants; assert `#000000` -> L* 0 and `#ffffff` -> L* 100 with a* = b* = 0. Two traps found during planning: validating against `coloraide`'s default `lab` (D50) instead of `lab-d65` breaks every row, and the trailing Pythagorean-triangle rows in some transcriptions are not colour pairs.

CHECKPOINT: suite green, clippy clean over test code.

## Step 2: Guard, calibration, and the one-hex retune

**Owner:** TASK-65.2.2 | **Depends:** Step 1

IMPLEMENTATION. `#[test] fn meal_ribbon_colours_survive_colour_vision_deficiency()` reading declared hexes through `light_dark_pair`, computing floors from the pinned Wong swatches (`#E69F00 #56B4E9 #009E73 #F0E442 #0072B2 #D55E00 #CC79A7`), asserting all 36 comparisons, plus the `.seg-none` hatch structural assert via `css_rule_body` (line 1245).

RED FIRST. Write it against the unmodified asset and quote the failure naming light `lunch`/`dinner` 5.05 vs floor 6.61 in `## Red-first evidence`, following TASK-65.1's notes format. Then fix.

THE FIX: one value. Light-scheme `--meal-lunch` (`nom-core/assets/weekly_progress_widget.html:30`) `#7c3aed` -> violet-700 `#6d28d9`. Measured minima after the change: light protan 8.28, deutan 9.56, tritan 14.31; normal-vision worst pair rises 10.65 -> 15.63; page contrast improves 5.70:1 -> 7.11:1, so SC 1.4.11 gets safer. Dinner stays put, the dark ramp stays put, no geometry changes, so TASK-63's contrast guard and TASK-64's seam guard are unaffected. Update the ramp rationale comment at lines 14-28 in the same commit. Alternates if review disagrees: purple-700 `#7e22ce`, fuchsia-700 `#a21caf`.

MUTATION CHECKS (each reverted, nothing committed red — CI gates nextest and clippy on test code): revert `.seg-none` to flat fill -> structural assert fires; repaint `--meal-lunch` light onto the legacy grey -> separation assert fires.

VERIFICATION. Full CI mirror, then the established screenshot playbook (`/tmp/widget-harness` iframe at 320px, seeded DB, detached `serve http`, `agent-browser set media light` plus a Blink deuteranopia `feColorMatrix` filter on the wrapper, fresh subagents at <=4 images each per the AGENTS.md vLLM cap, teardown). Record before/after numbers in the notes.

## Explicitly out of scope

- Per-user CVD configuration. The widget serves embedded cards in chat clients with no user-preference surface, and `prefers-color-scheme` is already the extent of adaptation available.
- APCA. TASK-63 rejected it as the gate because WCAG 3 marks the algorithm Exploratory; consistency wins.
- Retuning the dark scheme. It passes every condition measured above; touching it would be churn justified by a number that does not reproduce.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Validation provenance for the planning measurements (reproducible, scripts were throwaways in /tmp): a hand-rolled CIEDE2000 reproduces all 35 real rows of the published Sharma et al. (2005) table to within 5e-4, and the Machado severity-1.0 matrices match colorspacious' transcription digit-for-digit, with the deutan row identical to the feColorMatrix constants already in weight_trend_widget.html. The hatch blend model (stripe coverage 0.66/1.6 at 65% alpha over the base) reproduces TASK-65.1's rendered band contrasts, predicted 4.99:1 light / 6.09:1 dark against the recorded 4.99:1 / 6.02:1, which is why the effective-fill figures in the plan can be trusted without a browser. Note the white-point trap: coloraide's default lab is D50, so comparing its ciede2000 against D65 Lab rows fails every vector unless fed lab-d65. When TASK-65.2.2 lands, TASK-65's acceptance criterion 4 is discharged and the epic can be unblocked.

Parked as Blocked: this ticket is a coordination parent — AC#1 requires both sub-tickets Done, and neither has been executed. Evidence from code (not statuses): grep for ciede2000/machado/colour_math/survive_colour_vision in nom-core/src/operation/mcp_handler.rs returns nothing, so neither the colour primitives (TASK-65.2.1) nor the guard (TASK-65.2.2) exist yet. All planning decisions are settled in the parent plan above; the sub-ticket descriptions carry the full specs. Next actionable step: execute TASK-65.2.1 (already surfaces in backlog task list --ready), then TASK-65.2.2; when both flip to Done, this ticket becomes ready again and its remaining duties are just verifying the CI mirror at HEAD, checking the ACs off against the sub-tickets' evidence, recording TASK-65 AC#4 as discharged here, and unblocking TASK-65.
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
created: 2026-09-07 19:27
---
Planning correction from TASK-65.2.1 research (verified against colorspacious cvd.py, coloraide filters/cvd.py, colour-science tests/test_delta_e.py, and a local re-run of /tmp/t652_verify.py). Three defects in this plan that would have produced red-but-correct code: (1) the Machado severity-1.0 deutan and tritan matrices are mis-transcribed - correct digits are deutan [[0.367322,0.860646,-0.227968],[0.280085,0.672501,0.047413],[-0.011820,0.042940,0.968881]] and tritan [[1.255528,-0.076749,-0.178779],[-0.078411,0.930809,0.147602],[0.004733,0.691367,0.303900]], matching colorspacious and coloraide digit-for-digit; protan was right. (2) There is no feColorMatrix / filter / dichromacy constant anywhere under nom-core/assets (weight_trend_widget.html is 371 lines of render code); the Blink constants remembered at "lines 243-248" live only in the throwaway spike harness /tmp/t65-spike/harness.html:91, so any parity assert must pin them as test data rather than cite shipped markup. (3) The Sharma CIEDE2000 table carried here is corrupt and does not reproduce (worst error 1.75): rows 1-6 use b* = -82.7485, not -82.7775, and four pairs are mis-partnered; the verified 33-row set with expected values is pinned in TASK-65.2.1's implementation plan, where two independent implementations reproduce every published value to 4.9e-5. Everything else measured here did reproduce exactly with the corrected matrices - light-scheme lunch/dinner protanopia 5.05 worst pair, dark worst 15.61, Okabe-Ito floors protan 6.61 / deutan 6.04 / tritan 5.66, gamma-vs-linear dE76 11.40 vs 24.99 - so the conclusions and the one-hex fix stand unchanged.
---

created: 2026-09-07 21:44
---
Planning correction for this parent's plan, measured through the shipped colour_math primitives (TASK-65.2.1's own handoff note already supersedes six figures here; TASK-65.2.2's plan now carries the full corrected matrix and the candidate sweep). Authoritative values: Okabe-Ito floors protan 12.25 / deutan 11.61 / tritan 10.87. Shipped light scheme fails TWO conditions, not one - lunch/dinner measures 7.72 under protanopia and 10.33 under deuteranopia; dark passes everywhere (worst 14.86 tritan). The parent's 5.05/6.33/9.15 and 6.61/6.04/5.66 all came from the gamma-space pipeline that 65.2.1 disproved. Consequence for AC#4: the inherited one-hex fix (#6d28d9) clears protanopia by only 0.88, while purple-700 #7e22ce clears every floor by >=1.98 at negligible cost elsewhere, so 65.2.2 ships #7e22ce and rewrites the ramp comment to put the single hue break between breakfast and lunch. Two harness facts 65.2.2 verified end-to-end, both of which contradict how prior tickets rendered the widget: get_weekly_progress is Surfaces::MCP-only, so there is no POST /api/get_weekly_progress to curl - capture the payload by piping initialize + initialized + tools/call into serve stdio; and the legacy ribbon bucket is a meal row with meal_type IS NULL, because migration_v2.sql adds CHECK (meal_type IN ('breakfast','lunch','dinner')), which rejects any unrecognised string - relevant to TASK-67's bucket-detection cleanup too.
---
<!-- COMMENTS:END -->
