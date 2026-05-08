use rusqlite::{Connection, Transaction, params};

use crate::{DatabaseError, MigrationCatalog, MigrationDirection, TimestampSource};

use super::AppliedMigration;

const CREATE_MIGRATIONS_TABLE_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS migrations (
    version TEXT PRIMARY KEY,
    executed_at INTEGER NOT NULL
);
"#;

/// Reconciles a SQLite connection with the catalog's target prefix by applying or rolling back migrations.
pub fn reconcile_database<T>(
    connection: &mut Connection,
    catalog: &MigrationCatalog,
    timestamp_source: &T,
) -> Result<(), DatabaseError>
where
    T: TimestampSource,
{
    ensure_migrations_table(connection)?;

    let applied_migrations = load_applied_migrations(connection)?;
    let target_versions = catalog.target_versions();
    let shared_prefix_length = applied_migrations.len().min(target_versions.len());

    for (position, (applied, expected)) in applied_migrations
        .iter()
        .zip(target_versions.iter())
        .take(shared_prefix_length)
        .enumerate()
    {
        if applied.version != *expected {
            return Err(DatabaseError::DivergedMigrationHistory {
                position,
                expected: (*expected).to_string(),
                found: applied.version.clone(),
            });
        }
    }

    if applied_migrations.len() > target_versions.len() {
        for applied_migration in applied_migrations.iter().skip(target_versions.len()).rev() {
            let migration = catalog
                .migration(&applied_migration.version)
                .ok_or_else(|| DatabaseError::UnknownAppliedMigrationVersion {
                    version: applied_migration.version.clone(),
                })?;

            execute_migration_step(
                connection,
                migration.version(),
                migration.down_statements(),
                MigrationDirection::Down,
                |transaction| {
                    transaction.execute(
                        "DELETE FROM migrations WHERE version = ?1",
                        params![migration.version()],
                    )?;

                    Ok(())
                },
            )?;
        }
    }

    if target_versions.len() > applied_migrations.len() {
        for target_version in target_versions.iter().skip(applied_migrations.len()) {
            let migration = catalog.migration(target_version).ok_or_else(|| {
                DatabaseError::UnknownAppliedMigrationVersion {
                    version: (*target_version).to_string(),
                }
            })?;

            execute_migration_step(
                connection,
                migration.version(),
                migration.up_statements(),
                MigrationDirection::Up,
                |transaction| {
                    transaction.execute(
                        "INSERT INTO migrations (version, executed_at) VALUES (?1, ?2)",
                        params![
                            migration.version(),
                            timestamp_source.current_timestamp_millis()
                        ],
                    )?;

                    Ok(())
                },
            )?;
        }
    }

    Ok(())
}

/// Ensures the bookkeeping table exists before reconciliation starts reading or mutating migration state.
fn ensure_migrations_table(connection: &Connection) -> Result<(), DatabaseError> {
    connection.execute_batch(CREATE_MIGRATIONS_TABLE_SQL)?;
    Ok(())
}

/// Loads applied migration rows in ascending version order so prefix comparison stays deterministic.
fn load_applied_migrations(
    connection: &Connection,
) -> Result<Vec<AppliedMigration>, DatabaseError> {
    let mut statement =
        connection.prepare("SELECT version, executed_at FROM migrations ORDER BY version ASC")?;
    let rows = statement.query_map([], |row| {
        Ok(AppliedMigration::new(
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
        ))
    })?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(DatabaseError::from)
}

/// Executes one migration direction inside a transaction so SQL changes and bookkeeping updates succeed together.
fn execute_migration_step<F>(
    connection: &mut Connection,
    version: &str,
    statements: &[&str],
    direction: MigrationDirection,
    finalize: F,
) -> Result<(), DatabaseError>
where
    F: FnOnce(&Transaction<'_>) -> Result<(), rusqlite::Error>,
{
    let transaction = connection.transaction()?;

    for statement in statements {
        // Running each statement separately makes the failing direction and version easier to report.
        transaction.execute_batch(statement).map_err(|source| {
            DatabaseError::MigrationStepFailed {
                version: version.to_string(),
                direction,
                source,
            }
        })?;
    }

    finalize(&transaction).map_err(|source| DatabaseError::MigrationStepFailed {
        version: version.to_string(),
        direction,
        source,
    })?;
    transaction.commit()?;

    Ok(())
}
