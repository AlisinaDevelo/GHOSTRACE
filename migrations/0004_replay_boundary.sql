ALTER TABLE cursors ADD COLUMN boundary_json TEXT;

CREATE INDEX IF NOT EXISTS cursors_boundary_idx
    ON cursors(source, collector_instance, boundary_json);
