use super::Migration;

const UP_STATEMENTS: &[&str] = &[r#"
CREATE TABLE IF NOT EXISTS workflows (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    published_snapshot_id TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    is_deleted INTEGER NOT NULL DEFAULT 0
        CHECK (is_deleted IN (0, 1))
);

CREATE TABLE IF NOT EXISTS workflow_snapshots (
    id TEXT PRIMARY KEY,
    workflow_id TEXT NOT NULL,
    version TEXT NOT NULL,
    graph TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER,
    is_deleted INTEGER NOT NULL DEFAULT 0
        CHECK (is_deleted IN (0, 1)),
    FOREIGN KEY(workflow_id)
        REFERENCES workflows(id)
);

CREATE UNIQUE INDEX workflow_snapshots_active_version_unique
    ON workflow_snapshots(workflow_id, version)
    WHERE is_deleted = 0;
"#];

const DOWN_STATEMENTS: &[&str] = &[r#"
DROP TABLE IF EXISTS workflow_snapshots;
DROP TABLE IF EXISTS workflows;
"#];

/// Builds the workflow definition and snapshot version migration.
pub fn migration() -> Migration {
    Migration::new("0006", UP_STATEMENTS, DOWN_STATEMENTS)
}
