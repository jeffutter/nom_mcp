---
id: TASK-62.5
title: 'Optional: show meal type in the food-added and weekly-progress widgets'
status: Done
assignee:
  - '@ralph'
created_date: '2026-09-07 01:30'
updated_date: '2026-09-07 04:41'
labels:
  - planned
dependencies: []
parent_task_id: TASK-62
priority: low
type: enhancement
ordinal: 1000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Optional UI polish for TASK-62 — deliberately NOT a dependency of TASK-62, so the parent can close without it. Show the stored meal type in the MCP widgets once the data is flowing (TASK-62.2 for log_meal's result, TASK-62.3 for the weekly breakdown).

Candidates, smallest first:
1. `nom-core/assets/food_added_widget.html` — a meal-type label in the meal header above the portion list (`portionsHtml` at ~L284-299; the log_meal tool result is parsed by `extractLogMeal` at ~L336, so `meal_type` is already available in the payload). Must render gracefully when the field is null.
2. `nom-core/assets/weekly_progress_widget.html` — a per-meal-type line inside each day row, fed by `nutrients.daily_totals[].by_meal_type` (~L261 where daily_totals is indexed by date). Skip days whose breakdown is empty; handle the legacy null bucket.

Constraints: widgets are static HTML shells served verbatim from mcp_handler.rs resource branches, so no server change is needed. Visual verification must respect the repo's image limits — at most 4 screenshots per prompt (e.g. light+dark of one variant per pass), split larger comparisons across separate runs, and prefer analyzing them in subagents.

Needs its own /backlog-planner pass if picked up: layout, dark-mode colors, and whether the compact variants have room are undecided.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Food-added header shows the meal type inline before the date, lowercase wire value passed through escapeHtml; when meal_type is null, absent or empty the header renders the same date-only markup as today - no literal "null" text and no dangling separator.
- [x] #2 Food-added rendered height at 320px width is unchanged vs the HEAD baseline within +/-1px in light and dark (baseline ~139px), staying under the 150px TASK-58 ceiling.
- [x] #3 Weekly widget draws a per-day meal-type composition ribbon beneath each day bar: segments left-to-right in server order (breakfast, lunch, dinner, legacy null last), widths proportional to bucket calories with a 1.5-unit minimum sliver, and the ribbon spans exactly the bar width for every day present in daily_totals so the split reconciles to the day total. Days absent from daily_totals draw nothing.
- [x] #4 Existing calorie bars keep their goal-status fill, min-1px hover bar, tooltip and day-of-week labels, the goal reference line stays, and the new band adds at most 8 viewBox units (72 to 80) with chartH unchanged at 52.
- [x] #5 Weekly rendered height at 320px width stays <= 230px in both light and dark, measured against a HEAD baseline re-measured in the same harness run (expect ~224px vs 217px).
- [x] #6 Meal-type colors come from four new light-dark() custom properties in the violet family plus neutral gray for the legacy bucket, are legible in both schemes, stay visually distinct from the blue/green/red status hues on adjacent bars, and no category relies on color alone - per-segment titles and the enriched day tooltip carry the words.
- [x] #7 Legacy null bucket renders as a neutral gray segment ordered last and reads "(none)" in tooltips - never the word null, never silently dropped.
- [x] #8 Both assets stay self-contained (DOCTYPE kept, no script src or link tags, size-changed reporting intact) and the mcp_handler resource tests are extended to assert meal_type in the food-added asset and by_meal_type in the weekly asset.
- [x] #9 Manual visual verification on a seeded local instance (seed_data + serve http) in light and dark using real payloads pulled over /mcp and REST, screenshots taken in batches of at most 4 images and analyzed only by fresh subagents, dev DB and throwaway harness removed afterwards.
- [x] #10 Full CI gate suite green: fmt check, clippy -D warnings, nextest, doctests, rustdoc.
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
# Plan — TASK-62.5: show Meal Type in the food-added and weekly-progress widgets

## Approach

Two independent, purely client-side edits to static widget shells plus two test-guard
assertions. No server change: both fields already ship in the payloads these widgets bind to.

- `nom-core/assets/food_added_widget.html` — append the meal type to the existing slim header
  line, inline before the date.
- `nom-core/assets/weekly_progress_widget.html` — draw a per-day **meal-type composition
  ribbon** (a thin stacked band) directly beneath each day's calorie bar.

Then extend the two `mcp_handler` resource tests so the new markup is pinned, and run the
established headless-Chromium visual pass against a seeded instance.

Why this works without plumbing: `extractLogMeal`
(`food_added_widget.html:336-345`) and `extractWeeklyProgress`
(`weekly_progress_widget.html:443-452`) `JSON.parse` the whole tool result, so `data.meal_type`
and `byDate[d].by_meal_type` are already reachable inside `render()` untouched. Server side:
`log_meal` emits top-level `"meal_type"` (`nom-core/src/meal/mod.rs:807-815`, always present,
nullable — deliberately not `skip_serializing_if`, see `meal/mod.rs:93-97`);
`nutrients.daily_totals[].by_meal_type` is a `Vec<MealTypeTotals>` ordered
breakfast → lunch → dinner → legacy `null` last (`nom-core/src/meal_type.rs:145-154`,
`weekly/mod.rs:58-71`), and a day present in `daily_totals` always has ≥1 bucket
(`meal_type.rs:192-196`).

## Design decisions (all open questions from the description, resolved here)

**1. Food-added placement — inline prefix on the existing header line, not a new element.**
`.header` (`:35-41`, 11px muted, `line-height: 1`) already holds only the date. A pill/tag would
add ~18px against a ≤150px budget (TASK-58 AC#4; measured 139px in task-58.2). CONTEXT.md calls
the windows "a convenience label, not ground truth", so it must read as de-emphasized metadata:
existing muted color, no weight, no hue.

**2. Casing — render the wire value verbatim, lowercase.** No display map, no
`toUpperCase()`: the lowercase trio *is* the domain vocabulary (`README.md:86-120`,
`meal_type.rs:68` "for storage and display"), a lookup table would have to track enum changes,
and an unknown raw string still renders honestly instead of vanishing. Rejected title case even
though ring labels are abbreviated-initial ("Cal"/"Prot", `:230-236") — casing is presentation
we'd have to justify, and it's a one-line change later if we want it.

**3. Weekly representation — composition ribbon, not restyled bars and not a text block.**
Rejected alternatives:
- *Stacked meal-type segments replacing the status-colored bar*: silently deletes the goal-status
  fill (blue/green/red under/met/over, `:72-77`, `caloriesStatus` `:244-251`) that TASK-56 kept.
  Out of scope for "show meal type".
- *Tooltip-only enrichment*: zero cost, but invisible until hover, and MCP hosts on iOS have no
  hover at all — it wouldn't survive the "glance at the card" purpose. Kept only as the fallback
  (see Risks).
- *Seven new text rows*: +60–80px against a widget TASK-56 deliberately compacted 350px → 217px
  (−38%) with "≥35% reduction" as its acceptance gate. Not worth it.
The ribbon keeps every TASK-56 property, costs +8 viewBox units (~+7.5 rendered px), and puts the
split next to the mark it describes (direct labeling, no legend — legends lose in 320px cards).

**4. Height budget — stated number, not drift.** Calories SVG `height: 72 → 80` with
`padBottom: 16 → 24`, so `chartH` stays exactly 52 and the bars themselves do not grow. At the
widget's rendered scale (SVG viewBox 320 wide, content box 300px after `body { padding: 10px }`,
factor 0.9375) that is **+7.5px**. Baseline 217px @320px (task-56 notes) → expect ≈224px.
**Ceiling: ≤230px in both light and dark**, measured against a HEAD baseline re-measured in the
same harness run. Food-added ceiling unchanged: ≤150px, expected to stay ≈139px ±1px.

**5. Color — four new violet-family custom properties, never color-only.** The widget already
spends blue/green/red on goal status (`:16-18`), so meal types take the one unused family:
monotone-lightness violet ramp (single-hue ramps stay separable under CVD), plus neutral gray for
the legacy bucket. Identity is never carried by hue alone: every segment gets its own `<title>`
word and the day tooltip gains the split.

**6. Legacy `null` bucket — drawn, ordered last, labelled `(none)` in tooltips.** Omitting it
would make breakfast+lunch+dinner visibly less than the shown day total on pre-v2 rows (a trust
bug; cf. GA4 "(other)" backlash). It is *not* renamed "Other"/"Snack"/"Category" — CONTEXT.md
bans those coinages, and `(none)` invents no new domain noun while matching the server's own
ordering rule (`bucket_rank` `None => 3`, `meal_type.rs:145-154`). Unrecognized raw strings fall
into the same neutral treatment, mirroring `bucket_rank`.

## Step 1 — `nom-core/assets/food_added_widget.html` (~8 lines)

Replace the single header line in `render()` (`:323`) with a null-safe builder:

```js
  // Meal type prefixes the header when present. Legacy rows (and any payload
  // missing the field) render the date alone — never the string "null".
  function headerHtml(data) {
    var parts = [];
    if (typeof data.meal_type === "string" && data.meal_type) {
      parts.push(escapeHtml(data.meal_type));
    }
    if (typeof data.logged_date === "string" && data.logged_date) {
      parts.push(escapeHtml(data.logged_date));
    }
    return parts.join(" &middot; ");
  }
```

and in `render()` (`:322-330`): `var html = '<p class="header">' + headerHtml(data) + "</p>";`

Hard requirements:
- Keep `escapeHtml` (`:246-250`) on the value. Its `String(str)` coercion prints literal `"null"`
  for JSON null, which is why the guard above is a type check, not `|| ""` alone. Tasks 43/45 were
  exactly this class of bug; the file's own comment at `:243-245` says the widget cannot trust who
  produced the result.
- Use `textContent` nowhere/new nowhere — stay in the existing `innerHTML` string path so the
  escaping discipline is uniform.
- Separator `&middot;` matches `.portion-sub` (`:293-295`). Do not escape the separator itself —
  it is a literal HTML entity.
- No CSS change. Verify "breakfast · 2026-09-07" fits unwrapped at 320px (≈123px of the 296px
  content box) — wrapping would be the height regression.
- Leave `extractLogMeal` and the handshake alone; `tool-input` (`:384`) carries arguments only —
  read the type from the tool result, not the input.

## Step 2 — `nom-core/assets/weekly_progress_widget.html` (~45 lines)

All work is inside `caloriesChartSvg` (`:253-319`) plus the CSS block. Do not touch
`weightChartSvg` (`:321-396`), the transport, or the handshake.

**2a. Geometry.** In `caloriesChartSvg` (`:254-257`): `height = 72 → 80`, `padBottom = 16 → 24`
(`chartH` stays 52, `viewBox` picks up the new height automatically at `:278`). The day-of-week
label already uses `y = height - 4` (`:313`), so it drops to 76 unaided. New bands: bars keep
`y ∈ [4, 56]`; ribbon occupies `y ∈ [58, 63]` (5 units tall, 2-unit gap below the bars, 5.5-unit
gap above the label ascenders).

**2b. Colors.** Add to the custom-property block (`:9-20`), same `light-dark()` style as the
existing ten, and mirror classes next to `.bar` (`:72-77`):

```css
    --meal-breakfast: light-dark(#4c1d95, #8b5cf6);
    --meal-lunch:     light-dark(#7c3aed, #a78bfa);
    --meal-dinner:    light-dark(#a78bfa, #ddd6fe);
    --meal-none:      light-dark(#d4d4d8, #52525b);
...
    .seg-breakfast { fill: var(--meal-breakfast); }
    .seg-lunch     { fill: var(--meal-lunch); }
    .seg-dinner    { fill: var(--meal-dinner); }
    .seg-none      { fill: var(--meal-none); }
```

Violet is deliberate: `--under`/`--accent` blue and `--met` green / `--over` red are already
spoken for by goal status, and reusing them would read "dinner = over target". Fallback if the
visual review judges `--meal-lunch` too close to dark-theme `--under` (#60a5fa): shift the ramp
magenta (`#86198f` / `#c026d3` / `#f0abfc` light, `#a21caf` / `#e879f9` / `#f5d0fe` dark) rather
than re-tinting status.

**2c. Ribbon drawing.** Call a new helper from the existing per-day loop right after the label
append (`:315`), passing `svg`, `byDate[d]`, `x`, `barW`:

```js
  var MEAL_SEG_CLASS = {
    breakfast: "seg-breakfast",
    lunch: "seg-lunch",
    dinner: "seg-dinner"
  };

  // Thin stacked band under each day's bar: one segment per meal-type bucket,
  // left-to-right in server order (breakfast, lunch, dinner, legacy null last).
  // Widths are proportional to each bucket's calories, with a minimum sliver so
  // a small meal can't vanish and quietly break B+L+D vs the day total.
  function mealRibbon(svg, day, x, barW) { ... }
```

Algorithm (deterministic, sums exactly to `barW`):
1. `buckets = day && Array.isArray(day.by_meal_type) ? day.by_meal_type : []`; keep entries whose
   `calories` is a number `> 0`, preserving array order. Return immediately if none (a day absent
   from `daily_totals` draws nothing — matches the zero-height bar).
2. Raw width `w_k = barW * c_k / Σc`. Clamp any `0 < w_k < 1.5` up to `1.5` and mark fixed;
   redistribute the remaining `barW − Σfixed` across the unfixed segments proportionally to their
   calories. With `barW = max(4, slot*0.6) ≈ 26` at 320px and at most 4 buckets, the clamp can
   never overflow; still guard with "if every segment is fixed, fall back to raw widths".
3. Emit one `<rect>` per kept bucket at the running x offset, `class="meal-seg " +
   (MEAL_SEG_CLASS[b.meal_type] || "seg-none")` — unknown strings land neutral, like
   `bucket_rank`'s `None`. No inter-segment gaps: boundaries come from the lightness steps.
4. Per-segment `<title>` via `textContent` (no `escapeHtml` needed on the SVG DOM path, matching
   `:305-306`): `d + " · " + (typeof b.meal_type === "string" && b.meal_type ? b.meal_type : "(none)") + ": " + fmt(b.calories) + " cal"`.

**2d. Day tooltip.** Enrich the existing bar `<title>` (`:305-306`) to
`"2026-09-07: 1840 cal · breakfast 420 / lunch 610 / dinner 810"` (omit the breakdown entirely
when there are no buckets; use `(none)` for the null bucket). This is the accessibility mitigation
for a 1.5px-wide ribbon sliver being a hopeless hover target, and the reason mobile hosts without
hover still get the information.

**2e. Explicitly preserved:** status-classed bar (`:300`), min-1px hover bar for non-zero days
(`:302-304`), goal reference line (`:280-289`), day-of-week labels, `dateRange` zero-fill
(`:217-236`), `ui/notifications/size-changed` reporting (`:473-484`).

## Step 3 — pin the markup in Rust tests

The HTML assets have no rendering tests; the only guards are substring asserts on the served
bytes. Extend them in the existing "load-bearing string" style (the `size-changed` guard added
after TASK-56 is the precedent, `mcp_handler.rs:1180-1182`):

- `test_dispatch_read_resource_food_added_widget` (`nom-core/src/operation/mcp_handler.rs:1220-1243`):
  assert `text.contains("meal_type")`.
- `test_dispatch_read_resource_weekly_progress_widget` (`:1162-1183`): assert
  `text.contains("by_meal_type")`.

Both assets must keep `<!DOCTYPE html>`, `ui/notifications/size-changed`, and no `<script src` /
`<link` (self-containment asserts at `:1240-1242`; CSP is `connect-src 'none'`, `:44-48`).

## Step 4 — visual verification (reuse the TASK-56 playbook)

Fresh throwaway harness at `/tmp/widget-harness`; **do not commit it or the screenshots**.

1. Baselines first, in the same harness run: `git show HEAD:nom-core/assets/weekly_progress_widget.html`
   and `...food_added_widget.html` into `/tmp/widget-harness/baseline_*.html`.
2. Seed and serve:
   ```sh
   cargo run -p nom-mcp --bin nom-mcp -- seed_data --path /tmp/nom-dev/nom.db
   NOM_MCP_DB_PATH=/tmp/nom-dev/nom.db cargo run -p nom-mcp --bin nom-mcp -- serve http --port 8000
   ```
   (run the server detached with `nohup ... > /tmp/nom-serve.log 2>&1 &` and poll; `seed_data`
   refuses the production DB path by design). Seeds 7 foods, 18 meals over 7 days including today,
   so the weekly window has real multi-bucket days.
3. Real payloads, not synthetic. `get_weekly_progress` is `Surfaces::MCP` only
   (`weekly/mod.rs:284-286`), so pull it over streamable HTTP: POST `initialize` to
   `http://localhost:8000/mcp` with `Accept: application/json, text/event-stream`, capture the
   `mcp-session-id` response header, then `tools/call {"name":"get_weekly_progress","arguments":{}}`
   with that header and save `result.content[0].text` → `weekly.json`. For the food-added widget,
   `log_meal` is REST-surfaced: `POST /api/log_meal -d '{"portions":[{"food_id":<id>,"quantity":200,"quantity_mode":"grams"}]}'`
   (grab a seeded `food_id` from `POST /api/search_food` or `sqlite3 /tmp/nom-dev/nom.db
   'select id,name from foods limit 3'`) → `food_added.json`. Log twice more with explicit
   `"meal_type":"lunch"` / `"dinner"` on the same day if the seed leaves any single-bucket day.
4. Harness page per widget: iframe sized 320px wide (seeded short so `documentElement.scrollHeight`
   reflects content), answer `ui/initialize` with `hostContext.theme`, post
   `ui/notifications/tool-result` carrying the saved JSON, log `ui/notifications/size-changed` —
   that notification is the authoritative height metric. Also read `documentElement.scrollHeight`
   as a cross-check. Expect the known 305px-width scrollbar artifact equally in both variants.
5. Capture with `agent-browser`: `set viewport 320 <h>`, `set media light` / `set media dark`,
   `screenshot`, `eval "documentElement.scrollHeight"`, `console` (must be clean). Use
   `open --session <name>` per variant so the 24h `WIDGET_HTML_TTL_MS` cache (`mcp_handler.rs:101`)
   can't serve stale HTML.
6. **Image budget: ≤4 images per prompt, analyzed only inside fresh subagents — never attach them
   to this session** (AGENTS.md; LiteLLM 400s past 4). Pass A: weekly baseline + new × light/dark
   = 4. Pass B: food-added new × light/dark + the null-payload variant = 3. Ask each subagent for
   numbers (reported heights) and verdicts on legibility, palette separation from status hues, and
   ribbon-vs-bar alignment.
7. Null-path proof needs no server: copy `food_added.json`, set `"meal_type": null`, render, and
   confirm the header is byte-identical to today's date-only output (no "null", no dangling
   separator). Do the same with an adversarial value such as `"<img src=x>"` to prove escaping.
8. Teardown: stop the server, `rm -rf /tmp/nom-dev /tmp/widget-harness`.

## Step 5 — gates

HTML-only diffs skip lefthook's `*.rs` hooks but CI runs everything; run the full set per project
convention:

```sh
nix develop .#ci -c cargo fmt --all --check
nix develop .#ci -c cargo clippy --all-targets --all-features --workspace -- -D warnings
nix develop .#ci -c cargo nextest run --all-features --workspace
nix develop .#ci -c cargo test --doc --all-features --workspace
RUSTDOCFLAGS=-D warnings nix develop .#ci -c cargo doc --no-deps --document-private-items --all-features --workspace --examples
```

## Numbers to hit

| Widget | Metric | Baseline | Target | Ceiling |
|---|---|---|---|---|
| weekly-progress | rendered height @320px | 217px (task-56) | ≈224px | 230px |
| weekly-progress | SVG viewBox | 320×72 | 320×80 | chartH must stay 52 |
| food-added | rendered height @320px | 139px (task-58.2) | 139px | 150px (TASK-58 AC#4) |

## Risks / contingencies

- **Height judged unacceptable** → fall back to tooltip-only: enrich the day `<title>` with the
  split and ship zero geometry change; keep Step 1 and note the ribbon was dropped.
- **Palette collision with status hues** on adjacent bars → magenta ramp fallback in 2b; do not
  touch status colors.
- **Ribbon reads as decoration** (boundaries invisible at 26px bar width) → thicken to 6 units and
  raise `height` to 82 (+9.4px, still ≤230px) before considering a caption, which costs height.
- **Stale cached HTML** in a live host after deploy (24h Public TTL) — verify locally through the
  harness; tell users to reconnect if a host shows old markup.
- **Scope creep**: if the ribbon turns into a redesign, stop and file a follow-up ticket rather
  than spend the height budget.

## Why no sub-tickets

~55 lines across two static assets plus two test assertions, sharing one seed→serve→screenshot
pass, with every design decision already resolved above. Splitting candidate #1 from #2 would add
a planning round-trip (the parent blocks on unplanned children) without removing any risk, and
either step can be dropped independently without breaking the other — so the split buys nothing.
Execute as one session; Steps 1→3 are code, Step 4 is verification, Step 5 is the gate.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented per plan, client-side only. food_added_widget.html: headerHtml() prefixes the wire meal_type (verbatim lowercase, escapeHtml'd) before logged_date, joined with &middot;; null/absent/empty falls back to the exact date-only markup (verified: rendered .header innerHTML byte-identical to HEAD baseline). weekly_progress_widget.html: caloriesChartSvg height 72->80, padBottom 16->24 (chartH held at 52), new mealRibbon() draws one rect per bucket in server order at y=58 h=5, proportional widths with a 1.5-unit min sliver + proportional redistribution; day <title> enriched with the split; legacy/unrecognized buckets take neutral gray via MEAL_SEG_CLASS fallback and read "(none)" in tooltips. Four new light-dark() violet props + --meal-none, plus a 0.75-unit page-coloured hairline stroke on .meal-seg.

Visual pass (throwaway harness at /tmp/widget-harness, dev DB /tmp/nom-dev, both removed after; nothing committed): seeded with seed_data + 3 custom foods created over REST so breakfast/lunch/dinner all land today; weekly payload pulled over streamable HTTP /mcp, food-added over POST /api/log_meal, plus an MCP log_meal call whose result key-set matched REST exactly. Measured @320px viewport via iframe body.offsetHeight (the widget's own size-changed reports the deliberately oversized 400px iframe, so it is not a usable metric here): weekly 222px HEAD baseline -> 230px new in BOTH light and dark (delta +8 = the planned +7.5 rounded at both scales; ceiling <=230 met exactly, absolute px differs from the 217/224 quoted in TASK-56 notes because of harness scale). food-added 137px identical before/after in both themes (baseline was 137, not the 139 quoted in TASK-58.2 — same harness for both, delta 0). Ribbon geometry checked in DOM: every day's segment widths sum to 26.057 == bar width exactly, firstX == bar x; days absent from daily_totals draw nothing.

Three screenshot review passes by fresh subagents, <=4 images each (never attached to this session). Pass 1 flagged intra-violet seams reading as one block and pale violet fusing with the dark-theme blue bar; added the hairline seams and darkened the light palest step. Pass 2 said ship but wanted wider lightness steps; widened the ramp (light #3b0764/#7c3aed/#c4b5fd, dark #7c3aed/#a78bfa/#ede9fe) and made the legacy gray more clearly neutral (#8f8f99 light / #71717a dark). Pass 3: boundaries 1-2/3, no meal-type-vs-goal-status confusion, gray reads neutral, no regressions vs baseline shots, verdict ship. Known accepted limits: the 6-calorie synthetic sliver clamps to ~1.4px and stays hard to see by design (tooltip carries it); unrecognized raw strings keep server arrival order among equal-rank legacy buckets rather than being re-sorted client-side.
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
created: 2026-09-07 04:12
---
Planned. The three open questions in the description are now decided in the Implementation Plan: layout (inline prefix on the food-added header line; a stacked composition ribbon under each weekly calorie bar, not a restyled bar and not new text rows), dark-mode colors (four new light-dark() violet custom properties + neutral gray for the legacy bucket, chosen to avoid collision with the blue/green/red goal-status hues), and whether there is room (yes, stated: SVG viewBox 72->80 with chartH held at 52, ~+7.5px, ceiling 230px vs the 217px TASK-56 baseline; food-added stays ~139px under its 150px ceiling). No sub-tickets - see the plan rationale.
---
<!-- COMMENTS:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Both widgets now surface the stored meal type with no server change. food_added_widget.html prefixes the header line with the log_meal result's meal_type (lowercase wire value, escaped, middle-dot separator) and degrades to byte-identical date-only markup when the field is null/absent. weekly_progress_widget.html draws a per-day meal-type composition ribbon under each calorie bar: segments in server order, widths proportional to bucket calories with a 1.5-unit min sliver, sums reconciling exactly to bar width; legacy/unrecognized buckets render as neutral gray ordered last and read "(none)"; day tooltips carry the full split so nothing depends on hovering a 1.4px sliver or on colour alone. Geometry cost held to the plan: viewBox 320x72 -> 320x80 with chartH still 52, measured 222px -> 230px at 320px width in both themes (<=230 ceiling); food-added unchanged at 137px in both themes. Palette: four new light-dark() violet custom properties plus a neutral legacy gray, refined over three subagent screenshot passes (hairline seams added, ramp widened). mcp_handler resource tests now pin meal_type and by_meal_type bindings. Full gate suite green: fmt, clippy -D warnings, 379 nextest tests, doctests, rustdoc.
<!-- SECTION:FINAL_SUMMARY:END -->
