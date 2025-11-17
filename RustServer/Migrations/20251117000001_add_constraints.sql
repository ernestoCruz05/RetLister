-- Add CHECK constraints and indexes for data integrity

-- Create indexes for common queries
CREATE INDEX IF NOT EXISTS idx_leftovers_material ON leftovers(material);
CREATE INDEX IF NOT EXISTS idx_leftovers_created_at ON leftovers(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_leftovers_thickness ON leftovers(thickness_mm);


-- Documented constraints (enforced in app layer):
-- CHECK (width_mm > 0 AND width_mm <= 10000)
-- CHECK (height_mm > 0 AND height_mm <= 10000)
-- CHECK (thickness_mm > 0 AND thickness_mm <= 1000)
-- CHECK (LENGTH(material) > 0 AND LENGTH(material) <= 64)
-- CHECK (notes IS NULL OR LENGTH(notes) <= 256)
