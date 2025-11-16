CREATE TABLE IF NOT EXISTS leftovers (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    width_mm    INTEGER NOT NULL,
    height_mm   INTEGER NOT NULL,
    thickness_mm INTEGER NOT NULL,
    material    TEXT NOT NULL,
    notes TEXT,
    created_at  TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);