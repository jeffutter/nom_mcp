-- nom_mcp v1 schema — initial migration (version 1)
-- All six domain tables plus indexes per doc-5 §2.

-- foods: catalog of known foods with full nutrient cache per 100g
CREATE TABLE IF NOT EXISTS foods (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source TEXT NOT NULL CHECK (source IN ('OpenFoodFacts', 'USDA_FDC', 'Custom')),
    external_id TEXT,
    name TEXT NOT NULL,
    calories_per_100g REAL NOT NULL DEFAULT 0,
    protein_g_per_100g REAL NOT NULL DEFAULT 0,
    carbs_g_per_100g REAL NOT NULL DEFAULT 0,
    fat_g_per_100g REAL NOT NULL DEFAULT 0,
    fiber_g_per_100g REAL NOT NULL DEFAULT 0,
    serving_size_g REAL,
    UNIQUE(source, external_id)
);

-- meals: logged meals with optional raw-macro adjustment
CREATE TABLE IF NOT EXISTS meals (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    logged_at TEXT NOT NULL,
    logged_date TEXT NOT NULL,
    -- materialized totals (computed from portions at write time)
    total_calories REAL NOT NULL DEFAULT 0,
    total_protein_g REAL NOT NULL DEFAULT 0,
    total_carbs_g REAL NOT NULL DEFAULT 0,
    total_fat_g REAL NOT NULL DEFAULT 0,
    total_fiber_g REAL NOT NULL DEFAULT 0,
    -- optional raw-macro adjustments (nullable)
    adjustment_calories REAL,
    adjustment_protein_g REAL,
    adjustment_carbs_g REAL,
    adjustment_fat_g REAL,
    adjustment_fiber_g REAL
);

-- portions: individual food entries within a meal, snapshotting nutrients at log time
CREATE TABLE IF NOT EXISTS portions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    meal_id INTEGER NOT NULL REFERENCES meals(id),
    food_id INTEGER NOT NULL REFERENCES foods(id),
    quantity_mode TEXT NOT NULL CHECK (quantity_mode IN ('grams', 'servings')),
    quantity REAL NOT NULL,
    -- snapshot columns: captured from Food at insert time, never updated
    snapshot_calories_per_100g REAL NOT NULL,
    snapshot_protein_g_per_100g REAL NOT NULL,
    snapshot_carbs_g_per_100g REAL NOT NULL,
    snapshot_fat_g_per_100g REAL NOT NULL,
    snapshot_fiber_g_per_100g REAL NOT NULL,
    snapshot_serving_size_g REAL
);

-- weight_entries: daily weight logs
CREATE TABLE IF NOT EXISTS weight_entries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    logged_at TEXT NOT NULL,
    logged_date TEXT NOT NULL,
    value REAL NOT NULL
);

-- goals: versioned nutrition targets with direction
CREATE TABLE IF NOT EXISTS goals (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    effective_from TEXT NOT NULL,
    calories REAL,
    calories_direction TEXT CHECK (calories_direction IN ('target', 'minimum', 'maximum')),
    protein_g REAL,
    protein_g_direction TEXT CHECK (protein_g_direction IN ('target', 'minimum', 'maximum')),
    carbs_g REAL,
    carbs_g_direction TEXT CHECK (carbs_g_direction IN ('target', 'minimum', 'maximum')),
    fat_g REAL,
    fat_g_direction TEXT CHECK (fat_g_direction IN ('target', 'minimum', 'maximum')),
    fiber_g REAL,
    fiber_g_direction TEXT CHECK (fiber_g_direction IN ('target', 'minimum', 'maximum')),
    target_weight REAL
);

-- settings: single-row application settings
CREATE TABLE IF NOT EXISTS settings (
    widget_display_enabled BOOLEAN NOT NULL DEFAULT 0
);

-- Indexes for range queries and FK lookups
CREATE INDEX IF NOT EXISTS idx_meals_logged_date ON meals(logged_date);
CREATE INDEX IF NOT EXISTS idx_portions_meal_id ON portions(meal_id);
CREATE INDEX IF NOT EXISTS idx_weight_entries_logged_date ON weight_entries(logged_date);
CREATE INDEX IF NOT EXISTS idx_goals_effective_from ON goals(effective_from);
