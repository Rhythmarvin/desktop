use super::Migration;

const UP_STATEMENTS: &[&str] = &[r#"
CREATE TABLE IF NOT EXISTS project_work_contexts (
    id TEXT PRIMARY KEY,
    surface TEXT NOT NULL,
    window_id TEXT NOT NULL,
    project_id TEXT NOT NULL,
    lease_expires_at INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_project_work_contexts_surface_window
    ON project_work_contexts (surface, window_id);

CREATE INDEX IF NOT EXISTS idx_project_work_contexts_project_lease
    ON project_work_contexts (project_id, lease_expires_at, surface, window_id);

CREATE INDEX IF NOT EXISTS idx_project_work_contexts_expiry
    ON project_work_contexts (lease_expires_at);
"#];

const DOWN_STATEMENTS: &[&str] = &[r#"
DROP INDEX IF EXISTS idx_project_work_contexts_expiry;
DROP INDEX IF EXISTS idx_project_work_contexts_project_lease;
DROP INDEX IF EXISTS idx_project_work_contexts_surface_window;
DROP TABLE IF EXISTS project_work_contexts;
"#];

/// Builds the project work contexts migration.
pub fn migration() -> Migration {
    Migration::new("0002", UP_STATEMENTS, DOWN_STATEMENTS)
}
