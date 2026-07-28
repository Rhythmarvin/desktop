use super::Migration;

const UP_STATEMENTS: &[&str] = &[r#"
ALTER TABLE sessions ADD COLUMN title TEXT;
"#];

const DOWN_STATEMENTS: &[&str] = &[r#"
ALTER TABLE sessions DROP COLUMN title;
"#];

/// Builds the session title migration.
pub fn migration() -> Migration {
    Migration::new("0004", UP_STATEMENTS, DOWN_STATEMENTS)
}
