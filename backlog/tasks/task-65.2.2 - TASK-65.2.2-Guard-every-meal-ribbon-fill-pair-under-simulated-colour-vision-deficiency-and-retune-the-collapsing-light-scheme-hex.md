---
id: TASK-65.2.2
title: >-
  TASK-65.2.2 Guard every meal-ribbon fill pair under simulated colour vision
  deficiency and retune the collapsing light-scheme hex
status: Needs Plan
assignee: []
created_date: '2026-09-07 17:52'
updated_date: '2026-09-07 19:27'
labels: []
dependencies:
  - TASK-65.2.1
parent_task_id: TASK-65.2
ordinal: 78200
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Wire the shipped weekly-progress ribbon to the colour-vision guard built by TASK-65.2.1: assert every pair of meal fills is distinguishable under simulated protanopia, deuteranopia and tritanopia in both schemes, and fix the one pair that genuinely collapses today. Read the parent ticket's `## Implementation Plan` first — it carries the measured numbers, the calibrated floors, the retune candidate and the exact red-state demonstrations required. No open research questions remain.

**The defect this closes (measured, not assumed).** With a CIEDE2000 implementation validated against the published Sharma table, the shipped palette's worst pair per condition is:

| scheme | condition | worst pair | dE00 | floor |
|---|---|---|---|---|
| light | protanopia | lunch/dinner | **5.05** | 6.61 |
| light | deuteranopia | lunch/dinner | 6.33 | 6.04 |
| dark | tritanopia | lunch/legacy | 9.15 | 5.66 |

Light-scheme `lunch`/`dinner` is the real production collapse — 5.05 under protanopia, and only 0.29 above the floor for deuteranopia. The dark-scheme deuteranopia pair that motivated the parent epic measures 15.61, i.e. comfortably separable; do not go retuning the dark ramp chasing the ticket's original "dE 3.0-3.7" claim, which does not reproduce.

**The guard.** A new `#[test] fn meal_ribbon_colours_survive_colour_vision_deficiency()` in the inline `mod tests` of `nom-core/src/operation/mcp_handler.rs`, placed after `meal_ribbon_legacy_bucket_is_not_hue_only` (which ends at line 1449). It reads the declared hexes through the existing `light_dark_pair` helper (line 1191), forms all six pairs among breakfast/lunch/dinner/legacy per scheme, simulates each of the three deficiencies, and asserts CIEDE2000 separation against the floors. That is 36 comparisons. Failure messages must name the pair, the scheme, the condition, the measured value and the floor — the format style of `meal_ribbon_colours_meet_non_text_contrast` (line 1229).

**No exemption for the legacy bucket.** Every pair is judged on its declared hex, including `.seg-none`. Measured margins show this is achievable today, and judging the declared hex is what makes the acceptance criterion work: repainting `--meal-lunch` onto the legacy grey trips the guard instead of slipping past an exemption. Do not add a luminance-only rule for the legacy swatch — measured, its effective rendered fill (#6f6f76 light, #96969c dark) sits only 1.08-1.26:1 from the chromatic neighbours, so no such threshold can pass. Also assert, one line reusing `css_rule_body` (line 1245), that `.seg-none` still paints SVG through `url(#meal-none-hatch)`, so reverting TASK-65.1's texture fails this test too; say in the message that the non-colour cue is what went missing.

**Floors are derived, not invented.** Compute them in the test from the seven non-black Okabe-Ito/Wong (Nature Methods 2011) swatches pinned as data — orange `#E69F00`, sky blue `#56B4E9`, bluish green `#009E73`, yellow `#F0E442`, blue `#0072B2`, vermillion `#D55E00`, reddish purple `#CC79A7` — as the minimum pairwise simulated dE00 among them per condition. Same pipeline, so the calibration cannot drift from the metric. Planning measured those floors at protan 6.61 / deutan 6.04 / tritan 5.66; record them in the doc comment and cross-check the executor's own run reproduces them. Rationale for rejecting the borrowed numbers (PaletteGuard's published 9.79-13.38, Colorally's dE>=10, rgblind's CIE76 flags) is in the parent plan — they were computed under different conventions and would silently mean something else here.

**The fix.** Change exactly one shipped value: light-scheme `--meal-lunch` from `#7c3aed` to violet-700 `#6d28d9` (`nom-core/assets/weekly_progress_widget.html` line 30). Measured after the change: light minima become protan 8.28 / deutan 9.56 / tritan 14.31, normal-vision worst pair rises from 10.65 to 15.63, and contrast against the page improves from 5.70:1 to 7.11:1, so SC 1.4.11 gets safer rather than riskier. Nothing else moves — the dark scheme already passes, dinner stays put, and TASK-63's contrast guard and TASK-64's seam geometry are untouched because no width or stroke changes. Update the ramp rationale comment at lines 14-28 in the same commit so it does not describe a palette that no longer exists. If the screenshot review below disagrees with the arithmetic, alternates that also clear every floor are purple-700 `#7e22ce` and fuchsia-700 `#a21caf`.

**Red states are part of done.** Per the parent's acceptance wording, demonstrate and quote in `## Implementation Notes` (between `<!-- SECTION:NOTES:BEGIN/END -->`, following the `## Red-first evidence` heading style used by TASK-65.1): (1) write the guard first against the unmodified asset and capture the failure naming light lunch/dinner at 5.05 versus the 6.61 floor, then make it green with the hex change; (2) with the guard green, temporarily revert `.seg-none` to a flat `fill: var(--meal-none)` and confirm the structural assert fires, then revert that. Mutation-check the swap case too — set `--meal-lunch` light to a lightness adjacent to `--meal-none` and confirm the guard fails for the right reason. Revert every mutation; nothing may be committed red, because CI runs nextest and clippy over test code.

**Verification.** Full suite green (`cargo nextest run --all-features --workspace`, doctests, `cargo fmt --all --check`, `cargo clippy --all-targets --all-features --workspace -- -D warnings`, rustdoc with `-D warnings`). Then the screenshot playbook the repo already uses: throwaway `/tmp/widget-harness` host page iframing the widget at 320px, seeded DB, detached `serve http`, real `get_weekly_progress` payload, `agent-browser set media light` plus a Blink `feColorMatrix` deuteranopia filter on a wrapper, fresh subagents with at most four images each (the vLLM cap in AGENTS.md), teardown `rm -rf /tmp/nom-dev /tmp/widget-harness`. Record the measured before/after numbers in the notes.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A std-only test asserts all six fill-pair separations in both schemes under protanopia, deuteranopia and tritanopia, with thresholds derived in-test from the pinned Wong reference palette and provenance recorded in the doc comment.
- [ ] #2 The stated matrix space (linear light, clamped) and metric (CIEDE2000, D65) appear in the doc comments and match the implementation.
- [ ] #3 Including the legacy bucket in the judged set, the guard is green; swapping --meal-lunch light to a lightness adjacent to --meal-none makes it fail, and so does reverting .seg-none to a flat fill. Both red states are quoted in the implementation notes and reverted.
- [ ] #4 Light-scheme --meal-lunch is retuned so every condition clears its floor with margin, the CSS ramp comment is updated in the same commit, and meal_ribbon_colours_meet_non_text_contrast still passes.
- [ ] #5 Screenshot verification of the retuned light scheme (normal and deuteranopia-emulated, <=4 images per subagent prompt) is recorded in the notes, and the full CI mirror is green.
<!-- AC:END -->

## Comments

<!-- COMMENTS:BEGIN -->
created: 2026-09-07 19:27
---
Planning correction from TASK-65.2.1 research (verified against colorspacious cvd.py, coloraide filters/cvd.py, colour-science tests/test_delta_e.py, and a local re-run of /tmp/t652_verify.py). Three defects in this plan that would have produced red-but-correct code: (1) the Machado severity-1.0 deutan and tritan matrices are mis-transcribed - correct digits are deutan [[0.367322,0.860646,-0.227968],[0.280085,0.672501,0.047413],[-0.011820,0.042940,0.968881]] and tritan [[1.255528,-0.076749,-0.178779],[-0.078411,0.930809,0.147602],[0.004733,0.691367,0.303900]], matching colorspacious and coloraide digit-for-digit; protan was right. (2) There is no feColorMatrix / filter / dichromacy constant anywhere under nom-core/assets (weight_trend_widget.html is 371 lines of render code); the Blink constants remembered at "lines 243-248" live only in the throwaway spike harness /tmp/t65-spike/harness.html:91, so any parity assert must pin them as test data rather than cite shipped markup. (3) The Sharma CIEDE2000 table carried here is corrupt and does not reproduce (worst error 1.75): rows 1-6 use b* = -82.7485, not -82.7775, and four pairs are mis-partnered; the verified 33-row set with expected values is pinned in TASK-65.2.1's implementation plan, where two independent implementations reproduce every published value to 4.9e-5. Everything else measured here did reproduce exactly with the corrected matrices - light-scheme lunch/dinner protanopia 5.05 worst pair, dark worst 15.61, Okabe-Ito floors protan 6.61 / deutan 6.04 / tritan 5.66, gamma-vs-linear dE76 11.40 vs 24.99 - so the conclusions and the one-hex fix stand unchanged.
---
TASK-65.2.1 executed. One more correction to inherit, on top of the three already recorded here: the simulation SPACE behind this plan's separation measurements was wrong, so every dE figure below "conclusions ... stand unchanged" must be re-derived before it is used as a threshold.

The planning oracle (/tmp/t652_verify.py simulate()) applied the linear->XYZ matrix to gamma-encoded values, i.e. it simulated in gamma space while reporting linear. Tell: that pipeline moves neutral #808080 from L* 53.59 to L* 76.19, impossible for a matrix whose rows sum to 1 (neutrals must survive dichromacy simulation). Measured with the matrix genuinely in linear light and clamping after the multiply, verified against coloraide 8.12.1 (agreement ~1e-3) and reproduced by the shipped Rust primitives in nom-core/src/operation/mcp_handler.rs:

- motivating pair #a78bfa vs #71717a under deuteranopia: dE76 50.00 / dE00 25.38, not 24.99. The 24.99 figure is the GAMMA-space result (24.64 measured), so the plan's gamma-vs-linear comparison was measuring a bug, not a design choice.
- shipped light scheme worst pair: dE00 7.72 (protanopia, #7c3aed vs #a855f7), not 5.05.
- shipped dark scheme worst pair: dE00 14.86 (tritanopia, #7c3aed vs #71717a), not 15.61.
- Okabe-Ito calibration floors: protan 12.25, deutan 11.61, tritan 10.87, not 6.61 / 6.04 / 5.66.

Consequence for this plan's Step 2: the calibration ratio changes materially (smallest shipped separation 7.72 against an Okabe-Ito floor of 10.87 rather than 5.66 against 5.66-ish), so the "one hex must be retuned" conclusion is no longer established -- TASK-65.2.2 must re-derive whether any pair fails a defensible floor before touching --meal-lunch. The corrected machinery itself is done and merged: colour_math in mcp_handler.rs tests, with delta_e_2000 (coefficient 0.20, not the circulating 0.10), lab_from_hex, simulate_cvd and eight known-answer tests.
---
<!-- COMMENTS:END -->
