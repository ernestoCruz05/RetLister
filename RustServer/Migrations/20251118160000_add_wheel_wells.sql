-- Add wheel well dimensions to vans table
-- Wheels intrude into cargo space from the bottom, reducing usable height

ALTER TABLE vans ADD COLUMN wheel_well_height_mm INTEGER DEFAULT 0 CHECK(wheel_well_height_mm >= 0);
ALTER TABLE vans ADD COLUMN wheel_well_width_mm INTEGER DEFAULT 0 CHECK(wheel_well_width_mm >= 0);
ALTER TABLE vans ADD COLUMN wheel_well_start_x_mm INTEGER DEFAULT 0 CHECK(wheel_well_start_x_mm >= 0);

-- wheel_well_height_mm: How much height the wheel wells take from the floor (e.g., 300mm)
-- wheel_well_width_mm: Width of the wheel well intrusion from each side (e.g., 400mm)
-- wheel_well_start_x_mm: Distance from back door where wheel wells start (e.g., 1500mm)
-- This creates unusable space at the bottom corners starting at wheel_well_start_x_mm
