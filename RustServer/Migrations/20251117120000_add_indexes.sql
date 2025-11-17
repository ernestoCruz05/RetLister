-- Add indexes for query performance (idempotent)
CREATE INDEX IF NOT EXISTS idx_leftovers_material ON leftovers(material);
CREATE INDEX IF NOT EXISTS idx_leftovers_created_at ON leftovers(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_leftovers_thickness ON leftovers(thickness_mm);
