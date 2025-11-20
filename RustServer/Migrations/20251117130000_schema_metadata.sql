-- Schema version tracking for safer migrations
CREATE TABLE IF NOT EXISTS schema_metadata (
    version INTEGER PRIMARY KEY,
    applied_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    description TEXT
);

-- Insert initial version
INSERT OR IGNORE INTO schema_metadata (version, applied_at, description) 
VALUES (1, CURRENT_TIMESTAMP, 'Initial schema with leftovers table and indexes');
