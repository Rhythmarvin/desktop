use ora_application::{SessionRepository, SessionRepositoryError};
use ora_domain::{AgentCli, AuditFields, Session, SessionId, SessionStatus, TaskId};
use rusqlite::{Row, params};

use crate::repository::{RepositoryPool, connection::bool_to_sqlite};

/// Persists session snapshots through SQLite while hiding storage details from handlers.
#[derive(Clone, Debug)]
pub struct SqliteSessionRepository {
    pool: RepositoryPool,
}

impl SqliteSessionRepository {
    /// Builds a session repository from the shared repository pool.
    pub fn new(pool: RepositoryPool) -> Self {
        Self { pool }
    }
}

impl SessionRepository for SqliteSessionRepository {
    /// Inserts a new session row and returns the stored session snapshot.
    fn create_session(&self, session: Session) -> Result<Session, SessionRepositoryError> {
        self.pool
            .with_connection(|connection| {
                let inserted_rows = connection.execute(
                    "INSERT INTO sessions (id, task_id, agent_cli, agent_session_id, status, title, created_at, updated_at, is_deleted)
                     SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9
                     WHERE EXISTS (
                         SELECT 1 FROM tasks WHERE id = ?2 AND is_deleted = 0
                     )",
                    params![
                        session.id.as_ref(),
                        session.task_id.as_ref(),
                        session.agent_cli.database_value(),
                        session.agent_session_id,
                        session.status.database_value(),
                        session.title.as_deref(),
                        session.audit_fields.created_at,
                        session.audit_fields.updated_at,
                        bool_to_sqlite(session.audit_fields.is_deleted),
                    ],
                )?;
                if inserted_rows == 0 {
                    return Err(crate::DatabaseError::Sqlite(
                        rusqlite::Error::QueryReturnedNoRows,
                    ));
                }

                Ok(session)
            })
            .map_err(session_repository_error_from_database)
    }

    /// Loads one visible session row by identifier.
    fn find_session(
        &self,
        session_id: &SessionId,
    ) -> Result<Option<Session>, SessionRepositoryError> {
        self.pool
            .with_connection(|connection| {
                let mut statement = connection.prepare(
                    "SELECT id, task_id, agent_cli, agent_session_id, status, title, created_at, updated_at, is_deleted
                     FROM sessions
                     WHERE id = ?1 AND is_deleted = 0",
                )?;
                let mut rows = statement.query(params![session_id.as_ref()])?;

                match rows.next()? {
                    Some(row) => Ok(Some(map_session_row(row)?)),
                    None => Ok(None),
                }
            })
            .map_err(session_repository_error_from_database)
    }

    /// Lists every visible session row in stable storage order.
    fn list_sessions(&self) -> Result<Vec<Session>, SessionRepositoryError> {
        self.pool
            .with_connection(|connection| {
                let mut statement = connection.prepare(
                    "SELECT id, task_id, agent_cli, agent_session_id, status, title, created_at, updated_at, is_deleted
                     FROM sessions
                     WHERE is_deleted = 0
                     ORDER BY created_at, id",
                )?;
                let mut rows = statement.query([])?;
                let mut sessions = Vec::new();

                while let Some(row) = rows.next()? {
                    sessions.push(map_session_row(row)?);
                }

                Ok(sessions)
            })
            .map_err(session_repository_error_from_database)
    }

    /// Updates only the title and returns a fresh snapshot with concurrent status changes intact.
    fn update_session_title(
        &self,
        session_id: &SessionId,
        title: String,
        updated_at: i64,
    ) -> Result<Session, SessionRepositoryError> {
        self.pool
            .with_connection(|connection| {
                let mut statement = connection.prepare(
                    "UPDATE sessions
                     SET title = ?2, updated_at = MAX(updated_at, ?3)
                     WHERE id = ?1 AND is_deleted = 0
                     RETURNING id, task_id, agent_cli, agent_session_id, status, title,
                               created_at, updated_at, is_deleted",
                )?;
                let mut rows = statement.query(params![session_id.as_ref(), title, updated_at])?;
                match rows.next()? {
                    Some(row) => map_session_row(row),
                    None => Err(crate::DatabaseError::Sqlite(
                        rusqlite::Error::QueryReturnedNoRows,
                    )),
                }
            })
            .map_err(session_repository_error_from_database)
    }

    /// Updates only lifecycle state and returns a fresh snapshot with concurrent title changes intact.
    fn update_session_status(
        &self,
        session_id: &SessionId,
        status: SessionStatus,
        updated_at: i64,
    ) -> Result<Session, SessionRepositoryError> {
        self.pool
            .with_connection(|connection| {
                let mut statement = connection.prepare(
                    "UPDATE sessions
                     SET status = ?2, updated_at = MAX(updated_at, ?3)
                     WHERE id = ?1 AND is_deleted = 0
                     RETURNING id, task_id, agent_cli, agent_session_id, status, title,
                               created_at, updated_at, is_deleted",
                )?;
                let mut rows = statement.query(params![
                    session_id.as_ref(),
                    status.database_value(),
                    updated_at
                ])?;
                match rows.next()? {
                    Some(row) => map_session_row(row),
                    None => Err(crate::DatabaseError::Sqlite(
                        rusqlite::Error::QueryReturnedNoRows,
                    )),
                }
            })
            .map_err(session_repository_error_from_database)
    }

    /// Soft-deletes one visible session row and reports whether it existed.
    fn soft_delete_session(
        &self,
        session_id: &SessionId,
        deleted_at: i64,
    ) -> Result<bool, SessionRepositoryError> {
        self.pool
            .with_connection(|connection| {
                let updated_rows = connection.execute(
                    "UPDATE sessions
                     SET updated_at = ?2, is_deleted = 1
                     WHERE id = ?1 AND is_deleted = 0",
                    params![session_id.as_ref(), deleted_at],
                )?;

                Ok(updated_rows > 0)
            })
            .map_err(session_repository_error_from_database)
    }
}

/// Reconstructs a domain session from the selected session columns.
fn map_session_row(row: &Row<'_>) -> Result<Session, crate::DatabaseError> {
    let status = SessionStatus::from_database_value(row.get("status")?)?;
    let agent_cli = AgentCli::from_database_value(&row.get::<_, String>("agent_cli")?)?;
    let is_deleted = row.get::<_, i64>("is_deleted")? != 0;

    let title: Option<String> = row.get("title")?;
    Ok(Session {
        id: SessionId::new(row.get::<_, String>("id")?),
        task_id: TaskId::new(row.get::<_, String>("task_id")?),
        agent_cli,
        agent_session_id: row.get::<_, String>("agent_session_id")?,
        status,
        title,
        audit_fields: AuditFields::new(row.get("created_at")?, row.get("updated_at")?, is_deleted),
    })
}

/// Converts shared database-layer failures into session repository errors.
fn session_repository_error_from_database(error: crate::DatabaseError) -> SessionRepositoryError {
    SessionRepositoryError::OperationFailed(error.to_string())
}
