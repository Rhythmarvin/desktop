use ora_application::{WorkflowRepository, WorkflowRepositoryError};
use ora_domain::{
    AuditFields, CreatedWorkflow, Workflow, WorkflowDetail, WorkflowId, WorkflowSnapshot,
    WorkflowSnapshotId, WorkflowSummary, WorkflowVersion,
};
use rusqlite::{OptionalExtension, Row, Transaction, TransactionBehavior, params};

use crate::repository::{RepositoryPool, connection::bool_to_sqlite};

const DRAFT_VERSION: &str = "draft";

/// Persists workflow definitions and their versioned snapshots in SQLite.
#[derive(Clone, Debug)]
pub struct SqliteWorkflowRepository {
    pool: RepositoryPool,
}

impl SqliteWorkflowRepository {
    /// Builds a workflow repository from the shared SQLite connection pool.
    pub fn new(pool: RepositoryPool) -> Self {
        Self { pool }
    }
}

impl WorkflowRepository for SqliteWorkflowRepository {
    fn create_workflow(
        &self,
        workflow: Workflow,
        draft: WorkflowSnapshot,
    ) -> Result<CreatedWorkflow, WorkflowRepositoryError> {
        self.pool
            .with_connection(|connection| {
                let transaction =
                    Transaction::new_unchecked(connection, TransactionBehavior::Immediate)?;
                transaction.execute(
                    "INSERT INTO workflows (id, name, published_snapshot_id, created_at, updated_at, is_deleted) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        workflow.id.as_ref(),
                        &workflow.name,
                        workflow.published_snapshot_id.as_ref().map(|id| id.as_ref()),
                        workflow.audit_fields.created_at,
                        workflow.audit_fields.updated_at,
                        bool_to_sqlite(workflow.audit_fields.is_deleted),
                    ],
                )?;
                transaction.execute(
                    "INSERT INTO workflow_snapshots (id, workflow_id, version, graph, created_at, updated_at, is_deleted) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        draft.id.as_ref(),
                        draft.workflow_id.as_ref(),
                        &draft.version,
                        &draft.graph,
                        draft.created_at,
                        draft.updated_at,
                        bool_to_sqlite(draft.is_deleted),
                    ],
                )?;
                transaction.commit()?;
                Ok(CreatedWorkflow { workflow, draft })
            })
            .map_err(workflow_repository_error_from_database)
    }

    fn find_workflow(
        &self,
        workflow_id: &WorkflowId,
    ) -> Result<Option<Workflow>, WorkflowRepositoryError> {
        self.pool
            .with_connection(|connection| {
                let mut statement = connection.prepare(
                    "SELECT id, name, published_snapshot_id, created_at, updated_at, is_deleted FROM workflows WHERE id = ?1 AND is_deleted = 0",
                )?;
                let mut rows = statement.query(params![workflow_id.as_ref()])?;
                rows.next()?.map(map_workflow_row).transpose()
            })
            .map_err(workflow_repository_error_from_database)
    }

    fn get_workflow_detail(
        &self,
        workflow_id: &WorkflowId,
    ) -> Result<Option<WorkflowDetail>, WorkflowRepositoryError> {
        self.pool
            .with_connection(|connection| {
                let workflow = {
                    let mut statement = connection.prepare(
                        "SELECT id, name, published_snapshot_id, created_at, updated_at, is_deleted FROM workflows WHERE id = ?1 AND is_deleted = 0",
                    )?;
                    let mut rows = statement.query(params![workflow_id.as_ref()])?;
                    match rows.next()?.map(map_workflow_row).transpose()? {
                        Some(wf) => wf,
                        None => return Ok(None),
                    }
                };

                let draft = {
                    let mut statement = connection.prepare(
                        "SELECT id, workflow_id, version, graph, created_at, updated_at, is_deleted FROM workflow_snapshots WHERE workflow_id = ?1 AND version = ?2 AND is_deleted = 0",
                    )?;
                    let mut rows = statement.query(params![workflow_id.as_ref(), DRAFT_VERSION])?;
                    rows.next()?.map(map_snapshot_row).transpose()?
                };

                let published = workflow
                    .published_snapshot_id
                    .as_ref()
                    .and_then(|published_id| {
                        let mut statement = connection
                            .prepare(
                                "SELECT id, workflow_id, version, graph, created_at, updated_at, is_deleted FROM workflow_snapshots WHERE id = ?1 AND is_deleted = 0",
                            )
                            .ok()?;
                        let mut rows = statement
                            .query(params![published_id.as_ref()])
                            .ok()?;
                        rows.next().ok()?.map(map_snapshot_row).transpose().ok().flatten()
                    });

                Ok(Some(WorkflowDetail {
                    workflow,
                    // Every workflow must have a draft after creation; if it is missing something
                    // is corrupt — return None to signal the aggregate is incomplete.
                    draft: match draft {
                        Some(d) => d,
                        None => return Ok(None),
                    },
                    published,
                }))
            })
            .map_err(workflow_repository_error_from_database)
    }

    fn list_workflows(&self) -> Result<Vec<WorkflowSummary>, WorkflowRepositoryError> {
        self.pool
            .with_connection(|connection| {
                let mut statement = connection.prepare(
                    "SELECT w.id, w.name, ws.version, w.created_at, w.updated_at
                     FROM workflows w
                     LEFT JOIN workflow_snapshots ws
                       ON ws.id = w.published_snapshot_id AND ws.is_deleted = 0
                     WHERE w.is_deleted = 0
                     ORDER BY w.created_at ASC, w.id ASC",
                )?;
                let mut rows = statement.query([])?;
                let mut workflows = Vec::new();
                while let Some(row) = rows.next()? {
                    workflows.push(WorkflowSummary {
                        id: row.get::<_, String>("id")?,
                        name: row.get::<_, String>("name")?,
                        published_version: row.get::<_, Option<String>>("version")?,
                        created_at: row.get("created_at")?,
                        updated_at: row.get("updated_at")?,
                    });
                }
                Ok(workflows)
            })
            .map_err(workflow_repository_error_from_database)
    }

    fn update_workflow(
        &self,
        workflow: Workflow,
    ) -> Result<Workflow, WorkflowRepositoryError> {
        let updated = self
            .pool
            .with_connection(|connection| {
                connection
                    .execute(
                        "UPDATE workflows SET name = ?2, updated_at = ?3 WHERE id = ?1 AND is_deleted = 0",
                        params![workflow.id.as_ref(), &workflow.name, workflow.audit_fields.updated_at],
                    )
                    .map(|rows| rows > 0)
                    .map_err(Into::into)
            })
            .map_err(workflow_repository_error_from_database)?;

        if updated {
            Ok(workflow)
        } else {
            Err(WorkflowRepositoryError::OperationFailed(
                "workflow not found during update".to_string(),
            ))
        }
    }

    fn soft_delete_workflow(
        &self,
        workflow_id: &WorkflowId,
        deleted_at: i64,
    ) -> Result<bool, WorkflowRepositoryError> {
        self.pool
            .with_connection(|connection| {
                let transaction =
                    Transaction::new_unchecked(connection, TransactionBehavior::Immediate)?;
                let exists = transaction
                    .query_row(
                        "SELECT 1 FROM workflows WHERE id = ?1 AND is_deleted = 0",
                        params![workflow_id.as_ref()],
                        |_| Ok(()),
                    )
                    .optional()?
                    .is_some();

                if !exists {
                    return Ok(false);
                }

                transaction.execute(
                    "UPDATE workflow_snapshots SET updated_at = ?2, is_deleted = 1 WHERE workflow_id = ?1 AND is_deleted = 0",
                    params![workflow_id.as_ref(), deleted_at],
                )?;
                transaction.execute(
                    "UPDATE workflows SET updated_at = ?2, is_deleted = 1 WHERE id = ?1 AND is_deleted = 0",
                    params![workflow_id.as_ref(), deleted_at],
                )?;
                transaction.commit()?;
                Ok(true)
            })
            .map_err(workflow_repository_error_from_database)
    }

    fn find_snapshot_by_version(
        &self,
        workflow_id: &WorkflowId,
        version: &str,
    ) -> Result<Option<WorkflowSnapshot>, WorkflowRepositoryError> {
        self.pool
            .with_connection(|connection| {
                let mut statement = connection.prepare(
                    "SELECT id, workflow_id, version, graph, created_at, updated_at, is_deleted FROM workflow_snapshots WHERE workflow_id = ?1 AND version = ?2 AND is_deleted = 0",
                )?;
                let mut rows = statement.query(params![workflow_id.as_ref(), version])?;
                rows.next()?.map(map_snapshot_row).transpose()
            })
            .map_err(workflow_repository_error_from_database)
    }

    fn list_versions(
        &self,
        workflow_id: &WorkflowId,
    ) -> Result<Vec<WorkflowVersion>, WorkflowRepositoryError> {
        self.pool
            .with_connection(|connection| {
                let mut statement = connection.prepare(
                    "SELECT id, version, created_at FROM workflow_snapshots WHERE workflow_id = ?1 AND version != ?2 AND is_deleted = 0 ORDER BY created_at DESC",
                )?;
                let mut rows = statement.query(params![workflow_id.as_ref(), DRAFT_VERSION])?;
                let mut versions = Vec::new();
                while let Some(row) = rows.next()? {
                    versions.push(WorkflowVersion {
                        id: row.get::<_, String>("id")?,
                        version: row.get::<_, String>("version")?,
                        created_at: row.get("created_at")?,
                    });
                }
                Ok(versions)
            })
            .map_err(workflow_repository_error_from_database)
    }

    fn update_draft(
        &self,
        workflow_id: &WorkflowId,
        graph: String,
        updated_at: i64,
    ) -> Result<WorkflowSnapshot, WorkflowRepositoryError> {
        self.pool
            .with_connection(|connection| {
                let rows_affected = connection.execute(
                    "UPDATE workflow_snapshots SET graph = ?3, updated_at = ?4 WHERE workflow_id = ?1 AND version = ?2 AND is_deleted = 0",
                    params![workflow_id.as_ref(), DRAFT_VERSION, &graph, updated_at],
                )?;
                if rows_affected == 0 {
                    return Ok(None);
                }
                let mut statement = connection.prepare(
                    "SELECT id, workflow_id, version, graph, created_at, updated_at, is_deleted FROM workflow_snapshots WHERE workflow_id = ?1 AND version = ?2 AND is_deleted = 0",
                )?;
                let mut rows = statement.query(params![workflow_id.as_ref(), DRAFT_VERSION])?;
                rows.next()?.map(map_snapshot_row).transpose()
            })
            .map_err(workflow_repository_error_from_database)
            .and_then(|opt| {
                opt.ok_or_else(|| {
                    WorkflowRepositoryError::OperationFailed(
                        "draft not found after update".to_string(),
                    )
                })
            })
    }

    fn publish_snapshot(
        &self,
        workflow_id: &WorkflowId,
        snapshot: WorkflowSnapshot,
    ) -> Result<WorkflowSnapshot, WorkflowRepositoryError> {
        self.pool
            .with_connection(|connection| {
                let transaction =
                    Transaction::new_unchecked(connection, TransactionBehavior::Immediate)?;
                transaction.execute(
                    "INSERT INTO workflow_snapshots (id, workflow_id, version, graph, created_at, updated_at, is_deleted) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        snapshot.id.as_ref(),
                        snapshot.workflow_id.as_ref(),
                        &snapshot.version,
                        &snapshot.graph,
                        snapshot.created_at,
                        snapshot.updated_at,
                        bool_to_sqlite(snapshot.is_deleted),
                    ],
                )?;
                transaction.execute(
                    "UPDATE workflows SET published_snapshot_id = ?2, updated_at = ?3 WHERE id = ?1 AND is_deleted = 0",
                    params![workflow_id.as_ref(), snapshot.id.as_ref(), snapshot.created_at],
                )?;
                transaction.commit()?;
                Ok(snapshot)
            })
            .map_err(workflow_repository_error_from_database)
    }

    fn rollback_draft(
        &self,
        workflow_id: &WorkflowId,
        snapshot_id: &WorkflowSnapshotId,
        updated_at: i64,
    ) -> Result<WorkflowSnapshot, WorkflowRepositoryError> {
        self.pool
            .with_connection(|connection| {
                let rows_affected = connection.execute(
                    "UPDATE workflow_snapshots
                     SET graph = (SELECT graph FROM workflow_snapshots WHERE id = ?2 AND is_deleted = 0),
                         updated_at = ?3
                     WHERE workflow_id = ?1 AND version = ?4 AND is_deleted = 0",
                    params![workflow_id.as_ref(), snapshot_id.as_ref(), updated_at, DRAFT_VERSION],
                )?;

                if rows_affected == 0 {
                    return Ok(None);
                }

                let mut statement = connection.prepare(
                    "SELECT id, workflow_id, version, graph, created_at, updated_at, is_deleted FROM workflow_snapshots WHERE workflow_id = ?1 AND version = ?2 AND is_deleted = 0",
                )?;
                let mut rows = statement.query(params![workflow_id.as_ref(), DRAFT_VERSION])?;
                rows.next()?.map(map_snapshot_row).transpose()
            })
            .map_err(workflow_repository_error_from_database)
            .and_then(|opt| {
                opt.ok_or_else(|| {
                    WorkflowRepositoryError::OperationFailed(
                        "draft or target snapshot not found during rollback".to_string(),
                    )
                })
            })
    }

    fn activate_version(
        &self,
        workflow_id: &WorkflowId,
        snapshot_id: &WorkflowSnapshotId,
    ) -> Result<WorkflowSnapshot, WorkflowRepositoryError> {
        self.pool
            .with_connection(|connection| {
                let transaction =
                    Transaction::new_unchecked(connection, TransactionBehavior::Immediate)?;

                transaction.execute(
                    "UPDATE workflows SET published_snapshot_id = ?2 WHERE id = ?1 AND is_deleted = 0",
                    params![workflow_id.as_ref(), snapshot_id.as_ref()],
                )?;

                transaction.execute(
                    "UPDATE workflow_snapshots
                     SET graph = (SELECT graph FROM workflow_snapshots WHERE id = ?2 AND is_deleted = 0)
                     WHERE workflow_id = ?1 AND version = ?3 AND is_deleted = 0",
                    params![workflow_id.as_ref(), snapshot_id.as_ref(), DRAFT_VERSION],
                )?;

                transaction.commit()?;

                let mut statement = connection.prepare(
                    "SELECT id, workflow_id, version, graph, created_at, updated_at, is_deleted FROM workflow_snapshots WHERE workflow_id = ?1 AND version = ?2 AND is_deleted = 0",
                )?;
                let mut rows = statement.query(params![workflow_id.as_ref(), DRAFT_VERSION])?;
                rows.next()?.map(map_snapshot_row).transpose()
            })
            .map_err(workflow_repository_error_from_database)
            .and_then(|opt| {
                opt.ok_or_else(|| {
                    WorkflowRepositoryError::OperationFailed(
                        "draft not found after activation".to_string(),
                    )
                })
            })
    }

    fn soft_delete_snapshot(
        &self,
        snapshot_id: &WorkflowSnapshotId,
        deleted_at: i64,
    ) -> Result<bool, WorkflowRepositoryError> {
        self.pool
            .with_connection(|connection| {
                connection
                    .execute(
                        "UPDATE workflow_snapshots SET updated_at = ?2, is_deleted = 1 WHERE id = ?1 AND is_deleted = 0",
                        params![snapshot_id.as_ref(), deleted_at],
                    )
                    .map(|rows| rows > 0)
                    .map_err(Into::into)
            })
            .map_err(workflow_repository_error_from_database)
    }
}

/// Reconstructs a domain workflow from a selected SQLite row.
fn map_workflow_row(row: &Row<'_>) -> Result<Workflow, crate::DatabaseError> {
    Workflow::new(
        WorkflowId::new(row.get::<_, String>("id")?),
        row.get::<_, String>("name")?,
        row.get::<_, Option<String>>("published_snapshot_id")?
            .map(WorkflowSnapshotId::new),
        AuditFields::new(
            row.get("created_at")?,
            row.get("updated_at")?,
            row.get::<_, i64>("is_deleted")? != 0,
        ),
    )
    .map_err(Into::into)
}

/// Reconstructs a domain snapshot from a selected SQLite row.
fn map_snapshot_row(row: &Row<'_>) -> Result<WorkflowSnapshot, crate::DatabaseError> {
    Ok(WorkflowSnapshot::new(
        WorkflowSnapshotId::new(row.get::<_, String>("id")?),
        WorkflowId::new(row.get::<_, String>("workflow_id")?),
        row.get::<_, String>("version")?,
        row.get::<_, String>("graph")?,
        row.get("created_at")?,
        row.get("updated_at")?,
        row.get::<_, i64>("is_deleted")? != 0,
    ))
}

/// Converts database failures into application-port errors.
fn workflow_repository_error_from_database(
    error: crate::DatabaseError,
) -> WorkflowRepositoryError {
    WorkflowRepositoryError::OperationFailed(error.to_string())
}
