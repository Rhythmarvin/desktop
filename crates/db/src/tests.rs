use std::path::Path;

use pretty_assertions::assert_eq;
use rusqlite::{Connection, params};
use tempfile::TempDir;

use crate::{
    AppliedMigration, DatabaseBootstrapper, DatabaseError, DatabaseLocation, Migration,
    MigrationCatalog, MigrationDirection, TimestampSource, default_migration_catalog,
};

/// Produces deterministic timestamps so migration bookkeeping tests can assert full records.
#[derive(Clone, Copy, Debug)]
struct FixedTimestampSource {
    now: i64,
}

impl TimestampSource for FixedTimestampSource {
    /// Returns the preconfigured timestamp for every migration applied in a test step.
    fn current_timestamp_millis(&self) -> i64 {
        self.now
    }
}

/// Verifies a fresh database bootstrap applies the shipped schema migration and records its timestamp.
#[test]
fn bootstraps_empty_database_with_default_catalog() {
    let catalog = default_migration_catalog().unwrap();
    let database = DatabaseBootstrapper::new(FixedTimestampSource {
        now: 1_700_000_000_000,
    })
    .bootstrap(&DatabaseLocation::in_memory(), &catalog)
    .unwrap();

    assert_eq!(
        load_table_names(database.connection()),
        vec![
            "artifacts".to_string(),
            "migrations".to_string(),
            "projects".to_string(),
            "sessions".to_string(),
            "tasks".to_string(),
            "virtual_entries".to_string(),
            "virtual_folders".to_string(),
            "worktrees".to_string(),
        ]
    );
    assert_eq!(
        load_applied_migrations(database.connection()),
        vec![AppliedMigration::new("0001", 1_700_000_000_000)]
    );
}

/// Verifies the runner applies only the missing tail of a linear migration history in ascending order.
#[test]
fn applies_missing_migrations_in_ascending_order() {
    let temp_dir = TempDir::new().unwrap();
    let database_path = temp_dir.path().join("upgrade.sqlite3");

    bootstrap_file_database(
        &database_path,
        test_catalog_with_target_prefix(1).unwrap(),
        100,
    );
    bootstrap_file_database(&database_path, test_catalog().unwrap(), 200);

    let connection = Connection::open(&database_path).unwrap();

    assert_eq!(
        load_applied_migrations(&connection),
        vec![
            AppliedMigration::new("0001", 100),
            AppliedMigration::new("0002", 200),
            AppliedMigration::new("0003", 200),
        ]
    );
    assert_eq!(table_exists(&connection, "beta"), true);
    assert_eq!(table_exists(&connection, "gamma"), true);
}

/// Verifies the runner rolls back extra targeted versions in descending order while preserving older records.
#[test]
fn rolls_back_extra_versions_in_descending_order() {
    let temp_dir = TempDir::new().unwrap();
    let database_path = temp_dir.path().join("rollback.sqlite3");

    bootstrap_file_database(&database_path, test_catalog().unwrap(), 300);
    bootstrap_file_database(
        &database_path,
        test_catalog_with_target_prefix(2).unwrap(),
        400,
    );

    let connection = Connection::open(&database_path).unwrap();

    assert_eq!(
        load_applied_migrations(&connection),
        vec![
            AppliedMigration::new("0001", 300),
            AppliedMigration::new("0002", 300),
        ]
    );
    assert_eq!(table_exists(&connection, "beta"), true);
    assert_eq!(table_exists(&connection, "gamma"), false);
}

/// Verifies a mismatch inside the shared prefix fails fast instead of guessing at repair steps.
#[test]
fn rejects_diverged_history_in_shared_prefix() {
    let temp_dir = TempDir::new().unwrap();
    let database_path = temp_dir.path().join("diverged.sqlite3");

    bootstrap_file_database(&database_path, diverged_catalog().unwrap(), 500);

    let error = DatabaseBootstrapper::new(FixedTimestampSource { now: 600 })
        .bootstrap(
            &DatabaseLocation::path(&database_path),
            &test_catalog().unwrap(),
        )
        .unwrap_err();

    assert_eq!(
        match error {
            DatabaseError::DivergedMigrationHistory {
                position,
                expected,
                found,
            } => Some((position, expected, found)),
            _ => None,
        },
        Some((1, "0002".to_string(), "0003".to_string()))
    );
}

/// Verifies a failing up step does not record the version whose SQL could not be installed.
#[test]
fn leaves_failed_up_migration_unrecorded() {
    let temp_dir = TempDir::new().unwrap();
    let database_path = temp_dir.path().join("failed-up.sqlite3");

    bootstrap_file_database(
        &database_path,
        MigrationCatalog::new(vec![create_table_migration("0001", "alpha")]).unwrap(),
        700,
    );

    let error = DatabaseBootstrapper::new(FixedTimestampSource { now: 800 })
        .bootstrap(
            &DatabaseLocation::path(&database_path),
            &MigrationCatalog::new(vec![
                create_table_migration("0001", "alpha"),
                broken_up_migration("0002"),
            ])
            .unwrap(),
        )
        .unwrap_err();

    assert_migration_step_failed(&error, "0002", MigrationDirection::Up);

    let connection = Connection::open(&database_path).unwrap();

    assert_eq!(
        load_applied_migrations(&connection),
        vec![AppliedMigration::new("0001", 700)]
    );
}

/// Verifies a failing down step keeps the extra version recorded because the rollback never commits.
#[test]
fn leaves_failed_down_migration_recorded() {
    let temp_dir = TempDir::new().unwrap();
    let database_path = temp_dir.path().join("failed-down.sqlite3");

    bootstrap_file_database(
        &database_path,
        MigrationCatalog::new(vec![
            create_table_migration("0001", "alpha"),
            broken_down_migration("0002"),
        ])
        .unwrap(),
        800,
    );

    let error = DatabaseBootstrapper::new(FixedTimestampSource { now: 900 })
        .bootstrap(
            &DatabaseLocation::path(&database_path),
            &MigrationCatalog::with_target_versions(
                vec![
                    create_table_migration("0001", "alpha"),
                    broken_down_migration("0002"),
                ],
                vec!["0001"],
            )
            .unwrap(),
        )
        .unwrap_err();

    assert_migration_step_failed(&error, "0002", MigrationDirection::Down);

    let connection = Connection::open(&database_path).unwrap();

    assert_eq!(
        load_applied_migrations(&connection),
        vec![
            AppliedMigration::new("0001", 800),
            AppliedMigration::new("0002", 800),
        ]
    );
}

/// Opens a file-backed database through the bootstrapper and drops the handle once reconciliation finishes.
fn bootstrap_file_database(path: &Path, catalog: MigrationCatalog, now: i64) {
    DatabaseBootstrapper::new(FixedTimestampSource { now })
        .bootstrap(&DatabaseLocation::path(path), &catalog)
        .unwrap();
}

/// Loads visible user tables in alphabetical order so schema assertions remain stable across SQLite versions.
fn load_table_names(connection: &Connection) -> Vec<String> {
    let mut statement = connection
        .prepare(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name ASC",
        )
        .unwrap();
    let rows = statement
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap();

    rows.collect::<Result<Vec<_>, _>>().unwrap()
}

/// Loads persisted migration rows in the same order used by the reconciliation algorithm.
fn load_applied_migrations(connection: &Connection) -> Vec<AppliedMigration> {
    let mut statement = connection
        .prepare("SELECT version, executed_at FROM migrations ORDER BY version ASC")
        .unwrap();
    let rows = statement
        .query_map([], |row| {
            Ok(AppliedMigration::new(
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
            ))
        })
        .unwrap();

    rows.collect::<Result<Vec<_>, _>>().unwrap()
}

/// Reports whether a table currently exists so upgrade and rollback tests can assert schema effects directly.
fn table_exists(connection: &Connection, table_name: &str) -> bool {
    connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
            params![table_name],
            |row| row.get::<_, i64>(0),
        )
        .unwrap()
        == 1
}

/// Verifies a migration-step error identifies the version and direction while preserving the SQL parser context.
fn assert_migration_step_failed(
    error: &DatabaseError,
    expected_version: &str,
    expected_direction: MigrationDirection,
) {
    match error {
        DatabaseError::MigrationStepFailed {
            version,
            direction,
            source,
        } => {
            assert_eq!(version, expected_version);
            assert_eq!(direction, &expected_direction);
            assert_eq!(
                source.to_string().contains("near \"THIS\": syntax error"),
                true
            );
        }
        _ => panic!("expected migration step failure, got {error:?}"),
    }
}

/// Builds the reusable three-step catalog used by upgrade and rollback behavior tests.
fn test_catalog() -> Result<MigrationCatalog, DatabaseError> {
    MigrationCatalog::new(vec![
        create_table_migration("0001", "alpha"),
        create_table_migration("0002", "beta"),
        create_table_migration("0003", "gamma"),
    ])
}

/// Builds the same test catalog with a shorter active prefix to simulate a controlled rollback target.
fn test_catalog_with_target_prefix(
    prefix_length: usize,
) -> Result<MigrationCatalog, DatabaseError> {
    let migrations = vec![
        create_table_migration("0001", "alpha"),
        create_table_migration("0002", "beta"),
        create_table_migration("0003", "gamma"),
    ];
    let target_versions = migrations
        .iter()
        .take(prefix_length)
        .map(Migration::version)
        .collect();

    MigrationCatalog::with_target_versions(migrations, target_versions)
}

/// Builds an alternate catalog whose second version intentionally diverges from the main test sequence.
fn diverged_catalog() -> Result<MigrationCatalog, DatabaseError> {
    MigrationCatalog::new(vec![
        create_table_migration("0001", "alpha"),
        create_table_migration("0003", "gamma"),
    ])
}

/// Builds a simple migration that creates and drops one named table.
fn create_table_migration(version: &'static str, table_name: &'static str) -> Migration {
    let up_sql =
        Box::leak(format!("CREATE TABLE {table_name} (id INTEGER PRIMARY KEY);").into_boxed_str());
    let down_sql = Box::leak(format!("DROP TABLE IF EXISTS {table_name};").into_boxed_str());
    let up_statements = Box::leak(vec![up_sql as &'static str].into_boxed_slice());
    let down_statements = Box::leak(vec![down_sql as &'static str].into_boxed_slice());

    Migration::new(version, up_statements, down_statements)
}

/// Builds a migration whose `up` SQL fails immediately so transaction rollback behavior can be asserted.
fn broken_up_migration(version: &'static str) -> Migration {
    Migration::new(
        version,
        &["THIS IS NOT VALID SQL"],
        &["DROP TABLE IF EXISTS broken_up;"],
    )
}

/// Builds a migration whose `down` SQL fails immediately so rollback bookkeeping can be asserted.
fn broken_down_migration(version: &'static str) -> Migration {
    Migration::new(
        version,
        &["CREATE TABLE broken_down (id INTEGER PRIMARY KEY);"],
        &["THIS IS NOT VALID SQL"],
    )
}
