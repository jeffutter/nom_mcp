---
id: TASK-65.1
title: >-
  Encode the legacy meal-type bucket by texture, not hue alone, in the
  weekly-progress ribbon
status: Done
assignee:
  - '@ralph'
created_date: '2026-09-07 15:29'
updated_date: '2026-09-07 16:15'
labels:
  - task
  - planned
dependencies: []
parent_task_id: TASK-65
priority: medium
ordinal: 75100
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The dark-mode legacy/unrecognised meal-type bucket in nom-core/assets/weekly_progress_widget.html is the dimmest ribbon fill (#71717a = 3.71:1 vs --bg) and sits 1.92:1 from the muted legend label printed right beside it, so the swatch that flags unlabelled data recedes. TASK-65's spike proved this cannot be fixed by choosing a lighter gray: with --fg-muted at 7.11:1 vs the dark page background, a swatch would have to stay under 2.37:1 vs bg to hold 3:1 against its own label, which contradicts the 3:1 floor it must also clear. No hex satisfies both. In light mode it is worse: #8f8f99 renders at 3.20:1 vs white, a razor-thin pass, and the same 1.66:1 collision with the label.

Fix the actual defect instead: the legacy bucket's only cue is hue, so give it a non-colour cue — a 45-degree foreground-tinted hatch over the existing fill, mirrored on the legend swatch. Measured on rendered pixels at the shipped scale (spike harness at ~/.local/share/nom_mcp-spikes/t65), this lifts the legacy band's effective contrast against the page background to 6.02:1 dark / 4.99:1 light at dpr1 and 5.87:1 / 4.91:1 at dpr2 with no hex change at all, because the stripes move toward --fg, which is bright in dark mode and dark in light mode — a lightness bump can only ever help one half and always moves the fill toward the muted label text. Texture amplitude survives downscaling (rendered luminance sd 0.04-0.11 at dpr1-2 versus 0.00 for a flat fill), so the cue reaches non-retina hosts too. Two independent screenshot reviews (normal + Chromium deuteranopia emulation, both themes) ranked the 45-degree hatch first and second among seven candidates; hollow segments were rejected because they read as missing data (rendered mean equals the page background exactly: 1.00:1), vertical stripes because they alias into a single line inside the 2.3-CSS-px minimum sliver, and outline-only because the ring nearly consumes the sliver's fill and drifts toward "hollow".

Scope: the widget asset plus its std-only guard tests in nom-core/src/operation/mcp_handler.rs. Do not change any --meal-* hex value, do not touch MIN_SEG_WIDTH or the .meal-seg seam stroke, and keep proportion honest — the fill stays opaque beneath the hatch.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 .seg-none paints both surfaces with a non-colour cue: SVG segments through a pattern whose base rect is --meal-none, legend swatches through a repeating-linear-gradient whose perpendicular period matches the pattern within 0.1 CSS px at the shipped 300/320 scale
- [x] #2 Every --meal-* hex, MIN_SEG_WIDTH and the .meal-seg page-coloured seam stroke are unchanged; the fill stays opaque under the hatch so proportion stays honest
- [x] #3 Rendered pixels (not declared hexes) put the legacy band at >=4.5:1 vs the page in dark and >=4.0:1 in light at dpr 1, 2 and 3, with measurable texture amplitude at dpr1 and dpr2, verified with the saved spike measuring scripts
- [x] #4 New std-only guard meal_ribbon_legacy_bucket_is_not_hue_only is written red first, passes after, and fails again when either the pattern or the swatch mirror is removed
- [x] #5 Two fresh-subagent screenshot passes (max 4 images each, incl. Chromium deuteranopia emulation, both themes) judge the legacy bucket deliberate-and-present rather than artifact, hollow or missing
- [x] #6 Workspace suite green: nextest --all-features, doctests, clippy -D warnings, fmt --check, rustdoc -D warnings
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Direction (already decided by measurement — see the ticket description)

Ship the spike's variant D: keep every `--meal-*` hex exactly as it is, paint the legacy segment through an SVG hatch pattern whose base rect *is* `--meal-none`, and mirror the identical stripe geometry on the legend swatch. Do not re-open the lighter-gray question: no hex can clear 3:1 against both `--bg` and `--fg-muted` in either theme (dark needs <=2.37:1 vs bg to hold 3:1 against the label, light needs <=1.78:1), and hollow / vertical-stripe / outline-only variants were measured and reviewed out.

Reusable spike, including the harness, the measuring scripts and the four review sheets: `~/.local/share/nom_mcp-spikes/t65/` (README there documents the agent-browser commands). Nothing from it goes in the repo.

## 1. Widget asset — nom-core/assets/weekly_progress_widget.html

Line anchors from the current file (763 lines): `:root` vars 9-32, hue rules 102-109, `.meal-seg` 111-114, legend rules 168-196, `MIN_SEG_WIDTH` 341, `mealRibbon` 430-450, `caloriesChartSvg` 481-551 (svg built at 505-507, `mealRibbon` called at 540), `applyHostContext` 697-711.

a. **Define the paint server inside the chart svg.** In `caloriesChartSvg`, right after the `<svg class="chart">` element is created and before the day loop, append once:
   `defs` > `pattern#meal-none-hatch` with `patternUnits="userSpaceOnUse" width="1.6" height="1.6" patternTransform="rotate(45)"`, holding a full-tile rect `.meal-none-hatch-base` (1.6 x 1.6) and a stripe rect `.meal-none-hatch-stripe` (0.66 x 1.6).
   Put it in the consuming svg rather than a static hidden svg: the spike confirmed a cross-svg `fill: url(#id)` reference does paint in Chromium 148, but defining it locally deletes the cross-document-reference risk (WebKit hosts) for three lines of code, and inline defs keep the asset self-contained, which the resource tests already assert.
b. **Two CSS rules beside the hue rules** (so everything that paints a meal type stays in one place):
   `.meal-none-hatch-base { fill: var(--meal-none); }`
   `.meal-none-hatch-stripe { fill: color-mix(in srgb, var(--fg) 65%, transparent); }`
   Deriving the stripe from `--fg` is the whole trick: one declaration improves both halves (in dark mode stripes are bright, in light mode dark) and keeps following a host that overrides `--fg` through `applyHostContext`. Optional refinement — reviewers found the dark-mode hatch slightly weaker than light — is a per-theme alpha via `--meal-none-stripe: light-dark(color-mix(...60%...), color-mix(...80%...))`; verify with `agent-browser eval getComputedStyle(...).fill` that the nesting actually resolves before depending on it, otherwise ship the single 65%.
c. **`.seg-none` (line 109)** becomes `fill: url(#meal-none-hatch); background: var(--meal-none);`. Both properties must remain: `meal_ribbon_ships_a_static_colour_key` asserts it, correctly, because the HTML swatch ignores `fill` and the SVG rect ignores `background`.
d. **Swatch mirror**, immediately after: `.meal-legend-swatch.seg-none { background-image: repeating-linear-gradient(45deg, <same stripe colour> 0 0.66px, transparent 0.66px 1.5px); }`.
   Pitch parity is the trap worth a comment and a test: the SVG period is expressed in viewBox units and rotated, so its perpendicular period is `1.6 * 300/320 = 1.5` CSS px at the shipped layout, while gradient stops are already CSS px. Copying the number 1.6 into the gradient silently ships two different textures.
e. **Rewrite the comments that go false**: the hue-ramp rationale (19-26) gains "gray carries a hatch because hue was its only cue (WCAG 1.4.1)" plus the measured before/after separation; the load-bearing-seam comment (90-101) notes the hatch lives *inside* the fill and the page-coloured seam is untouched; the legend comment (~168-171) saying "Segments carry their meaning in hue alone" is no longer true.

## 2. Guards — nom-core/src/operation/mcp_handler.rs, inline `mod tests`, std-only

New `meal_ribbon_legacy_bucket_is_not_hue_only` next to the three existing ribbon tests (1228-1333), reusing `css_rule_body` (1248) and the numeric-parsing style of `meal_ribbon_segments_survive_their_seam_stroke`:
- `.seg-none` names a `url(#` paint server in `fill:` and still sets `background:`.
- The pattern exists with `patternUnits="userSpaceOnUse"` and a 45-degree `patternTransform`; its base class rule resolves to `var(--meal-none)`; the stripe rule derives from `var(--fg)`.
- The swatch mirror declares `background-image: repeating-linear-gradient` whose perpendicular period agrees with the pattern's `width` attribute within 0.1 CSS px after the 300/320 scale — one numeric invariant, not two substrings that can drift.
- The defs builder is defined *and* called (count its name >= 2 occurrences), the same way the colour-key test proves `mealLegendHtml(` both exists and is reached from `render()`.
Write it red first against the unmodified asset and quote the failure text in the notes. Expect zero churn in the other guards: contrast (base fill unchanged), seam stroke (untouched), colour key (both props still set), self-containment.

## 3. Verification — throwaway, commit nothing

Playbook from TASK-62.5/63/64, unchanged: `seed_data --path /tmp/nom-dev/nom.db`, `serve http --port 8000` detached, real `get_weekly_progress` payload pulled over `/mcp` (capture `mcp-session-id`), host page iframing the asset at 320px that answers `ui/initialize` with `hostContext.theme` and posts `ui/notifications/tool-result`. Edit the fixture JSON so one day carries a 50-cal legacy bucket and another a 500-cal one.
- Capture with `agent-browser set viewport 340 <h> <scale>` at scale 1, 2 and 3, `set media light|dark`. Deuteranopia: wrap the iframe in `filter: url(#vd-filter)` with Blink's own matrix (see the spike harness) — agent-browser does not expose `Emulation.setEmulatedVisionDeficiency`.
- Assert on pixels, not vibes, with the saved scripts: rendered mean of the legacy band must reach >=4.5:1 (dark) and >=4.0:1 (light) against the page at every dpr, luminance sd must stay > 0.02 at dpr1 and dpr2 (texture actually resolved, not averaged away), and the breakfast/lunch/dinner bands must be pixel-identical to before the change.
- Two fresh-subagent screenshot passes, never more than 4 images in one prompt (AGENTS.md, vLLM cap): pass 1 = dpr3 light+dark, normal+deutan, real widget; pass 2 = dpr1 and dpr2 versus the pre-change baseline. Ask exactly: does the legacy bucket read as a deliberate present segment (not artifact, not hollow, not missing data), is its swatch clearly its own object beside the muted label, do proportions still read true?
- Teardown: `rm -rf /tmp/nom-dev /tmp/widget-harness`.

## 4. Done means

`cargo nextest run --all-features --workspace`, `cargo test --doc`, `cargo clippy --all-targets --all-features --workspace -- -D warnings`, `cargo fmt --all --check`, rustdoc with `-D warnings` all clean. No schema, operation or registry change — this is purely the asset plus its guards, so no new Operation and no CONTEXT.md term is needed.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
## Change
- Asset (nom-core/assets/weekly_progress_widget.html): .seg-none now paints SVG segments through an inline pattern paint server (#meal-none-hatch, patternUnits=userSpaceOnUse width=1.6 rotate(45), base rect = untouched --meal-none, stripe = color-mix(in srgb, var(--fg) 65%, transparent)) while keeping background: var(--meal-none) for the HTML legend swatch; .meal-legend-swatch.seg-none mirrors the texture with repeating-linear-gradient(45deg, ... 0 0.66px, transparent 0.66px 1.5px) -- 1.5 CSS px = the pattern's rendered perpendicular period at the shipped 300/320 scale (the numbers differ from the pattern's raw 1.6 on purpose; comment says so). Defs built by mealNoneHatchDefs() appended inside caloriesChartSvg (local url(#id) ref, no cross-svg reference). No --meal-* hex, MIN_SEG_WIDTH or .meal-seam stroke touched; false comments (hue-alone claims) rewritten.
- Guard: meal_ribbon_legacy_bucket_is_not_hue_only (mcp_handler.rs inline tests, std-only) asserts the fill url(#...) + background pair, patternUnits/rotate(45), base resolves to var(--meal-none), stripe derives from var(--fg), builder defined+called (>=2 occurrences), swatch mirror declares a 45deg repeating gradient whose max px stop equals pattern width*300/320 within 0.1 CSS px. New helpers xml_number_attribute/css_px_values.

## Red-first evidence
Before asset change: 'panicked ... .seg-none must paint SVG segments through a hatch pattern (fill: url(#...)) and keep a background for the HTML legend swatch; found fill: var(--meal-none); background: var(--meal-none)'. Green after. Mutation checks (each reverted): reverting .seg-none fill -> fails naming the missing url(#..); swapping repeating-linear-gradient->linear-gradient -> fails the swatch-mirror assert; drifting the gradient period 1.5->1.6px -> fails 'swatch stripes repeat every 1.6 CSS px but the SVG pattern repeats every 1.5 CSS px'. One helper bug caught by red-first discipline: reversed digit-run collection parsed 0.66 as 66.0 (fixed by reversing back).

## Pixel verification (throwaway harness /tmp/widget-harness, deleted after; nothing committed)
seed_data -> serve http :8000 -> real get_weekly_progress payload over /mcp (mcp-session-id capture); legacy buckets created honestly in the dev DB (two seeded meals set meal_type=NULL with totals 50 cal Tue 2026-09-01 and 500 cal Fri 2026-09-04, so daily totals stay consistent). Host page iframed the asset at 320px, answered ui/initialize with hostContext.theme, posted tool-result; geometry probed via getBoundingClientRect of every segment/swatch, measured with ImageMagick pixel crops x dpr.
Rendered legacy-band mean vs page: dark 5.43/5.95/6.10:1 (dpr1/2/3, sliver band0; wide band up to 6.10) -- all >=4.5; light 4.45/4.99/5.06:1 -- all >=4.0. Baseline same crops: 3.71:1 dark flat (lumSD 0.0000), 3.20:1 light flat. Texture amplitude (luminance sd) patched: 0.095-0.178 dark, 0.065-0.098 light at every dpr incl. 1 -- all >0.02. Proportion honest: both bands' rendered means are gray-based, never page-coloured.
Byte-identity: every breakfast/lunch/dinner segment and labelled swatch crop, patched vs baseline at dpr3, magick compare AE=0 in BOTH themes (15 segments + 3 swatches each).

## Screenshot reviews (two fresh subagents, 4 images each, text back only)
Pass 1 (patched dpr3: dark/light normal + Blink deuteranopia matrix both themes): SHIP x4. Legacy band reads deliberate-present in Tue/Fri columns; under deutan it pops as the only achromatic segment ('effectively colorblind-proof'); swatch clearly its own object beside the muted label; proportions honest. Pass 2 (before/after composites dpr1+dpr2 both themes, top=before bottom=after): SHIP x4; texture resolved on the wide band at dpr1; no accidental change to violet segments/bars/swatches. Both passes independently flagged one non-blocking soft spot: the ~2.5-CSS-px legacy sliver at dpr1 undersamples the hatch into mottled gray (worst light theme) -- still judged present-and-deliberate, filed as a follow-up ticket rather than tuned blind here.
Full gate: nextest 383/383, doctests, clippy -D warnings all-targets, fmt --check, rustdoc -D warnings --document-private-items --examples all clean.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The legacy/unrecognised meal-type bucket stops being hue-only: its ribbon segments now paint through an inline SVG 45-degree hatch pattern (#meal-none-hatch, base rect = the untouched --meal-none fill so proportion stays honest, stripe tinted from --fg at 65% so one declaration lifts both themes), and the '(none)' legend swatch mirrors the identical texture via repeating-linear-gradient whose 1.5-CSS-px period is the pattern's rendered pitch at the shipped 300/320 scale - no --meal-* hex, MIN_SEG_WIDTH or seam-stroke change. Rendered pixels (real get_weekly_progress payload over /mcp, seeded DB with honest NULL-meal_type buckets of 50 and 500 cal): legacy band 5.43-6.10:1 vs page in dark and 4.45-5.06:1 in light at dpr 1/2/3 (baseline flat gray measured 3.71/3.20), luminance sd 0.065-0.178 everywhere incl. dpr1; every breakfast/lunch/dinner segment and labelled swatch byte-identical to baseline (AE=0, both themes). New std-only guard meal_ribbon_legacy_bucket_is_not_hue_only written red first, mutation-verified on all three failure modes; two fresh-subagent screenshot passes (dpr3 normal+deuteranopia, before/after dpr1-2 composites) returned SHIP 8/8. fmt/clippy/nextest 383/doctests/rustdoc all clean; harness removed. Reviewer-flagged dpr1 sliver mottling filed as TASK-66 (non-blocking).
<!-- SECTION:FINAL_SUMMARY:END -->
