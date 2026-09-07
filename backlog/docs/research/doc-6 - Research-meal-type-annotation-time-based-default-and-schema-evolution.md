# Research — Meal Type annotation: prior art, time-based defaults, schema evolution

Supports **TASK-62** (Annotate meals with meal type (breakfast/lunch/dinner) with time-based default).
Findings only — no implementation plan here. Verified-in-this-repo items are marked **[verified]**.

## 1. Industry prior art: three-ish slots, time-derived default, explicit override

- **Android Health Connect `MealType`** is the closest thing to an industry-canonical enum:
  `MEAL_TYPE_BREAKFAST` ("first meal of the day, usually the morning meal"),
  `MEAL_TYPE_LUNCH` ("the noon meal"), `MEAL_TYPE_DINNER` ("last meal of the day, usually the evening
  meal"), `MEAL_TYPE_SNACK` ("any meal outside of the usual three"), and **`MEAL_TYPE_UNKNOWN = 0`**.
  The `UNKNOWN` zero-value is the vendor answer to "rows written before this field existed" —
  relevant to AC #5. <https://developer.android.com/reference/kotlin/androidx/health/connect/client/records/MealType>
- **LogMeal (food-register API)** does exactly what TASK-62 asks, end to end: occasion
  (Breakfast/Lunch/Dinner/Snack) is **assigned server-side from registration time**, is
  **timezone-aware** ("detection takes the user's assigned timezone into account"), supports
  **manual override of a previously stored occasion**, and **history/list endpoints return the
  occasion** so clients can group by it. Default windows:
  **06:00–10:00 → Breakfast, 12:00–15:00 → Lunch, 18:00–23:00 → Dinner, everything else → Snack.**
  Note the windows leave gaps (10–12, 15–18) precisely because it has a fourth catch-all bucket.
  <https://docs.logmeal.com/docs/guides-features-occasion-detection>
- **MyFitnessPal** ships fixed diary *slots* Breakfast/Lunch/Dinner/Snack; timestamps per food are a
  premium feature, and its own help docs tell free users to bake the window into the slot name
  ("Breakfast 7am-10am, Snack 10am-12pm, Lunch 12pm-3pm, Snack 3pm-5pm, Dinner 5pm-9pm,
  Snack 9pm-7am"). Slots are user-renamable; slot ordering is the display grouping in the diary.
  <https://support.myfitnesspal.com/hc/en-us/articles/360032621731>
- Net: nobody publishes a defensible 24-hour breakfast/lunch/dinner partition **without** a snack /
  other catch-all. A three-value-only model must make late-night and mid-afternoon entries fall into
  one of the three by fiat — that is a decision, not a discovery.

## 2. Research literature: there is no accepted time-of-day definition

- Leech et al. (Int J Behav Nutr Phys Act, 2016, Australian NNPAS 2011-12): "There is currently no
  consensus on which approach is best for classifying meals and snacks," and cites work finding
  "little difference in predicting variance in total energy intake when meals and snacks were based
  on either self-report or time-of-day methods." Self-report labels vs time-of-day agree well for
  snacks (ICC ≈ 0.93) but poorly for **meals** (ICC 0.36–0.38 in the 2021 children's follow-up).
  <https://doi.org/10.1186/s12966-016-0459-6>,
  <https://doi.org/10.1186/s12966-021-01231-7>
  → Implication: a time-derived default is a convenience label, not ground truth, which is exactly why
  the explicit-override path (AC #3) carries the real semantic weight.
- Observed population clock times cluster tightly enough for hourly buckets to be sane: Japanese
  nationwide 8-day records give mean start times **07:24 breakfast / 12:29 lunch / 19:15 dinner**, with
  85–97% of each meal inside the ±1h-around-peak 3-hour slot. Australian/UK latent-class studies find
  "Conventional" (~40%), "Later" (~35%), "Grazing/irregular" (~16–25%) patterns — i.e. roughly a quarter
  to a third of adults do not fit conventional hour windows at all.
  <https://doi.org/10.1017/s1368980021000975>, <https://doi.org/10.1186/s12966-016-0459-6>

## 3. Schema evolution under SQLite/Turso — tested against the real dependency

**[verified]** Throwaway integration test against this repo's `turso` 0.8.0-pre.4 via
`Connection::open_at` on a temp DB seeded with a legacy `meals` table (two rows, no `meal_type`):

- `ALTER TABLE … ADD COLUMN meal_type TEXT CHECK(meal_type IN ('breakfast','lunch','dinner'))` on a
  **populated** table: **succeeded**; both existing rows came back with `meal_type IS NULL`.
- `INSERT … VALUES (…, NULL)` after that: **accepted** — SQL CHECK is tri-valued, `NULL IN (…)` is
  NULL, not false, so NULL always passes. So "nullable + CHECK" is a legal shape for legacy rows.
- `INSERT … 'snack'`: **rejected** by the DB (`CHECK constraint failed: meal_type IN (…)`, code 19),
  surfaced through turso as a storage/query error — i.e. it lands in `ErrorCategory::StorageFailure`,
  **not** Validation. AC #4's "standard Validation ErrorData" therefore cannot come from the DB
  constraint; it must be produced in Rust before the INSERT.
- `ADD COLUMN … NOT NULL DEFAULT 'lunch' CHECK(…)`: also succeeded, silently stamping every legacy row
  as `lunch` (available, but it fabricates a claim never made by the user).
- A pure-SQL `UPDATE … SET meal_type = CASE WHEN CAST(substr(logged_at,12,2) AS INTEGER) < 10 …`
  backfill ran fine — but note `logged_at` is stored as a **UTC** string, so any in-SQL hour bucketing
  is wrong for non-UTC users: the timezone lives in Rust's `Clock`, not in the DB.

SQLite docs confirm the general rules and their costs: added-column CHECK constraints are evaluated
against pre-existing rows since 3.37.0, which makes such an `ALTER TABLE` O(rows) rather than O(1);
`ADD CONSTRAINT … CHECK` (adding a constraint to an existing table without a rebuild) exists only from
SQLite 3.52.0 onward. <https://www.sqlite.org/lang_altertable.html>

## 4. Repo-specific constraints the plan has to live inside **[verified]**

- **Migration runner is single-migration by construction.** `apply_migration()` early-returns
  `if current_version >= 1`, `MIGRATION_V1` is `include_str!("schema.sql")`, and hashing is a bespoke
  `hash_v1()`. Adding v2 means generalizing the runner to a version→SQL list. Two tests hard-code
  "exactly one `_migrations` entry" (`nom-core/src/storage/test.rs:113`
  `test_migrations_table_has_version_entry`, `:147` `test_migration_idempotency`) and must change with it.
- **Fresh-install vs upgrade paths must converge.** If `meal_type` is inserted mid-table in
  `schema.sql` but appended by `ALTER TABLE`, fresh and upgraded DBs get different column order.
  Appending at the end of `meals` keeps them identical (all reads/writes use explicit column lists, so
  order is cosmetic — but keeping it identical avoids a latent trap).
- **Validation precedent is `Option<String>` + a `VALID_*` array, not a typed enum.** Goal's
  `calories_direction: Option<String>` is checked against `const VALID_DIRECTIONS: [&str; 3]` to emit
  `ErrorData::validation("calories_direction", "must be one of 'target', 'minimum', 'maximum', got '{d}'")`
  with the *specific* field name (`nom-core/src/goal/mod.rs:~514`). A typed `Option<MealType>` instead
  fails inside `serde_json::from_value`, which the ops map to
  `ErrorData::validation("request", "invalid request: …")` — wrong field name on every surface
  (`cli_router` builds clap args from `input_schema.properties` and does raw-string→JSON conversion with
  no enum awareness, so CLI gives no earlier or better message). Enum-typed fields would buy JSON-Schema
  `enum:` choices for MCP clients at the cost of AC #4's field naming.
- **Output surfaces where meal type must reappear (AC #6/#7):**
  `MealSummary` (serializes as the array returned by `search_meals` **and**
  `get_meals_by_date_range`, and its `.portions` is reused in `log_meal`'s response) is the single
  read chokepoint — `build_meal_summary()` is the only place those two ops assemble rows.
  Daily/weekly summaries are **pure aggregates today**: `goal::fetch_consumed_totals()` does one
  `SUM(...) … WHERE logged_date = ?`, and `weekly::fetch_daily_totals()` groups by `logged_date` only,
  feeding `WeeklySummary{days_with_data, nutrients, daily_totals, weight, fasting}` plus the
  `nom://weekly-summary` resource. Per-meal-type breakdown is new shape in both, not a new field.
- **Eight raw `INSERT INTO meals` call sites** exist outside `log_meal` (goal/weekly/fasting/seed test
  helpers and `seed_data`'s fixture inserts at `seed/mod.rs:363`). A nullable column leaves all of them
  compiling and working unchanged; they'd just produce NULL meal types — which then determines whether
  `seed_data` fixtures need updating for demo/widget realism.
- **Glossary friction (CONTEXT.md).** **Meal** is defined as "Any logged eating occasion — a full
  dinner, a snack, a single protein bar," so a breakfast/lunch/dinner-only taxonomy has nowhere to put
  snacks; the new term must say what a 15:30 protein bar becomes. Also **Direction** already reserves
  `_Avoid_: Type, mode`, so `meal_type` as a name should be chosen deliberately (alternatives in the
  wild: *eating occasion*, LogMeal's *occasion*, MyFitnessPal's *meal slot*). Existing pattern for
  derived-but-materialized attributes to copy is `logged_date` ("materialize at write time, never
  retroactively recompute"); the counter-example is **Fasting Window** ("nothing is stored… computed
  from Meal timestamps").

## 5. Timezone / DST mechanics

- The derivation must go UTC → Clock TZ and read the **local wall-clock hour**, mirroring
  `Clock::logged_date()` (`utc_datetime.with_timezone(&self.tz).date_naive()`). That direction is
  infallible: `TimeZone::from_utc_datetime` / `with_timezone` never fail and never go ambiguous —
  ambiguity only exists in the reverse (local → UTC) direction, which returns
  `MappedLocalTime::{Single, Ambiguous, None}` and is not on this path.
  <https://docs.rs/chrono/latest/chrono/trait.TimeZone.html>,
  <https://docs.rs/chrono/latest/chrono/offset/type.MappedLocalTime.html>
- Interaction worth deciding explicitly: `logged_date` follows the local **calendar day** (midnight
  boundary), so a 01:00 log is today's date while intuitively yesterday's dinner. App vendors split on
  this — Nutrola counts a post-midnight meal in the new day; consumer fora show users expecting the
  previous day. Changing `logged_date` semantics is out of TASK-62's scope, but the meal-type buckets
  make the inconsistency visible for the first time.
- Boundary handling should be half-open intervals over `0..=23` hours (or minute-of-day) so exactly one
  bucket matches, including across midnight; the acceptance criterion names boundary times, so
  `<`/`<=` choice belongs in a helper with named constants rather than inline comparisons.

## 6. Library check: no new dependency needed

`strum` is in `Cargo.lock` only transitively — it is not a `nom-core` dependency. For a 3-variant
enum the repo already has a settled in-house pattern (`Direction`: `#[derive(Serialize, Deserialize,
JsonSchema)] #[serde(rename_all = "lowercase")]` + a hand-written `parse_direction()` match + a
`VALID_*` validation array). Schemars 1.2 honors `#[serde(rename_all = "lowercase")]` when generating
schemas, so a typed enum would produce lowercase `enum:` choices if that trade-off is ever taken.
Per the "prefer settled practice / existing code over something bespoke" rule, strum buys nothing here.
