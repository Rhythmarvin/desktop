use super::Migration;

const UP_STATEMENTS: &[&str] = &[r#"
CREATE TABLE IF NOT EXISTS user_config (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
"#];

const DOWN_STATEMENTS: &[&str] = &[r#"
DROP TABLE IF EXISTS user_config;
"#];

/// Adds the shared key-value store for non-sensitive user preferences.
pub fn migration() -> Migration {
    Migration::new("0006", UP_STATEMENTS, DOWN_STATEMENTS)
}
