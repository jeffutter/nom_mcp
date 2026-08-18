# nom_mcp

A single-user Rust MCP server for tracking food, nutrition, and body weight — exposed identically over MCP, local CLI, HTTP, and a remote-CLI thin client.

## Language

**Meal**:
Any logged eating occasion — a full dinner, a snack, a single protein bar. Composed of zero or more Portions plus an optional raw-macro adjustment for anything that doesn't map to a catalog Food; total macros are the sum of both.
_Avoid_: Log Entry, Food Log

**Food**:
A nutrition reference — a name plus macros per serving. One entity type with a `source` discriminator: OpenFoodFacts (barcode), USDA FDC (whole/raw foods), or Custom (user-defined, for dishes uncatalogued in either source). Custom Foods are not a separate type — they share the same shape and are reused the same way once defined.
_Avoid_: Product, Item, CustomFood (as a distinct type)

**Portion**:
A quantity of a specific Food included in a Meal — e.g. "150g of Food X" or "2 servings of Food Y". Links a Meal to a Food plus an amount.
_Avoid_: Serving (reserved for a Food's own reference serving size, not the amount consumed), MealItem

**Weight Entry**:
A single body-weight measurement logged at a point in time. Distinct from Meal — not composed of Portions or Foods.
_Avoid_: Weigh-in

**Goal**:
A user-set daily target for calories, macros, and fiber, tracked against logged Meals to compute progress. Each nutrient target carries an explicit Direction. Distinct from a Weight Entry's target weight, which is part of Goal, not a separate concept and carries no Direction — progress toward it is read directly off the comparison to the latest Weight Entry.
_Avoid_: Limit (ambiguous on its own — Direction says whether a target is a ceiling, floor, or exact aim)

**Direction**:
Whether one of a Goal's nutrient targets is an exact aim, a floor to reach, or a ceiling not to exceed. Set explicitly the first time a nutrient is targeted; never inferred or defaulted, since nothing about a bare number says whether hitting it or staying under it is the win.
_Avoid_: Type, mode

**Fasting Window**:
An automatically derived intermittent-fasting measure: the time from a day's last logged Meal to the next logged Meal (the earliest Meal on any later day). If the following calendar day has no Meals, the window extends to the first Meal on the next day that has one. A day's window is incomplete — and unreported — when that day has no Meals or no Meal exists after it. Nothing is stored or manually logged; it is computed from Meal timestamps. Reported per-day by `get_goal_progress` (`fasting_hours`) and as a weekly average in the Weekly Summary.
_Avoid_: Fast Timer, IF Streak

**Weekly Summary**:
A rolling 7-day nutrition and weight snapshot: daily-average nutrient consumption vs Goal targets (plus a per-day breakdown) alongside a weight trend (start/end/delta within the window, or the latest known Weight Entry if none was logged this week). Computed by the shared `fetch_weekly_summary()` and surfaced two ways: the read-only MCP Resource `nom://weekly-summary` (no CLI/HTTP equivalent, since it has no Operation shape), and the `get_weekly_progress` MCP tool — the latter exists solely so the weekly-progress widget has a `call_tool` result to bind to, since MCP Apps widgets can't get live data from a resource read.
_Avoid_: Weekly Report, Dashboard

**Weight Trend**:
A compact sparkline of the most recent Weight Entries (up to 30, oldest first) plus a signed week-over-week delta whose color reflects movement relative to the active goal's target weight (toward = green, away = red, neutral when no target or at target). Computed server-side by the `get_weight_trend` MCP tool — the latter exists solely so the weight-trend widget has a `call_tool` result to bind to, since MCP Apps widgets can't get live data from a resource read.
_Avoid_: Weight Chart, Trend Line

**Widget Display**:
A single global on/off user preference, settable only via MCP (no CLI/HTTP equivalent), that governs whether an MCP client renders visual widgets instead of plain text/JSON. Persisted in its own settings storage, separate from startup Config. `get_goal_progress` was the first consumer, followed by `get_weekly_progress` and `get_weight_trend`: when enabled, each tool's `list_tools` declaration carries a `_meta.ui.resourceUri` pointing at its own `ui://` MCP Apps widget resource (per SEP-1865 / modelcontextprotocol/ext-apps); `call_tool` output itself never changes. (`TASK-41`)
_Avoid_: Widget Toggle (names the action of flipping it, not the preference itself)
