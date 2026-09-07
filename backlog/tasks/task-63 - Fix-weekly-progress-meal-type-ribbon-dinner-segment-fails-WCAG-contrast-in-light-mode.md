---
id: TASK-63
title: >-
  Fix: weekly-progress meal-type ribbon dinner segment fails WCAG contrast in
  light mode
status: Done
assignee:
  - '@ralph'
created_date: '2026-09-07 04:51'
updated_date: '2026-09-07 14:07'
labels:
  - review-followup
  - planned
dependencies:
  - TASK-62.5
priority: high
ordinal: 100
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while reviewing TASK-62.5 (nom-core/assets/weekly_progress_widget.html:25). The light-mode value of `--meal-dinner` (#c4b5fd) computes to a WCAG relative-luminance contrast ratio of only ~1.85:1 against the widget's white page background (--bg: light-dark(#ffffff, #171717)), well under the 3:1 minimum WCAG 1.4.11 sets for non-text graphical UI elements. In practice this means the dinner segment of the per-day meal-type composition ribbon (mealRibbon() in the same file) is barely visible in light mode, especially on days where dinner is the largest bucket (the common case) and therefore renders as the widest, most visually load-bearing segment. This violates TASK-62.5's own AC #6, which required the four new meal-type colors to be "legible in both schemes" and "stay visually distinct" — the color was reviewed for hue-separation from the blue/green/red goal-status colors across three subagent screenshot passes, but contrast against the page background itself was never checked quantitatively. Correctness axis: an acceptance criterion was checked off without the property it promises actually holding for one of its four color values.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 --meal-dinner's light-mode hex value in nom-core/assets/weekly_progress_widget.html achieves a WCAG relative-luminance contrast ratio of >= 3.0:1 against --bg's light-mode value (#ffffff), computed via the standard WCAG 2.x relative-luminance formula
- [x] #2 The revised --meal-dinner value stays within the violet family (distinguishable hue from --under/--met/--over) and remains visually distinct in lightness from the other three meal-type colors (--meal-breakfast, --meal-lunch, --meal-none) in light mode -- no two meal-type colors read as the same segment
- [x] #3 --meal-dinner's dark-mode value is re-checked against --bg's dark-mode value (#171717) for the same >= 3:1 minimum and adjusted if it also fails (verify with the same formula before assuming it passes)
- [x] #4 A fresh throwaway visual pass (per the TASK-62.5 playbook: seed_data + serve http, screenshots in batches of <=4 images analyzed only by fresh subagents, harness removed afterward) confirms the new dinner segment is clearly visible against the page background in both light and dark, and that boundaries between adjacent ribbon segments remain legible
- [x] #5 nix develop -c cargo fmt --all --check and nix develop -c cargo clippy --all-targets --all-features --workspace -- -D warnings both pass (asset-only change, but keep the gate green per project convention)
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
SETUP (read first): This is a Rust workspace (nom-core / nom-mcp crates). ALL commands must run inside the Nix dev shell: either run 'direnv allow' once, or prefix every command with 'nix develop -c'. Work from the repository root unless told otherwise. Do not change pinned dependency versions, and do not add new Cargo dependencies.

## Scope verdict (measured during planning — read before editing)

The fix is **one CSS line**: `nom-core/assets/weekly_progress_widget.html:25`. The four `--meal-*` props have exactly one definition site (`:root`, lines 23–26) and one consumption chain (`.meal-seg.seg-*` fills, :91–94 → the single `setAttribute("class", …)` in `mealRibbon()` at :349). No other asset renders meal-type colour — `food_added_widget.html` prints `meal_type` as muted *text* only (`color: var(--fg-muted)`); no text sits on a meal-coloured fill (the ribbon rects carry only `<title>` tooltips), so the bar is WCAG 1.4.11's **3:1 non-text** floor, not 4.5:1; and no Rust test, README or CONTEXT.md asserts any of these hexes.

Measured with the standard WCAG 2.x formula (reproduced in Step 1):

| light vs `--bg #ffffff` | ratio | dark vs `#171717` | ratio |
|---|---|---|---|
| breakfast `#3b0764` | 15.00 ✓ | breakfast `#7c3aed` | 3.15 ✓ |
| lunch `#7c3aed` | 5.70 ✓ | lunch `#a78bfa` | 6.59 ✓ |
| **dinner `#c4b5fd`** | **1.85 ✗** | dinner `#ede9fe` | **15.10 ✓** |
| none `#8f8f99` | 3.20 ✓ | none `#71717a` | 3.71 ✓ |

AC#3 therefore needs **no dark-mode change** — verified, not assumed. Note dark breakfast `#7c3aed` at 3.15 is the thinnest margin in the file: don't perturb it.

Segment separation already relies on `.meal-seg { stroke: var(--bg); stroke-width: 0.75 }` (:96–99), not on gaps — `mealRibbon()` lays rects edge-to-edge (`offset += widths[i]`, zero gap). Raw adjacent-fill contrast in light mode is breakfast|lunch **2.63** and dinner|none **1.73**; those seams read as separate today *only* because each fill meets a page-coloured hairline that itself clears 3:1 against both neighbours (W3C technique G209). Step 4 records that so nobody thins the stroke.

### Why "just make it lighter" is off the table

3:1 against `#ffffff` caps relative luminance at 0.3000, i.e. okL ≈ 66 for a violet of this chroma, while `--meal-lunch` `#7c3aed` already sits at okL 54.1. The entire legal band for light dinner is okL ≈ 54–66 — the shipped `#c4b5fd` (okL 81.1) was never contrast-compatible, which is why three screenshot passes couldn't spot it by eye. Extra *lightness* for AC#2 is not available; if seams fuse, buy separation with hue/chroma instead (candidate B).

- **Primary pick: `#8b5cf6`** — Tailwind violet-500, the same scale the ramp is already built from (`#3b0764`=violet-950, `#7c3aed`=violet-600, `#a78bfa`=violet-400, `#ede9fe`=violet-50). Measured 4.23:1 vs white (comfortable margin over the floor), okL 60.6 keeps dinner strictly between lunch (54.1) and none (65.3): ΔL +6.4 from lunch, −4.8 from none, hue 292.7° still squarely violet.
- **Fallback, only if the visual pass says lunch|dinner read as one block: `#a855f7`** (purple-500) — 3.96:1 vs white, okL 62.7 (ΔL +8.6 from lunch) with hue rotated 10.9° toward magenta.
- **Rejected:** `#9d6bfa` (3.55 but okL 64.8, within 0.6 of `--meal-none`, so the dinner|none seam fuses) and `#a78bfa` (2.72, fails outright).

## Step 1 — reproduce the measurement before touching anything

```sh
python3 - <<'PY'
def lum(h):
    h=h.lstrip('#'); ch=[int(h[i:i+2],16)/255 for i in (0,2,4)]
    f=lambda v: v/12.92 if v<=0.03928 else ((v+0.055)/1.055)**2.4
    r,g,b=[f(v) for v in ch]; return 0.2126*r+0.7152*g+0.0722*b
def c(a,b):
    la,lb=lum(a),lum(b); hi,lo=max(la,lb),min(la,lb); return (hi+0.05)/(lo+0.05)
print("dinner light", round(c('#c4b5fd','#ffffff'),2))
print("dinner dark", round(c('#ede9fe','#171717'),2))
PY
```

Expect `dinner light 1.85` (< 3.0) and `dinner dark 15.10` (≥ 3.0). If your numbers differ, stop and reconcile before editing.

## Step 2 — write the failing regression test first (makes AC#1 and AC#3 CI-enforced)

The real defect isn't the hex, it's that nothing measures contrast: axe-core has no 1.4.11 rule (its `color-contrast` rule is text-only), which is how TASK-62.5 signed off "legible in both schemes" while one of its four values measured 1.85. Add a std-only guard — no new deps (`regex`, `palette`, `wcag-contrast` are not dependencies and stay out).

Insert into the existing `#[cfg(test)] mod tests` in `nom-core/src/operation/mcp_handler.rs`, immediately after `test_dispatch_read_resource_weekly_progress_widget` (its `}` is :1186). `use super::*` (:707) already exposes `WEEKLY_PROGRESS_WIDGET_HTML` (:62). Three helpers plus one plain `#[test]` — no TempDb, no `#[serial_test::serial]`, no tokio, since it reads a const string:

```rust
/// Parses a `--name: light-dark(<light>, <dark>);` custom property out of the
/// widget CSS. Alignment padding after the property name varies, so trim both
/// halves.
fn light_dark_pair(css: &str, name: &str) -> (String, String) {
    let marker = format!("--{name}:");
    let at = css.find(&marker).expect("property present") + marker.len();
    let rest = &css[at..];
    let open = "light-dark(";
    let start = rest.find(open).expect("light-dark() value") + open.len();
    let close = start + rest[start..].find(')').expect("closing paren");
    let (light, dark) = rest[start..close].split_once(',').expect("two colours");
    (light.trim().to_owned(), dark.trim().to_owned())
}

/// WCAG 2.x relative luminance of a `#rrggbb` colour.
fn relative_luminance(hex: &str) -> f64 {
    let digits = hex.trim_start_matches('#');
    let channel = |i: usize| {
        let raw = u8::from_str_radix(&digits[i..i + 2], 16).expect("hex pair") as f64 / 255.0;
        if raw <= 0.039_28 {
            raw / 12.92
        } else {
            ((raw + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(0) + 0.7152 * channel(2) + 0.0722 * channel(4)
}

/// WCAG 2.x contrast ratio between two `#rrggbb` colours.
fn contrast_ratio(a: &str, b: &str) -> f64 {
    let (x, y) = (relative_luminance(a), relative_luminance(b));
    let (hi, lo) = if x > y { (x, y) } else { (y, x) };
    (hi + 0.05) / (lo + 0.05)
}

/// SC 1.4.11: every meal-type ribbon fill must hold >= 3:1 against the page
/// background it sits on, in both schemes. The pale light-mode dinner value
/// measured 1.85:1 and shipped anyway (TASK-63) because nothing measured it --
/// axe-core has no non-text contrast rule, so screenshot review was the only
/// check, and 1.85:1 does not look wrong to the eye.
#[test]
fn meal_ribbon_colours_meet_non_text_contrast() {
    let (bg_light, bg_dark) = light_dark_pair(WEEKLY_PROGRESS_WIDGET_HTML, "bg");
    for name in ["meal-breakfast", "meal-lunch", "meal-dinner", "meal-none"] {
        let (light, dark) = light_dark_pair(WEEKLY_PROGRESS_WIDGET_HTML, name);
        let modes = [("light", (&light, &bg_light)), ("dark", (&dark, &bg_dark))];
        for (mode, (fill, bg)) in modes {
            let ratio = contrast_ratio(fill, bg);
            assert!(
                ratio >= 3.0,
                "--{name} {mode} {fill} vs --bg {bg} = {ratio:.2}:1, below the \
                 3:1 minimum for non-text graphical elements (WCAG 1.4.11)"
            );
        }
    }
}
```

This spec was prototyped against the real asset with `rustc` during planning: it parses all five properties correctly and reproduces 1.85 / 15.10 exactly, so the parse path isn't a guess.

Run `nix develop -c cargo nextest run -p nom-core meal_ribbon_colours_meet_non_text_contrast` and expect it to **fail naming `--meal-dinner light #c4b5fd … 1.85:1`**. That red is required before Step 3.

## Step 3 — change the one line (AC#1, #2, #3)

`nom-core/assets/weekly_progress_widget.html:25` becomes:

```css
    --meal-dinner:    light-dark(#8b5cf6, #ede9fe);
```

Dark half unchanged (measured 15.10 ✓). Touch nothing else in `:root`; leave `RIBBON_TOP`, `RIBBON_HEIGHT`, `mealRibbon()` geometry/math, and every `--under/--met/--over/--accent/--track/--border` value alone. Darkening `--meal-lunch` to widen the ramp is out of scope per AC — if you feel you need it, stop and report rather than doing it.

## Step 4 — record the seam contract where it can't be lost

Extend the existing comment above `.meal-seg.seg-breakfast` (:87–90) so the hairline reads as load-bearing rather than decorative: adjacent fills measure only 2.63 (breakfast|lunch) and 1.73 (dinner|none) apart, so the 0.75-unit `var(--bg)` stroke is what makes every seam pass WCAG 1.4.11 (W3C technique G209 — a border that contrasts ≥3:1 with each adjoining colour); thinning or removing it breaks boundary legibility no matter what the fills are. Comment text only — do not change `stroke` or `stroke-width`.

## Step 5 — green the guard

```sh
nix develop -c cargo nextest run -p nom-core meal_ribbon_colours_meet_non_text_contrast
nix develop -c cargo nextest run -p nom-core mcp_handler::tests::test_dispatch_read_resource_weekly_progress_widget
```

Both pass. The other three meal values now act as tripwires: light `none` (3.20) and dark `breakfast` (3.15) clear the floor but barely, so any future lightening fails loudly.

## Step 6 — throwaway visual pass (AC#4)

Follow TASK-62.5's Step 4 playbook verbatim (`backlog/tasks/task-62.5 - Optional-show-meal-type-in-the-food-added-and-weekly-progress-widgets.md`:245–266). Harness under `/tmp/widget-harness`, dev DB `/tmp/nom-dev`, **nothing from this step gets committed**:

1. Baseline for comparison: `git show HEAD:nom-core/assets/weekly_progress_widget.html > /tmp/widget-harness/baseline_weekly.html`.
2. Seed and serve detached (ping-safe): `cargo run -p nom-mcp --bin nom-mcp -- seed_data --path /tmp/nom-dev/nom.db`, then `NOM_MCP_DB_PATH=/tmp/nom-dev/nom.db nohup cargo run -p nom-mcp --bin nom-mcp -- serve http --port 8000 > /tmp/nom-serve.log 2>&1 &` and poll the log. `seed_data` refuses the production DB path by design.
3. Real payload, not synthetic: POST `initialize` to `http://localhost:8000/mcp` with `Accept: application/json, text/event-stream`, capture the `mcp-session-id` response header, then `tools/call {"name":"get_weekly_progress","arguments":{}}` with that header (`Surfaces::MCP` only — `nom-core/src/weekly/mod.rs:284`) and save `result.content[0].text` as `weekly.json`. Confirm the seeded week contains at least one dinner-heavy day and one day carrying all four buckets; create extra meals over `POST /api/log_meal` with explicit `meal_type` if needed.
4. Screenshot the patched asset and the baseline side by side at 320px width in **both** schemes. **Hard limit: ≤4 images per subagent prompt** — the local vLLM rejects larger prompts with HTTP 400 — and never attach screenshots to your own context; hand each batch to a fresh subagent and take back text only. Batch A = light + dark of the patched asset (≤4 shots). Ask for explicit verdicts on: (i) is the dinner segment clearly visible against the page background, (ii) are boundaries between adjacent segments legible, (iii) does dinner read as a meal hue rather than a goal-status colour, (iv) any regression vs the baseline shots.
5. Delete the harness, dev DB and screenshots afterward.

If a fresh-subagent pass still calls lunch|dinner fused, switch the light half to `#a855f7` (Step 3), re-run Step 5 (3.96 ≥ 3.0 keeps the guard green), and re-screenshot with a new subagent. Two consecutive failed passes on different values means stop and report — don't tune blind against the same reviewer.

## Step 7 — full gate (AC#5)

```sh
nix develop -c cargo fmt --all --check \
 && nix develop -c cargo clippy --all-targets --all-features --workspace -- -D warnings \
 && nix develop -c cargo nextest run --all-features --workspace \
 && nix develop -c cargo test --doc --all-features --workspace
```

Clippy runs with `--all-targets -D warnings`, so the new test code must be warning-free too. Commit as `TASK-63: <summary>` with Step 6 artifacts excluded.

## Out of scope / notes

- Don't touch `--meal-breakfast`, `--meal-lunch`, `--meal-none`, or any status colour; don't touch `food_added_widget.html` (it has no meal colours).
- Don't add a colour/contrast crate and don't pull `regex` in for the parser — std string slicing is the house style (see `extract_jsonrpc_errors` in `nom-mcp/src/main.rs:411`).
- Historical backlog prose naming the old `#c4b5fd` (`task-62.5 …md`:337) records what shipped then; leave it and note the correction in this ticket's Implementation Notes instead.
- APCA was considered and deliberately not used as the gate: the WCAG 3 draft still marks its algorithm Exploratory, so it isn't conformance. Everything here is judged against WCAG 2.x 3:1.

## Breakdown

No sub-tickets: this is one CSS value plus the std-only guard test that makes the fix stick, and they must ship together (the guard alone leaves production wrong, the hex alone re-opens the hole). Execute as one session; Steps 1→5 are the change, Step 6 is verification, Step 7 is the gate.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
## Change
- `nom-core/assets/weekly_progress_widget.html:29` (was :25): light half of `--meal-dinner` `#c4b5fd` -> **`#a855f7`**; dark half `#ede9fe` unchanged (measured 15.10:1 vs `--bg` dark `#171717`, so AC#3 needed no change — verified, not assumed).
- Two comments added where the knowledge can't be lost: the `:root` note now states every `--meal-*` value must hold >= 3:1 against `--bg` in its own scheme and names the enforcing test; the `.meal-seg` comment records the seam contract with measured numbers (adjacent fills 2.63 / 1.44 / 1.24 apart, so the 0.75-unit `var(--bg)` stroke is what makes each seam pass SC 1.4.11 per W3C G209 — thinning it breaks boundaries whatever the fills are).
- New std-only guard `meal_ribbon_colours_meet_non_text_contrast` in `nom-core/src/operation/mcp_handler.rs` (after `test_dispatch_read_resource_weekly_progress_widget`): parses the five `light-dark()` props out of `WEEKLY_PROGRESS_WIDGET_HTML`, computes WCAG 2.x relative luminance/contrast, asserts >= 3.0 for all four meal fills in both schemes. No new deps (no regex/palette crate); no TempDb/serial/tokio.

## Red-first evidence
Ran the guard before touching the asset: failed naming `--meal-dinner light #c4b5fd vs --bg #ffffff = 1.85:1`. Green after the hex change. A standalone python reproduction of the same formula gave 1.85 / 15.10 first, matching the plan's table exactly.

## Why the planned primary pick was replaced by its documented fallback
The plan proposed `#8b5cf6` (violet-500, 4.23:1). It passed the numeric gate but a fresh-subagent screenshot pass rated lunch|dinner 2/5 — "read as nearly one continuous violet band; if the stroke were removed they'd merge" — which is precisely the condition the plan reserved for the `#a855f7` fallback. An explicit A/B subagent pass on the two candidates (2 images) picked `#a855f7`: okLCH L 62.7 (+8.6 over lunch's 54.1 vs +6.4 for `#8b5cf6`) with hue rotated 292.7° -> 303.9°, i.e. separation from lunch comes from hue as well as lightness. Cost paid, recorded honestly: contrast vs white drops 4.23 -> 3.96 (still clears 3.0 with margin) and dinner|none raw fill contrast falls 1.32 -> 1.24, so that seam leans harder on the hairline plus chroma (chroma 0.233 vs gray's 0.015). Final pass rated lunch|dinner 4/5 and confirmed both seams read.

## Visual pass (AC#4) — throwaway, nothing committed
Harness under `/tmp/widget-harness` (deleted afterward, with `/tmp/nom-dev`): `git show HEAD:` baseline asset alongside the patched one; `seed_data --path /tmp/nom-dev/nom.db`; `serve http --port 8000` detached; real payload pulled over streamable HTTP (`initialize` -> `mcp-session-id` -> `tools/call get_weekly_progress`), never synthetic. Seeded week Tue 2026-09-01 – Mon 2026-09-07: dinner-heavy day present (Mon, 944 of 1945 cal). To exercise the four-bucket case the log path cannot produce (`meal_type` NULL = legacy bucket), logged one extra breakfast via `POST /api/log_meal` then nulled that row directly in the dev DB. Host page iframed the widget at 320px, answered `ui/initialize` with `hostContext.theme`, posted `ui/notifications/tool-result`, captured `ui/notifications/size-changed`.

- Rendered height **230px for both baseline and patched**, identical in light and dark — no layout regression. Console clean on every load.
- Screenshots taken at 3x device scale; **never more than 3 images in any one prompt, and no image ever entered this session** — each batch went to a fresh subagent that reported back as text.
- Final-pass pixel diff (subagent, ImageMagick): every differing pixel between final-light and baseline-light lies inside a single 740x12 box at y=353–365 — the ribbon strip — with only `#c4b5fd -> #a855f7` changed; title, date range, section headers, bar heights, dashed reference lines, day labels, weight polyline, summary row and canvas size byte-comparable outside it. Dark shots of baseline and patched are md5-identical, as expected with the dark half untouched.
- Verdicts: dinner clearly visible on white in all seven days (baseline's narrow-dinner days were marginal-to-invisible), boundaries legible in both schemes, dinner stays distinguishable from the blue status bars. Marginal-but-pre-existing, not caused by this change: the legacy-bucket sliver renders ~1 CSS px wide, and the ribbon has no legend. Filed as TASK-64 rather than fixed here.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Weekly-progress meal-type ribbon's dinner segment now clears WCAG 1.4.11's 3:1 non-text floor in both schemes: light half of `--meal-dinner` changed `#c4b5fd` (measured 1.85:1 against white) -> `#a855f7` (3.96:1), dark half `#ede9fe` left alone after measuring it at 15.10:1 rather than assuming. The real defect was that nothing measured it, so a std-only regression guard (`meal_ribbon_colours_meet_non_text_contrast` in `mcp_handler.rs`) now parses the widget's five `light-dark()` custom properties and asserts >= 3:1 for all four meal fills in both schemes — written red first (it failed naming 1.85:1) before the hex changed, no new dependencies. The plan's primary pick `#8b5cf6` passed numerically but a fresh-subagent screenshot pass rated lunch|dinner as reading like one band, so its documented fallback `#a855f7` shipped; final pass rated that seam 4/5 and a pixel diff confirmed the only change on screen is the ribbon strip itself, with rendered height unchanged at 230px in both schemes. Throwaway harness (seeded dev DB, real `get_weekly_progress` payload over streamable HTTP, screenshots at 3x in batches of <=3 analyzed only by subagents) removed afterward. fmt, clippy -D warnings, nextest 380/380 and doctests all green. Two pre-existing legibility gaps surfaced by the same pass (legacy-bucket sliver ~1 CSS px wide, no ribbon legend) filed as TASK-64.
<!-- SECTION:FINAL_SUMMARY:END -->
