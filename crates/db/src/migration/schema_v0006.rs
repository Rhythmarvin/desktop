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
    UNIQUE(workflow_id, version),
    FOREIGN KEY(workflow_id)
        REFERENCES workflows(id)
);
"#];

const DOWN_STATEMENTS: &[&str] = &[r#"
DROP TABLE IF EXISTS workflow_snapshots;
DROP TABLE IF EXISTS workflows;
"#];

/// Builds the workflow definition and snapshot version migration.
pub fn migration() -> Migration {
    Migration::new("0006", UP_STATEMENTS, DOWN_STATEMENTS)
}
