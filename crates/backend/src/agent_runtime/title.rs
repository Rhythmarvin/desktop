use crate::clock::SystemClock;
use ora_acp::AcpClient;
use ora_application::{Clock, NorthboundBus, SessionRepository};
use ora_contracts::Northbound;
use ora_contracts::acp::literals::AGENT_METHOD_NAMES;
use ora_contracts::acp::session::{ListSessionsRequest, ListSessionsResponse};
use ora_db::SqliteSessionRepository;
use ora_domain::SessionId;
use ora_logging::ora_debug;
use std::future::Future;
use std::path::PathBuf;
use std::time::Duration;
use tokio::process::ChildStdin;

/// Calls ACP `session/list`, finds the matching agent session, and persists
/// the title to the database when it differs from the current value.
///
/// On a successful title change, emits `Northbound::SessionTitleUpdated`.
pub(crate) async fn refresh_session_title(
    client: &AcpClient<ChildStdin>,
    agent_session_id: &str,
    session_id: &SessionId,
    repository: &SqliteSessionRepository,
    cwd: PathBuf,
    northbound: &dyn NorthboundBus,
) {
    let response = match client
        .request::<_, ListSessionsResponse>(
            AGENT_METHOD_NAMES.session_list,
            &ListSessionsRequest::new().cwd(cwd),
        )
        .await
    {
        Ok(response) => response,
        Err(error) => {
            ora_debug!(
                session_id = %session_id,
                error = %error,
                "session/list failed during title refresh"
            );
            return;
        }
    };

    let acp_title = match extract_title(&response, agent_session_id) {
        Some(title) => title,
        None => {
            ora_debug!(
                session_id = %session_id,
                agent_session_id = %agent_session_id,
                "session not found in session/list response"
            );
            return;
        }
    };

    let session = match repository.find_session(session_id) {
        Ok(Some(session)) => session,
        Ok(None) => {
            ora_debug!(session_id = %session_id, "session not found in DB during title refresh");
            return;
        }
        Err(error) => {
            ora_debug!(
                session_id = %session_id,
                error = ?error,
                "failed to read session from DB during title refresh"
            );
            return;
        }
    };

    if !title_needs_update(session.title.as_deref(), &acp_title) {
        return;
    }

    let now = SystemClock.now_timestamp_millis();
    let updated = session.with_title(acp_title.clone(), now);
    if let Err(error) = repository.update_session(updated) {
        ora_debug!(
            session_id = %session_id,
            title = %acp_title,
            error = ?error,
            "failed to persist updated session title"
        );
    } else {
        ora_debug!(
            session_id = %session_id,
            title = %acp_title,
            "session title updated"
        );
        northbound.emit(Northbound::SessionTitleUpdated {
            session_id: session_id.to_string(),
            title: acp_title,
        });
    }
}

/// Finds the matching agent session in a `session/list` response and returns
/// its title, if present.
pub(crate) fn extract_title(
    response: &ListSessionsResponse,
    agent_session_id: &str,
) -> Option<String> {
    response
        .sessions
        .iter()
        .find(|s| s.session_id.0.as_ref() == agent_session_id)
        .and_then(|s| s.title.clone())
}

/// Returns `true` when the current persisted title differs from the agent's
/// latest title.
pub(crate) fn title_needs_update(current: Option<&str>, incoming: &str) -> bool {
    current != Some(incoming)
}

/// Schedules a fire-and-forget task after `delay`.
///
/// Currently backed by `tokio::spawn`; will delegate to the scheduler once it is available.
pub(crate) fn schedule_deferred(delay: Duration, task: impl Future<Output = ()> + Send + 'static) {
    let delay_ms = delay.as_millis();
    tokio::spawn(async move {
        ora_debug!(delay_ms, "deferred task sleeping");
        tokio::time::sleep(delay).await;
        ora_debug!(delay_ms, "deferred task executing");
        task.await;
        ora_debug!(delay_ms, "deferred task completed");
    });
}

/// Returns `true` when the session title is still a default placeholder that
/// should be refreshed.
pub(crate) fn is_default_title(title: Option<&str>) -> bool {
    match title {
        None => true,
        Some(t) if t.starts_with("New session") => true,
        Some(t) if t.starts_with("ACP Session") => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ora_contracts::acp::common::SessionId as AcpSessionId;
    use ora_contracts::acp::session::{ListSessionsResponse, SessionInfo};
    use ora_domain::{AgentCli, AuditFields, Session, SessionId, SessionStatus, TaskId};
    use pretty_assertions::assert_eq;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    // ── is_default_title ─────────────────────────────────────────────────

    #[test]
    fn detects_default_titles() {
        assert_eq!(is_default_title(None), true);
        assert_eq!(
            is_default_title(Some("New session - 2026-07-28T10:00:00.000Z")),
            true
        );
        assert_eq!(is_default_title(Some("ACP Session abc123")), true);
        assert_eq!(is_default_title(Some("项目介绍")), false);
        assert_eq!(is_default_title(Some("Code review")), false);
    }

    // ── extract_title ────────────────────────────────────────────────────

    #[test]
    fn extracts_title_when_session_found() {
        let response = ListSessionsResponse::new(vec![
            SessionInfo::new(AcpSessionId::new("agent-1"), PathBuf::from("/test"))
                .title("New session".to_string()),
        ]);
        assert_eq!(
            extract_title(&response, "agent-1"),
            Some("New session".to_string())
        );
    }

    #[test]
    fn returns_none_when_session_not_found() {
        let response = ListSessionsResponse::new(vec![]);
        assert_eq!(extract_title(&response, "agent-1"), None);
    }

    #[test]
    fn returns_none_when_session_has_no_title() {
        let response = ListSessionsResponse::new(vec![SessionInfo::new(
            AcpSessionId::new("agent-1"),
            PathBuf::from("/test"),
        )]);
        assert_eq!(extract_title(&response, "agent-1"), None);
    }

    // ── title_needs_update ───────────────────────────────────────────────

    #[test]
    fn needs_update_when_current_is_none() {
        assert_eq!(title_needs_update(None, "问候"), true);
    }

    #[test]
    fn needs_update_when_titles_differ() {
        assert_eq!(title_needs_update(Some("旧标题"), "新标题"), true);
    }

    #[test]
    fn skips_update_when_titles_match() {
        assert_eq!(title_needs_update(Some("问候"), "问候"), false);
    }

    // ── schedule_deferred ────────────────────────────────────────────────

    #[tokio::test]
    async fn schedule_deferred_runs_after_delay() {
        let flag = Arc::new(Mutex::new(false));
        let flag_clone = flag.clone();
        let start = tokio::time::Instant::now();

        schedule_deferred(Duration::from_millis(100), async move {
            *flag_clone.lock().unwrap() = true;
        });

        assert_eq!(*flag.lock().unwrap(), false);

        tokio::time::sleep(Duration::from_millis(200)).await;
        assert_eq!(*flag.lock().unwrap(), true);
        assert!(start.elapsed() >= Duration::from_millis(100));
    }

    #[tokio::test]
    async fn schedule_deferred_returns_immediately() {
        let start = tokio::time::Instant::now();

        schedule_deferred(Duration::from_secs(60), async move {
            // This task would run after 60s, but we don't wait for it
        });

        assert!(start.elapsed() < Duration::from_millis(500));
    }

    // ── integration checks ───────────────────────────────────────────────

    #[test]
    fn new_session_needs_title_refresh() {
        let session = Session::new(
            SessionId::new("new"),
            TaskId::new("task-1"),
            AgentCli::OpenCode,
            "agent-new",
            SessionStatus::Running,
            AuditFields::new(1, 1, false),
        );
        assert_eq!(is_default_title(session.title.as_deref()), true);
    }

    #[test]
    fn titled_session_skips_refresh() {
        let session = Session::new(
            SessionId::new("titled"),
            TaskId::new("task-1"),
            AgentCli::OpenCode,
            "agent-titled",
            SessionStatus::Running,
            AuditFields::new(1, 1, false),
        )
        .with_title("项目介绍".to_string(), 1);
        assert_eq!(is_default_title(session.title.as_deref()), false);
    }
}
