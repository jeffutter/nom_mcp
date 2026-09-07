---
id: TASK-65.2.2
title: >-
  TASK-65.2.2 Guard every meal-ribbon fill pair under simulated colour vision
  deficiency and retune the collapsing light-scheme hex
status: Done
assignee: []
created_date: '2026-09-07 17:52'
updated_date: '2026-09-07 22:29'
labels:
  - planned
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

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
# Plan — TASK-65.2.2

One guard test, one shipped hex, three red-state demonstrations, one screenshot pass. No sub-tickets: the guard, the retune, the comment rewrite and the visual check are one tightly-coupled increment that cannot ship apart (see §7).

Everything below was re-measured through the **shipped** primitives (`colour_math::simulate_cvd` / `delta_e_2000`, extracted verbatim into `/tmp/t6522/plan_sweep.rs`, reproducible with `rustc --edition 2021 -O /tmp/t6522/plan_sweep.rs -o /tmp/t6522/plan_sweep && /tmp/t6522/plan_sweep`). It reproduces every figure in TASK-65.2.1's handoff note exactly.

## Step 0 — restate the inherited numbers before writing anything

Three places carry measurements computed in the mislabelled (gamma-space) simulation that TASK-65.2.1 disproved. Use these, and say so in the notes so nobody thinks the ticket was sloppy rather than superseded:

| claim | stale text says | authoritative | where the stale number lives |
|---|---|---|---|
| light lunch/dinner protanopia | 5.05 vs floor 6.61 | **7.72 vs floor 12.25** | this ticket's description ¶2 + AC discussion; parent TASK-65.2 plan table |
| light lunch/dinner deuteranopia | 6.33 vs 6.04 (clears by 0.29) | **10.33 vs floor 11.61 → also FAILS** | same |
| dark worst pair | tritan 9.15 vs 5.66 | **tritan 14.86 vs 10.87 (passes)** | same |
| Wong floors | protan 6.61 / deutan 6.04 / tritan 5.66 | **protan 12.25 / deutan 11.61 / tritan 10.87** | same |
| retune result for `#6d28d9` | protan 8.28 / deutan 9.56 / tritan 14.31 | **protan 13.13 / deutan 15.60 / tritan 18.77** | same |
| effective legacy fill, tighter reading | dark lunch/legacy tritan 5.81 vs 5.66 | **9.93 vs 10.87** | parent plan §3 |

Corrected full matrix (worst of the six pairs per scheme × condition, declared hexes):

| scheme | normal | protanopia | deuteranopia | tritanopia |
|---|---|---|---|---|
| light (shipped) | 10.65 lunch/dinner | **7.72 lunch/dinner — FAIL** | **10.33 lunch/dinner — FAIL** | 18.77 dinner/none |
| dark (shipped) | 22.20 breakfast/lunch | 17.34 breakfast/lunch | 18.71 breakfast/lunch | 14.86 breakfast/none |

Two consequences the old numbers hid: the defect is **two** failing conditions, not one, and the second-worst light pair under protanopia is 23.50 — i.e. the palette has enormous headroom everywhere except `lunch`/`dinner`. The fix really is one pair.

Floors, derived from the seven non-black Wong swatches through the same pipeline: protan **12.2458** (`#0072B2`/`#CC79A7`), deutan **11.6051** (`#E69F00`/`#F0E442`), tritan **10.8686** (`#E69F00`/`#CC79A7`).

## Step 1 — the guard, written red first

Insert between line 1445 (the closing brace of `meal_ribbon_legacy_bucket_is_not_hue_only`) and the `/// Same as test_dispatch_read_resource_goal_progress_widget above` doc comment at line 1447 of `nom-core/src/operation/mcp_handler.rs`:

1. One helper, placed with the other CSS/colour helpers' style (it belongs next to the test that uses it, not in `colour_math`, which is finished and pinned):

   ```rust
   /// Minimum pairwise CIEDE2000 separation among `hexes` after simulating `kind`
   /// (`None` = unsimulated), with the indices of the pair that set it.
   fn min_separation(hexes: &[&str], kind: Option<ColourVisionDeficiency>) -> (f64, usize, usize)
   ```

   Both the floors and the ribbon measurement must go through this one function. That is what makes "the calibration cannot drift from the metric" structural rather than aspirational.

2. `#[test] fn meal_ribbon_colours_survive_colour_vision_deficiency()`:
   - Structural assert first, one statement reusing `css_rule_body(WEEKLY_PROGRESS_WIDGET_HTML, ".seg-none")` (line 1244): the rule body must contain both `url(#meal-none-hatch)` and `background:`. Message must name the missing thing as the *non-colour cue* (WCAG 1.4.1), because that is why a colour guard cares.
   - `const WONG: [&str; 7] = ["#E69F00", "#56B4E9", "#009E73", "#F0E442", "#0072B2", "#D55E00", "#CC79A7"];` with the Wong/Okabe-Ito citation (Nature Methods 2011) — these are the canonical seven non-black swatches.
   - Per deficiency kind: derive the floor with `min_separation(&WONG, Some(kind))`, then **pin it**: `assert!((floor - documented).abs() <= 0.05)` against 12.25 / 11.61 / 10.87, message saying a floor move means the metric moved and must be recalibrated deliberately, never silently. Deriving keeps calibration tied to the pipeline; pinning turns a future edit to `colour_math` into a loud failure instead of a goalpost shift.
   - Then for each scheme, read the four fills with `light_dark_pair(WEEKLY_PROGRESS_WIDGET_HTML, name)` for `meal-breakfast`, `meal-lunch`, `meal-dinner`, `meal-none` (order fixed, so pair names come from an index array) and assert all six pairs ≥ floor. **36 comparisons, legacy bucket included, judged on the declared hex.** No exemption, no luminance-only fallback.
   - Failure messages follow `meal_ribbon_colours_meet_non_text_contrast` (line 1229): pair, scheme, condition, measured dE00, floor, and how the floor was derived. Target format: `--meal-lunch/--meal-dinner light protanopia: dE00 {got:.2}, below the {floor:.2} floor (minimum pairwise simulated separation of the seven Wong reference swatches)`.
   - Do **not** gate unsimulated distances against these floors. The floors are calibrated *under simulation*; comparing raw Lab distances to them mixes conventions. Normal-vision separability is already covered by the contrast guard.

Doc comment must state, compactly: metric (CIEDE2000, kL=kC=kH=1, D65 Lab), simulation (Machado/Oliveira/Fernandes 2009 severity 1.0 = full dichromacy, the conservative worst case; anomalous trichromats sit between normal and it), matrix space (linear-light sRGB, clamped to [0,1] after the multiply, as Blink does) and that thresholds are only meaningful together with that space, the three pinned floors with their binding Wong pairs, the derived-not-borrowed rationale (PaletteGuard's published 13.38/12.19/9.79 and Colorally's dE≥10 were computed under other conventions; the label hides three different measurements), and the known tighter reading that is deliberately ungated: substituting the hatch-blended *effective* legacy fill (`#6f6f76` light / `#96969c` dark, blend weight 0.66/1.6 × 0.65 = 0.268125, gamma-space compositing) drops dark `lunch`/legacy to **9.93** under tritanopia against the 10.87 floor — ungated because there the texture, not the hue, is the cue being relied on. Add the honest model caveat: the Machado model is less validated for tritanopia (Brettel et al. 1997 / Roy & Foster 2012 are the accurate alternatives); it is kept because it is the model Chromium emulates, so the guard and the browser agree.

Also state why no luminance-only rule can substitute for judging the legacy hex: the effective fill sits **1.26:1** from light dinner and **1.08:1** from dark lunch, so any minimum-lightness-separation threshold fails immediately. Those two ratios reproduce TASK-65.1's recorded figures, and the blend model reproduces its rendered contrasts too (predicted 4.99:1 light / 6.09:1 dark against the recorded 4.99:1 / 6.02:1), which is why they can be quoted without a browser.

Optional but recommended, one comment-only edit: the `t` expression in `delta_e_2000` (line 1838) carries Sharma eq. (15)'s *phase-shifted* cosines `+0.32·cos(3h̄+6°) − 0.20·cos(4h̄−63°)`. Note that in two lines — the widely-circulated unshifted form with `0.10` fails 27 of the 34 published rows, so it must not be "simplified" back.

Run it against the **unmodified** asset and capture the red. Expected (quote verbatim in the notes; your numbers should match to the second decimal):

```
--meal-lunch/--meal-dinner light protanopia: dE00 7.72, below the 12.25 floor …
--meal-lunch/--meal-dinner light deuteranopia: dE00 10.33, below the 11.61 floor …
```

Commit the guard alone once it is green-after-Step-2, or hold both edits until Step 3 — either is fine, but nothing may be committed red.

## Step 2 — the one-hex retune, and which hex

Measured sweep of every plausible replacement for light `--meal-lunch` (all six pairs, four fills, floors 12.25 / 11.61 / 10.87; `N` = unsimulated worst pair; contrast is against `--bg` light `#ffffff`, so the TASK-63 guard's 3:1 floor is nowhere near threatened):

| hex | Tailwind | contrast | N | protan | deutan | tritan | min margin |
|---|---|---|---|---|---|---|---|
| `#7c3aed` | violet-600 (shipped) | 5.70 | 10.65 | 7.72 | 10.33 | 18.77 | **−4.53** |
| `#7e22ce` | purple-700 | 6.98 | 14.99 | 14.23 | 14.72 | 15.22 | **+1.98** |
| `#5b21b6` | violet-800 | 8.98 | 13.58 | 14.92 | 13.71 | 17.79 | +2.10 |
| `#6b21a8` | purple-800 | 8.72 | 12.92 | 13.43 | 13.48 | 12.83 | +1.18 |
| `#a21caf` | fuchsia-700 | 6.32 | 15.29 | 16.26 | 12.66 | 18.77 | +1.05 |
| `#6d28d9` | violet-700 | 7.10 | 15.63 | 13.13 | 15.60 | 18.77 | +0.88 |
| `#86198f` | fuchsia-800 | 8.24 | 14.83 | 11.29 | 14.25 | 18.77 | −0.96 |
| `#9333ea` | purple-600 | 5.38 | 8.76 | 7.68 | 8.06 | 8.49 | −4.57 |
| `#4c1d95` | violet-900 | 10.95 | 8.57 | 9.28 | 8.42 | 13.15 | −3.19 |
| `#c026d3` | fuchsia-600 | 4.71 | 9.45 | 8.70 | 4.80 | 16.27 | −6.81 |
| `#8b5cf6` | violet-500 | 4.23 | 4.99 | 0.35 | 2.66 | 14.17 | −11.90 |

**Ship purple-700 `#7e22ce`.** The ticket and the parent plan name violet-700 `#6d28d9`; it clears protanopia by **0.88**, which is 7% over the floor — thin enough that AC #4's "with margin" is a stretch, and thin enough that any later palette nudge trips the guard for reasons nobody will remember. Purple-700 doubles the weakest margin (+1.98) while giving up almost nothing: unsimulated worst pair 14.99 vs 15.63, contrast 6.98:1 vs 7.10:1. Violet-800 buys a further +0.12 of margin and costs 2.05 of normal-vision separation plus a much darker mid-ramp, so purple-700 is the knee of the curve. This is a pre-authorised choice, not an invention: the parent plan lists purple-700 first among "alternates if review disagrees".

With `#7e22ce` the light minima are protan 14.23 / deutan 14.72 / tritan 15.22, and `lunch`/`dinner` stays the binding pair in all three conditions (next-worst 18.25–19.78). The decision is convention-robust too: re-simulated gamma-space (the other camp's convention), the binding pair moves from 6.85/7.58/7.70 shipped to 13.39/12.21/12.88 after the change, at or above PaletteGuard's published 13.38/12.19/9.79 — while violet-700 lands at 12.16/12.52/11.76, under their protanopia figure. Quote that as corroboration, not as the reason: our own convention is the gate.

Record the whole table in Implementation Notes. If the screenshot pass (§5) rejects purple-700 on appearance grounds, fall back to `#6d28d9` (green, +0.88 protan) or `#a21caf` (green, +1.05 deutan) and say which and why — do not invent a fourth candidate without re-running the sweep.

Change exactly one value: `nom-core/assets/weekly_progress_widget.html:30`, `--meal-lunch: light-dark(#7c3aed, #a78bfa)` → `light-dark(#7e22ce, #a78bfa)`. Dark half untouched, dinner untouched, no geometry, so TASK-63's contrast guard and TASK-64's seam guard stay green by construction.

Same commit: rewrite the ramp rationale comment at lines 17–27 so it describes the palette that shipped. It currently claims "a violet lightness ramp (violet-950 / -600 / purple-500) broken once at dinner, whose extra magenta rotation is what stops it fusing with lunch" — with purple-700 in the middle the single hue break moves to **breakfast/lunch**, and the sentence must say so and give the measured reason. Requirements for the new text: name the three hues by role and value, state where the hue break now sits and that the ramp is still monotonic in lightness, keep the existing sentence pointing at the hatch and WCAG 1.4.1, keep the sentence pointing at `meal_ribbon_colours_meet_non_text_contrast` for the 3:1 rule, and add one clause naming `meal_ribbon_colours_survive_colour_vision_deficiency` as the reason the mid-ramp value may not be nudged lighter again. Keep it prose, not a data dump — the numbers belong in the test's doc comment.

## Step 3 — mutation checks (three red states, all reverted, nothing committed red)

CI runs nextest and clippy over test code, so every mutation is temporary. Back the asset up first (`cp nom-core/assets/weekly_progress_widget.html /tmp/widget-backup.html`) and confirm `git diff --stat` is empty for the restored files afterwards.

1. **Pre-fix red** — already captured in Step 1 (guard against the unmodified asset). This is the primary demonstration.
2. **Legacy-grey repaint** — with the guard green, set light `--meal-lunch` to `#8f8f99` (exactly `--meal-none`). Expect the separation assert to fire on `lunch`/`none` at dE00 ≈ 0.00 against 12.25. A softer variant worth one extra run: `#86868f` (a lightness adjacent to legacy) fires at 3.20 — proof the guard bites on separation, not on string equality. Restore.
3. **Texture revert** — with the guard green, temporarily reduce `.seg-none` to a flat `fill: var(--meal-none)`. Expect the *structural* assert in the new test to fire; note in the notes that `meal_ribbon_legacy_bucket_is_not_hue_only` fires too, which is correct (two guards, one invariant, different reasons). Restore.

Quote each message verbatim in `## Red-first evidence`, following TASK-65.1's notes formatting, each followed by "reverted".

## Step 4 — screenshot verification of the retuned light scheme

Prior tickets ran this ad hoc; the exact commands were never recorded and `/tmp/widget-harness` did not survive, so the recipe is spelled out here. `/tmp/t65-spike/harness.html` survives but **reconstructs** the ribbon from copied constants — it is not the shipped file, so do not cite it as evidence that the shipped asset renders correctly.

Steps 1–3 below were executed end-to-end while planning (`/tmp/t6522/validate_harness.sh`, log `/tmp/t6522/validate.log`) and work; the corrections they forced are already folded in.

1. `mkdir -p /tmp/nom-dev /tmp/widget-harness`; copy the real widget: `cp nom-core/assets/weekly_progress_widget.html /tmp/widget-harness/weekly.html`.
2. Seed a throwaway DB with the purpose-built op — it takes a **required `--path`**, refuses the default path, and seeds goals, foods, 18 meals over 7 days, weights, and widget display:
   ```sh
   NOM_MCP_DB_PATH=/tmp/nom-dev/db.sqlite ./target/debug/nom-mcp seed_data --path /tmp/nom-dev/db.sqlite
   ```
   `seed_data` derives types from wall-clock hours (`MealType::from_local_hour`), so it emits only `breakfast`/`lunch`/`dinner`. The legacy bucket is **not** an unknown string: `migration_v2.sql:5` adds `meal_type TEXT CHECK (meal_type IN ('breakfast','lunch','dinner'))`, so `'snack'` is rejected by the CHECK and the legacy bucket is a row whose `meal_type` is **NULL** (pre-v2 data; NULL passes the check). Insert one, **before** any server holds the DB open (advisory lock probe, `lock_probe.rs`); there is no `sqlite3` binary in the dev shell, but python3 ships the module:
   ```sh
   python3 - <<'PY'
   import sqlite3, datetime
   d = datetime.date.today()
   c = sqlite3.connect("/tmp/nom-dev/db.sqlite")
   c.execute("INSERT INTO meals (id, logged_at, logged_date, total_calories, total_protein_g,"
             " total_carbs_g, total_fat_g, total_fiber_g, meal_type)"
             " VALUES (9001, ?, ?, 420, 20, 40, 15, 4, NULL)",
             (f"{d.isoformat()}T16:30:00Z", d.isoformat()))
   c.commit(); print(c.execute("select meal_type, count(*) from meals group by meal_type").fetchall())
   PY
   ```
   Verified: that row arrives in the payload as `(None, 420.0)` inside today's `by_meal_type`, which is exactly what `mealSegClass` maps to `seg-none`.
3. Capture a real `get_weekly_progress` payload. **There is no REST route for it** — `GetWeeklyProgress::surfaces()` is `Surfaces::MCP` (`weekly/mod.rs:284`), so `POST /api/get_weekly_progress` 404s. Pipe JSON-RPC into the stdio transport instead; its `result` is already the envelope `extractWeeklyProgress` wants, so write `content[0].text` straight to a file:
   ```sh
   { printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2026-01-26","capabilities":{},"clientInfo":{"name":"harness","version":"0"}}}'
     printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}'
     printf '%s\n' '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"get_weekly_progress","arguments":{}}}'
     sleep 8; } | NOM_MCP_DB_PATH=/tmp/nom-dev/db.sqlite ./target/debug/nom-mcp serve stdio 2>/dev/null \
     | python3 -c "import json,sys; [open('/tmp/widget-harness/payload.json','w').write(json.loads(l)['result']['content'][0]['text']) for l in sys.stdin if '\"id\": 2' in l.replace(':2',': 2')]"
   ```
   (Simpler if that one-liner fights you: save the raw stream and pick the line with `"id":2` in python.) Measured payload ≈3.7 KB; `daily_totals` sits under `nutrients`, which the host need not care about — it pastes the text through untouched.
4. Write `/tmp/widget-harness/host.html`: iframe `weekly.html` at 320px inside a wrapper `<div>`, implementing the three-message handshake the widget actually speaks — answer the `ui/initialize` **request** with `{hostContext:{theme:"light"}}` (the widget sets `documentElement.style.colorScheme` from it, asset lines 742–755), then send the `ui/notifications/tool-result` **notification** whose params are the captured result object verbatim, `{content:[{type:"text",text:<payload.json contents as a string>}]}` (`extractWeeklyProgress`, asset line 727, reads `content[0].text` and JSON-parses it). Add the CVD emulation to the wrapper div, using Blink's rounded deuteranopia triple and **`color-interpolation-filters="linearRGB"`** so the filter runs in linear light like the guard — copy the working markup from `/tmp/t65-spike/harness.html:89-92`, which already carries the right attribute; toggle it with a query param or a second host page.
5. Serve it (`python3 -m http.server 8199 -d /tmp/widget-harness &`) and drive `agent-browser` (both subcommands verified present): `open http://localhost:8199/host.html`, `set media light`, `screenshot /tmp/widget-harness/light-normal.png`, then the filtered variant to `light-deutan.png`. Crop to the ribbon band (`crop rect=x,y,w,h`) before showing anything to a model.
6. Review with **fresh subagents at ≤4 images each** (the vLLM cap in AGENTS.md): one agent gets light normal + light deuteranopia of the retuned scheme, another gets the same pair for the pre-change asset (`/tmp/widget-backup.html`, screenshotted in the same harness) so the comparison is honest. Ask specifically: can you tell lunch from dinner as separate bands, in both schemes, with and without the filter; does the legacy hatch still read as texture rather than a fourth hue.
7. Teardown: kill the `python3 -m http.server` and any stray `nom-mcp`, then `rm -rf /tmp/nom-dev /tmp/widget-harness`. No long-lived `serve http` process is involved once the payload has been captured over stdio.

If the emulation route fights you, the fallback is not "skip it": render the *simulated* colours by computing `simulate_cvd` on each fill and screenshotting a palette built from those hexes, and record that you substituted arithmetic for the browser and why.

## Step 5 — gate, notes, bookkeeping

```sh
nix develop .#ci -c cargo fmt --all
nix develop .#ci -c cargo fmt --all --check
nix develop .#ci -c cargo clippy --all-targets --all-features --workspace -- -D warnings
nix develop .#ci -c cargo nextest run --all-features --workspace     # 391 → 392 passed
nix develop .#ci -c cargo test --doc --all-features --workspace
RUSTDOCFLAGS="-D warnings" nix develop .#ci -c cargo doc --no-deps --document-private-items --all-features --workspace --examples
```

Float assertions use tolerance form (`(a-b).abs() < eps`) — the repo has no `clippy.toml` and no `[lints]` section, and `float_cmp` isn't enabled by default; don't add `#[allow]`. No new dependencies. `WEEKLY_PROGRESS_WIDGET_HTML` is a `include_str!` const, so the test reads the shipped file, not a fixture.

Implementation Notes sections, matching TASK-65.1's shape: `## Change`, `## Deviations from the plan` (the hex choice is the expected one — defend it with the §2 table), `## Red-first evidence` (three quotes, each reverted), `## Measured before/after` (both matrices, the floors with their binding Wong pairs, the effective-fill caveat figures, the gamma-space corroboration), `## Handoff to TASK-65.2` (AC #4 discharged: the shipped palette clears every floor with margin, and the ramp comment describes the palette that shipped).

Then map ACs to evidence: #1/#2 → the test and its doc comment; #3 → the two mutation quotes; #4 → the diff plus `meal_ribbon_colours_meet_non_text_contrast` still passing at 6.98:1; #5 → the screenshots and the CI mirror. Update the parent TASK-65.2 with a comment carrying the corrected numbers if it still shows the stale ones (its plan and comment #1 do).

## Risks, and what is explicitly out of scope

- **Stale numbers are the main hazard.** Six figures in this ticket and its parent are wrong; restate them (Step 0) rather than quietly diverging, or the reviewer will think the guard contradicts its own ticket.
- **Thin margins are by design, not fragility to negotiate away.** If a later ticket trips the guard, that is the guard working. Do not soften it by averaging pairs, dropping the legacy bucket, or adding a tolerance parameter — a configuration knob here would be a decision the plan already made.
- Out of scope, unchanged from the parent: per-user CVD configuration, APCA, retuning the dark scheme (it passes everywhere), milder-severity interpolation, gating the hatch-blended effective fill, and any change to `colour_math` beyond the optional two-line comment.
- TASK-66 (hatch undersampling) touches the same pattern block; if it lands first, re-check that the stripe width and alpha still imply blend weight 0.268125 before re-quoting the effective-fill figures.
- **No sub-tickets.** Guard + hex + comment + red-states + screenshots are one increment: the guard is red without the hex, the comment lies without the hex, and the screenshots verify the hex. Splitting them would create tickets that cannot ship independently, which is the criterion against.
<!-- SECTION:PLAN:END -->

## Comments

<!-- COMMENTS:BEGIN -->
created: 2026-09-07 19:27
---
Planning correction from TASK-65.2.1 research (verified against colorspacious cvd.py, coloraide filters/cvd.py, colour-science tests/test_delta_e.py, and a local re-run of /tmp/t652_verify.py). Three defects in this plan that would have produced red-but-correct code: (1) the Machado severity-1.0 deutan and tritan matrices are mis-transcribed - correct digits are deutan [[0.367322,0.860646,-0.227968],[0.280085,0.672501,0.047413],[-0.011820,0.042940,0.968881]] and tritan [[1.255528,-0.076749,-0.178779],[-0.078411,0.930809,0.147602],[0.004733,0.691367,0.303900]], matching colorspacious and coloraide digit-for-digit; protan was right. (2) There is no feColorMatrix / filter / dichromacy constant anywhere under nom-core/assets (weight_trend_widget.html is 371 lines of render code); the Blink constants remembered at "lines 243-248" live only in the throwaway spike harness /tmp/t65-spike/harness.html:91, so any parity assert must pin them as test data rather than cite shipped markup. (3) The Sharma CIEDE2000 table carried here is corrupt and does not reproduce (worst error 1.75): rows 1-6 use b* = -82.7485, not -82.7775, and four pairs are mis-partnered; the verified 33-row set with expected values is pinned in TASK-65.2.1's implementation plan, where two independent implementations reproduce every published value to 4.9e-5. Everything else measured here did reproduce exactly with the corrected matrices - light-scheme lunch/dinner protanopia 5.05 worst pair, dark worst 15.61, Okabe-Ito floors protan 6.61 / deutan 6.04 / tritan 5.66, gamma-vs-linear dE76 11.40 vs 24.99 - so the conclusions and the one-hex fix stand unchanged.
---

created: 2026-09-07 21:44
---
The description above carries six measurements computed in the gamma-space pipeline that TASK-65.2.1 disproved: the guard's red state is light lunch/dinner protanopia 7.72 against a floor of 12.25 (not 5.05 vs 6.61), deuteranopia also fails at 10.33 vs 11.61, dark passes everywhere (worst tritan 14.86), and the Wong floors are 12.25/11.61/10.87 rather than 6.61/6.04/5.66. Plan Step 0 restates every one of them with where each stale figure lives, so treat the Implementation Plan as authoritative wherever the two disagree - including the shipped hex, which the sweep says should be purple-700 #7e22ce rather than violet-700 #6d28d9.
---
<!-- COMMENTS:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Closed without implementation: reviewing the TASK-65 -> 65.2 -> 65.2.1/65.2.2 chain found it had escalated a single decorative ribbon-chart palette check into a full CIEDE2000 + Machado-dichromacy-simulation CI guard, calibrated against an externally published colourblind-safe palette. Disproportionate for a single-user app (see AGENTS.md). TASK-65.2.1's colour-math code was squashed out of git history; this guard ticket is closed unimplemented. TASK-65 AC#4 revised to rely on the existing meal_ribbon_colours_meet_non_text_contrast WCAG guard instead.
<!-- SECTION:FINAL_SUMMARY:END -->
