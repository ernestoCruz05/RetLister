-- Van fleet management
CREATE TABLE IF NOT EXISTS vans (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL,
  length_mm INTEGER NOT NULL CHECK(length_mm > 0),
  width_mm INTEGER NOT NULL CHECK(width_mm > 0),
  height_mm INTEGER NOT NULL CHECK(height_mm > 0),
  max_weight_kg INTEGER CHECK(max_weight_kg IS NULL OR max_weight_kg > 0),
  active BOOLEAN NOT NULL DEFAULT 1,
  notes TEXT,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Historical loading plans (cargo items sent in request body, not stored)
CREATE TABLE IF NOT EXISTS loading_plans (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  van_id INTEGER NOT NULL REFERENCES vans(id),
  plan_date DATE NOT NULL DEFAULT CURRENT_DATE,
  total_items INTEGER NOT NULL,
  total_weight_kg REAL NOT NULL,
  utilization_percent REAL,  -- Space used / space total
  plan_json TEXT NOT NULL,   -- JSON with 3D positions + original item data
  notes TEXT,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_vans_active ON vans(active);
CREATE INDEX IF NOT EXISTS idx_plans_van ON loading_plans(van_id);
CREATE INDEX IF NOT EXISTS idx_plans_date ON loading_plans(plan_date DESC);
