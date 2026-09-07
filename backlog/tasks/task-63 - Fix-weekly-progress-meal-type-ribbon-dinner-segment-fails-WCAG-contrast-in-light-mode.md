---
id: TASK-63
title: >-
  Fix: weekly-progress meal-type ribbon dinner segment fails WCAG contrast in
  light mode
status: To Do
assignee: []
created_date: '2026-09-07 04:51'
updated_date: '2026-09-07 04:51'
labels:
  - review-followup
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
- [ ] #1 --meal-dinner's light-mode hex value in nom-core/assets/weekly_progress_widget.html achieves a WCAG relative-luminance contrast ratio of >= 3.0:1 against --bg's light-mode value (#ffffff), computed via the standard WCAG 2.x relative-luminance formula
- [ ] #2 The revised --meal-dinner value stays within the violet family (distinguishable hue from --under/--met/--over) and remains visually distinct in lightness from the other three meal-type colors (--meal-breakfast, --meal-lunch, --meal-none) in light mode -- no two meal-type colors read as the same segment
- [ ] #3 --meal-dinner's dark-mode value is re-checked against --bg's dark-mode value (#171717) for the same >= 3:1 minimum and adjusted if it also fails (verify with the same formula before assuming it passes)
- [ ] #4 A fresh throwaway visual pass (per the TASK-62.5 playbook: seed_data + serve http, screenshots in batches of <=4 images analyzed only by fresh subagents, harness removed afterward) confirms the new dinner segment is clearly visible against the page background in both light and dark, and that boundaries between adjacent ribbon segments remain legible
- [ ] #5 nix develop -c cargo fmt --all --check and nix develop -c cargo clippy --all-targets --all-features --workspace -- -D warnings both pass (asset-only change, but keep the gate green per project convention)
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
SETUP (read first): This is a Rust workspace (nom-core / nom-mcp crates). ALL commands must run inside the Nix dev shell: either run 'direnv allow' once, or prefix every command with 'nix develop -c'. Work from the repository root unless told otherwise. Do not change pinned dependency versions.

1. Open nom-core/assets/weekly_progress_widget.html and locate the `--meal-dinner` custom property in the `:root` block (currently around line 25): `--meal-dinner: light-dark(#c4b5fd, #ede9fe);`.

2. Compute the current WCAG contrast ratio to confirm the finding before touching anything:
```
node -e '
function lum(hex){const c=hex.replace("#","");const[r,g,b]=[0,2,4].map(i=>parseInt(c.substr(i,2),16)/255);const f=v=>v<=0.03928?v/12.92:Math.pow((v+0.055)/1.055,2.4);const[R,G,B]=[r,g,b].map(f);return 0.2126*R+0.7152*G+0.0722*B;}
function contrast(h1,h2){const l1=lum(h1),l2=lum(h2);const[a,b]=l1>l2?[l1,l2]:[l2,l1];return(a+0.05)/(b+0.05);}
console.log("light dinner vs bg:", contrast("#c4b5fd","#ffffff"));
console.log("dark dinner vs bg:", contrast("#ede9fe","#171717"));
'
```

3. Pick a new light-mode hex for `--meal-dinner` that (a) scores >= 3.0 contrast against #ffffff using the script above, (b) stays in the violet family (same hue neighborhood as --meal-breakfast #3b0764 and --meal-lunch #7c3aed), and (c) remains visibly darker/lighter than the other three meal colors so no two segments look identical. A darker violet such as #8b5cf6 or #7c5cf0 is a reasonable starting point -- verify with the script, don't guess. Do not touch --meal-breakfast, --meal-lunch, --meal-none, or any of the pre-existing --under/--met/--over/--accent status colors.

4. Re-run the contrast script for the dark-mode value (#ede9fe against #171717) with the same formula. If it also scores < 3.0, adjust the dark-mode hex too, keeping it inside the violet ramp and distinct from --meal-lunch's dark value (#a78bfa).

5. Update the `--meal-dinner: light-dark(...)` line in nom-core/assets/weekly_progress_widget.html with the new value(s). Do not change RIBBON_TOP, RIBBON_HEIGHT, the mealRibbon() geometry/math, or any other CSS custom property.

6. Run the existing widget resource test to confirm nothing else broke: `nix develop -c cargo nextest run -p nom-core mcp_handler::tests::test_dispatch_read_resource_weekly_progress_widget`.

7. Do a throwaway visual pass following TASK-62.5's Step 4 playbook (seed_data + serve http on a scratch DB path, pull a real get_weekly_progress payload over streamable HTTP, render in a small local harness at 320px width in light and dark, screenshot at most 4 images per prompt and hand them to a fresh subagent for a verdict -- never attach screenshots to your own context). Confirm the dinner segment is clearly visible against the page background and still visually distinct from breakfast/lunch/none. Delete the harness, dev DB, and screenshots afterward -- nothing from this step gets committed.

8. Run: nix develop -c cargo fmt --all --check && nix develop -c cargo clippy --all-targets --all-features --workspace -- -D warnings && nix develop -c cargo nextest run --all-features --workspace
<!-- SECTION:PLAN:END -->
