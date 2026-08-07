// The executor is wired into the composition root in the endpoints stage; until then every item
// in this module is only referenced by tests, so the dead-code lint is suppressed here.
#![allow(dead_code)]

use crate::agent_runtime::AgentRuntimeManager;
use crate::clock::SystemClock;
use crate::error::BackendError;
use ora_application::{
    AgentDefinitionRepository, Clock, ExecutionContext, NodeExecutor, RepositoryError,
    WorkflowGraph, WorkflowGraphNode, WorkflowRunCallback, WorkflowRunEngineRepository,
};
use ora_contracts::{
    AgentCli as ContractAgentCli, AttachSessionRequest, PromptSessionEvent, PromptSessionRequest,
    SetSessionConfigRequest, StopSessionRequest, WarmSessionRequest, WarmSessionTarget,
};
use ora_contracts::acp::content::{ContentBlock, TextContent};
use ora_contracts::acp::prompt::StopReason;
use ora_contracts::acp::session::SessionUpdate;
use ora_contracts::acp::session_config_options::{
    SessionConfigOption, SessionConfigOptionCategory, SessionConfigKind, SessionConfigSelectOption,
    SessionConfigSelectOptions,
};
use ora_db::{RepositoryPool, SqliteAgentDefinitionRepository, SqliteWorkflowRunEngineRepository};
use ora_domain::{WorkflowNodeRun, WorkflowNodeRunId, WorkflowNodeStatus, WorkflowRunId, SessionId};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;

/// Separator between multiple transitive-predecessor outputs in an agent prompt.
const UPSTREAM_PREDECESSOR_SEPARATOR: &str = "\n\n---\n\n";

/// Executes one agent node through a real Ora session, reporting completion to the run engine.
///
/// `dispatch` spawns a background task that warms, attaches, configures, prompts, and stops one
/// dedicated session per node, then reports the result through the `WorkflowRunCallback`.
pub struct WorkflowRunNodeExecutor {
    agent_runtime: Arc<AgentRuntimeManager>,
    pool: RepositoryPool,
    agent_repository: SqliteAgentDefinitionRepository,
    callback: Arc<dyn WorkflowRunCallback>,
    clock: SystemClock,
}

impl WorkflowRunNodeExecutor {
    /// Builds an executor from the session runtime, persistence, role catalog, and engine callback.
    pub fn new(
        agent_runtime: Arc<AgentRuntimeManager>,
        pool: RepositoryPool,
        agent_repository: SqliteAgentDefinitionRepository,
        callback: Arc<dyn WorkflowRunCallback>,
        clock: SystemClock,
    ) -> Self {
        Self {
            agent_runtime,
            pool,
            agent_repository,
            callback,
            clock,
        }
    }
}

impl NodeExecutor for WorkflowRunNodeExecutor {
    fn dispatch(
        &self,
        node_run_id: &WorkflowNodeRunId,
        node: &WorkflowGraphNode,
        context: &ExecutionContext,
    ) {
        let agent_runtime = self.agent_runtime.clone();
        let pool = self.pool.clone();
        let agent_repository = self.agent_repository.clone();
        let callback = self.callback.clone();
        let clock = self.clock;
        let node_run_id = node_run_id.clone();
        let node = node.clone();
        let context = context.clone();
        tokio::spawn(async move {
            match drive_agent_node(
                &agent_runtime,
                &pool,
                &agent_repository,
                &clock,
                &node_run_id,
                &node,
                &context,
            )
            .await
            {
                Ok(outcome) => report_outcome(&callback, &context.run.id, &node_run_id, outcome),
                Err(error) => callback.fail_node(&context.run.id, &node_run_id, error.message(), None),
            }
        });
    }
}

/// One finished agent node turn: the accumulated conversation and the provider stop reason.
struct AgentNodeOutcome {
    output: Option<String>,
    stop_reason: StopReason,
}

/// Failures raised while driving one agent node's session.
#[derive(Debug, Error)]
pub enum NodeExecutionError {
    #[error("agent CLI {agent_cli} is not supported")]
    UnknownAgentCli { agent_cli: String },
    #[error("model {model_id} is not advertised by agent CLI {agent_cli}")]
    WorkflowModelNotFound { agent_cli: String, model_id: String },
    #[error("agent node {node_id} has no agent configuration")]
    MissingAgentConfig { node_id: String },
    #[error("prompt session ended without a stop reason")]
    SessionEndedWithoutStopReason,
    #[error("workflow run repository operation failed")]
    Repository(#[from] RepositoryError),
    #[error("session failed: {0}")]
    Session(#[from] BackendError),
}

impl NodeExecutionError {
    /// Renders the actionable message surfaced to the failed node.
    fn message(&self) -> String {
        self.to_string()
    }
}

/// Runs the warm → attach → model → prompt → stop session chain for one agent node.
async fn drive_agent_node(
    agent_runtime: &AgentRuntimeManager,
    pool: &RepositoryPool,
    agent_repository: &SqliteAgentDefinitionRepository,
    clock: &SystemClock,
    node_run_id: &WorkflowNodeRunId,
    node: &WorkflowGraphNode,
    context: &ExecutionContext,
) -> Result<AgentNodeOutcome, NodeExecutionError> {
    let config = node
        .agent_config
        .as_ref()
        .ok_or_else(|| NodeExecutionError::MissingAgentConfig {
            node_id: node.id.clone(),
        })?;
    let agent_cli = resolve_agent_cli(&config.executor.agent_cli)?;

    // Warm a reusable provider session for this run's task.
    let warm = agent_runtime
        .warm_session(WarmSessionRequest {
            target: WarmSessionTarget::Task {
                task_id: context.task.id.to_string(),
            },
            agent_cli,
            client_id: format!("{}:{}", context.run.id, node.id),
        })
        .await?;

    // Attach the warm session to the run task and bind it to the node run immediately, so the
    // frontend can subscribe to permission requests before the prompt starts.
    let attach = agent_runtime
        .attach_session(AttachSessionRequest {
            session_id: warm.session_id.clone(),
            task_id: context.task.id.to_string(),
        })
        .await?;
    let repository = SqliteWorkflowRunEngineRepository::new(pool.clone());
    let now = clock.now_timestamp_millis();
    repository.set_node_run_session_id(node_run_id, &SessionId::new(attach.session.id), now)?;

    // Select the graph-declared model from the warm-advertised options; no silent fallback.
    let (config_id, model_value) =
        match_model_value(&warm.config_options, &config.executor.agent_cli, &config.executor.model_id)?;
    agent_runtime
        .set_session_config(SetSessionConfigRequest {
            session_id: warm.session_id.clone(),
            config_id,
            value: model_value,
        })
        .await?;

    // Resolve the role's system instructions from the agents catalog.
    let role_content = match &config.role_id {
        Some(role_id) => agent_repository
            .find_agent_definition_by_name(role_id)?
            .map(|definition| definition.content),
        None => None,
    };

    // Assemble the prompt: role instructions, the node task, then the transitive-predecessor
    // lineage plus run input.
    let node_runs = repository.list_node_runs(&context.run.id)?;
    let upstream = assemble_upstream(context, node, &node_runs);
    let prompt = assemble_prompt(node, role_content.as_deref(), &upstream);

    let mut stream = agent_runtime
        .prompt_session(PromptSessionRequest {
            session_id: warm.session_id.clone(),
            prompt,
        })
        .await?;

    // Consume the prompt stream; pending permissions surface to the frontend through
    // `load_session` while this driver keeps consuming toward `Completed` (Q1).
    let mut accumulator = ConversationAccumulator::default();
    let mut stop_reason = None;
    while let Some(event) = stream.recv().await {
        match event? {
            PromptSessionEvent::SessionUpdate { update } => accumulator.consume(&update),
            PromptSessionEvent::PermissionRequest(_) => {}
            PromptSessionEvent::Completed { stop_reason: reason } => {
                stop_reason = Some(reason);
                break;
            }
        }
    }
    let stop_reason = stop_reason.ok_or(NodeExecutionError::SessionEndedWithoutStopReason)?;

    // Stop the node's session; the Ora record stays queryable.
    agent_runtime
        .stop_session(StopSessionRequest {
            session_id: warm.session_id.clone(),
        })
        .await?;

    Ok(AgentNodeOutcome {
        output: accumulator.into_json(),
        stop_reason,
    })
}

/// Reports one finished turn to the engine according to the confirmed stop-reason mapping.
fn report_outcome(
    callback: &Arc<dyn WorkflowRunCallback>,
    run_id: &WorkflowRunId,
    node_run_id: &WorkflowNodeRunId,
    outcome: AgentNodeOutcome,
) {
    match outcome.stop_reason {
        StopReason::EndTurn => {
            callback.complete_node(run_id, node_run_id, outcome.output, Some("end_turn".to_string()))
        }
        StopReason::MaxTokens => callback
            .complete_node(run_id, node_run_id, outcome.output, Some("max_tokens".to_string())),
        StopReason::MaxTurnRequests => callback.complete_node(
            run_id,
            node_run_id,
            outcome.output,
            Some("max_turn_requests".to_string()),
        ),
        StopReason::Refusal => callback.fail_node(
            run_id,
            node_run_id,
            "agent refused the request".to_string(),
            outcome.output,
        ),
        StopReason::Cancelled => {
            // A cancelled turn belongs to the cancel flow; the driver must not complete the node.
        }
    }
}

/// Maps the graph's `agentCli` string to the transport `AgentCli` enum.
fn resolve_agent_cli(value: &str) -> Result<ContractAgentCli, NodeExecutionError> {
    match value {
        "open_code" => Ok(ContractAgentCli::OpenCode),
        "nga" => Ok(ContractAgentCli::Nga),
        "code_agent_cli" => Ok(ContractAgentCli::CodeAgentCli),
        "claude" => Ok(ContractAgentCli::Claude),
        "codex" => Ok(ContractAgentCli::Codex),
        _ => Err(NodeExecutionError::UnknownAgentCli {
            agent_cli: value.to_string(),
        }),
    }
}

/// Finds the model option and the value to select for the graph's `modelId`.
///
/// Matching follows the confirmed order: a `Model`-category select, falling back to the sole
/// select option; then an exact value match, then a label-contains match. No match fails the
/// node instead of silently using the CLI default.
fn match_model_value(
    config_options: &[SessionConfigOption],
    agent_cli: &str,
    model_id: &str,
) -> Result<(String, String), NodeExecutionError> {
    let model_option = config_options
        .iter()
        .find(|option| matches!(option.category, Some(SessionConfigOptionCategory::Model)))
        .or_else(|| {
            let selects: Vec<&SessionConfigOption> = config_options
                .iter()
                .filter(|option| matches!(option.kind, SessionConfigKind::Select(_)))
                .collect();
            (selects.len() == 1).then_some(selects[0])
        })
        .ok_or_else(|| NodeExecutionError::WorkflowModelNotFound {
            agent_cli: agent_cli.to_string(),
            model_id: model_id.to_string(),
        })?;
    let SessionConfigKind::Select(select) = &model_option.kind else {
        return Err(NodeExecutionError::WorkflowModelNotFound {
            agent_cli: agent_cli.to_string(),
            model_id: model_id.to_string(),
        });
    };
    let options: Vec<&SessionConfigSelectOption> = match &select.options {
        SessionConfigSelectOptions::Ungrouped(options) => options.iter().collect(),
        SessionConfigSelectOptions::Grouped(groups) => {
            groups.iter().flat_map(|group| group.options.iter()).collect()
        }
    };
    let matched = options.iter().find(|option| {
        option.value.0.as_ref() == model_id || option.name.contains(model_id)
    });
    match matched {
        Some(option) => Ok((model_option.id.0.to_string(), option.value.0.to_string())),
        None => Err(NodeExecutionError::WorkflowModelNotFound {
            agent_cli: agent_cli.to_string(),
            model_id: model_id.to_string(),
        }),
    }
}

/// Builds the prompt content blocks: role instructions, node task, and upstream context.
fn assemble_prompt(
    node: &WorkflowGraphNode,
    role_content: Option<&str>,
    upstream: &str,
) -> Vec<ContentBlock> {
    let mut blocks = Vec::new();
    if let Some(role) = role_content {
        blocks.push(ContentBlock::Text(TextContent::new(format!(
            "<system_instructions>\n{role}\n</system_instructions>"
        ))));
    }
    if let Some(prompt) = node.agent_config.as_ref().map(|config| config.prompt.as_str())
        && !prompt.is_empty()
    {
        blocks.push(ContentBlock::Text(TextContent::new(prompt)));
    }
    if !upstream.is_empty() {
        blocks.push(ContentBlock::Text(TextContent::new(upstream)));
    }
    blocks
}

/// Concatenates the transitive predecessors' final assistant messages plus `run.input`.
fn assemble_upstream(
    context: &ExecutionContext,
    node: &WorkflowGraphNode,
    node_runs: &[WorkflowNodeRun],
) -> String {
    let Ok(graph) = WorkflowGraph::parse(&context.graph_json) else {
        return String::new();
    };
    let mut parts: Vec<String> = graph
        .transitive_predecessors(&node.id)
        .iter()
        .map(|predecessor| {
            node_runs
                .iter()
                .find(|node_run| {
                    node_run.node_id == predecessor.id
                        && node_run.status == WorkflowNodeStatus::Succeeded
                })
                .map(|node_run| last_assistant_message(node_run.output.as_deref()))
                .unwrap_or_default()
        })
        .filter(|part| !part.is_empty())
        .collect();
    if let Some(input) = context.run.input.as_deref()
        && !input.is_empty()
    {
        parts.push(input.to_string());
    }
    parts.join(UPSTREAM_PREDECESSOR_SEPARATOR)
}

/// One message in a node conversation array.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ConversationEntry {
    role: String,
    text: String,
}

/// Accumulates the multi-turn conversation from prompt stream updates.
///
/// Text chunks are grouped by role into messages; the conversation ends with the accumulated
/// array, which the engine writes to `node_run.output` at session end.
#[derive(Debug, Default)]
struct ConversationAccumulator {
    entries: Vec<ConversationEntry>,
    current_role: Option<&'static str>,
    current_text: String,
}

impl ConversationAccumulator {
    /// Records one stream update, keeping user and assistant text messages.
    fn consume(&mut self, update: &SessionUpdate) {
        match update {
            SessionUpdate::UserMessageChunk(chunk) => self.consume_text("user", &chunk.content),
            SessionUpdate::AgentMessageChunk(chunk) => {
                self.consume_text("assistant", &chunk.content)
            }
            _ => {}
        }
    }

    /// Appends one text block to the current message, starting a new one on a role change.
    fn consume_text(&mut self, role: &'static str, block: &ContentBlock) {
        let Some(text) = chunk_text(block) else {
            return;
        };
        if self.current_role == Some(role) {
            self.current_text.push_str(text);
        } else {
            self.flush();
            self.current_role = Some(role);
            self.current_text.push_str(text);
        }
    }

    /// Closes the in-flight message, discarding empty ones.
    fn flush(&mut self) {
        if let Some(role) = self.current_role {
            if !self.current_text.is_empty() {
                self.entries.push(ConversationEntry {
                    role: role.to_string(),
                    text: std::mem::take(&mut self.current_text),
                });
            }
            self.current_role = None;
        }
    }

    /// Serializes the accumulated conversation, or returns `None` when nothing was produced.
    fn into_json(mut self) -> Option<String> {
        self.flush();
        if self.entries.is_empty() {
            return None;
        }
        serde_json::to_string(&self.entries).ok()
    }
}

/// Extracts the text payload from a content block, ignoring non-text blocks.
fn chunk_text(block: &ContentBlock) -> Option<&str> {
    match block {
        ContentBlock::Text(text) => Some(&text.text),
        _ => None,
    }
}

/// Extracts the final assistant message from a node conversation array.
fn last_assistant_message(output: Option<&str>) -> String {
    let Some(output) = output else {
        return String::new();
    };
    let Ok(conversation) = serde_json::from_str::<Vec<ConversationEntry>>(output) else {
        return String::new();
    };
    conversation
        .iter()
        .rev()
        .find_map(|entry| (entry.role == "assistant").then_some(entry.text.clone()))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ora_contracts::acp::session_config_options::{
        SessionConfigId, SessionConfigSelect, SessionConfigValueId,
    };
    use pretty_assertions::assert_eq;

    fn select_option(value: &str, name: &str) -> SessionConfigSelectOption {
        SessionConfigSelectOption::new(
            SessionConfigValueId::new(value.to_string()),
            name.to_string(),
        )
    }

    fn model_option(options: Vec<SessionConfigSelectOption>) -> SessionConfigOption {
        SessionConfigOption {
            id: SessionConfigId::new("model".to_string()),
            name: "Model".to_string(),
            description: None,
            category: Some(SessionConfigOptionCategory::Model),
            kind: SessionConfigKind::Select(SessionConfigSelect::new(
                SessionConfigValueId::new("current".to_string()),
                SessionConfigSelectOptions::Ungrouped(options),
            )),
        }
    }

    #[test]
    fn resolve_agent_cli_maps_snake_case_names() {
        assert_eq!(
            resolve_agent_cli("open_code").unwrap(),
            ContractAgentCli::OpenCode
        );
        assert_eq!(resolve_agent_cli("codex").unwrap(), ContractAgentCli::Codex);
        assert!(matches!(
            resolve_agent_cli("bogus"),
            Err(NodeExecutionError::UnknownAgentCli { .. })
        ));
    }

    #[test]
    fn match_model_value_prefers_exact_value_then_label() {
        let options = vec![
            select_option("fast", "Fast model"),
            select_option("deepseek/deepseek-v4-pro", "DeepSeek V4 Pro"),
        ];
        let config = vec![model_option(options)];
        assert_eq!(
            match_model_value(&config, "open_code", "deepseek/deepseek-v4-pro").unwrap(),
            ("model".to_string(), "deepseek/deepseek-v4-pro".to_string())
        );
        // A label-contains match also works (case-sensitive, per the confirmed model rule).
        assert_eq!(
            match_model_value(&config, "open_code", "DeepSeek").unwrap(),
            ("model".to_string(), "deepseek/deepseek-v4-pro".to_string())
        );
    }

    #[test]
    fn match_model_value_fails_when_no_option_matches() {
        let config = vec![model_option(vec![select_option("fast", "Fast model")])];
        assert!(matches!(
            match_model_value(&config, "open_code", "missing-model"),
            Err(NodeExecutionError::WorkflowModelNotFound { .. })
        ));
    }

    #[test]
    fn match_model_value_falls_back_to_the_lone_select() {
        let option = SessionConfigOption {
            id: SessionConfigId::new("model".to_string()),
            name: "Model".to_string(),
            description: None,
            category: None,
            kind: SessionConfigKind::Select(SessionConfigSelect::new(
                SessionConfigValueId::new("smart".to_string()),
                SessionConfigSelectOptions::Ungrouped(vec![select_option("smart", "Smart")]),
            )),
        };
        assert_eq!(
            match_model_value(&[option], "open_code", "smart").unwrap(),
            ("model".to_string(), "smart".to_string())
        );
    }

    #[test]
    fn assemble_prompt_orders_role_node_and_upstream() {
        let node = WorkflowGraphNode {
            id: "a".to_string(),
            node_type: ora_application::NodeType::Agent,
            title: String::new(),
            description: String::new(),
            instruction: None,
            agent_config: Some(ora_application::AgentConfig {
                executor: ora_application::AgentExecutor {
                    agent_cli: "open_code".to_string(),
                    model_id: "m".to_string(),
                },
                role_id: Some("Researcher".to_string()),
                skills: Vec::new(),
                prompt: "do the task".to_string(),
            }),
        };
        let blocks = assemble_prompt(&node, Some("role text"), "upstream text");
        assert_eq!(blocks.len(), 3);
        match &blocks[0] {
            ContentBlock::Text(text) => {
                assert!(text.text.contains("<system_instructions>"));
                assert!(text.text.contains("role text"));
            }
            _ => panic!("expected text block"),
        }
        match &blocks[1] {
            ContentBlock::Text(text) => assert_eq!(text.text, "do the task"),
            _ => panic!("expected text block"),
        }
        match &blocks[2] {
            ContentBlock::Text(text) => assert_eq!(text.text, "upstream text"),
            _ => panic!("expected text block"),
        }
    }

    #[test]
    fn conversation_accumulator_groups_text_chunks_by_role() {
        let mut accumulator = ConversationAccumulator::default();
        accumulator.consume_text("user", &ContentBlock::Text(TextContent::new("hello ")));
        accumulator.consume_text("user", &ContentBlock::Text(TextContent::new("world")));
        accumulator.consume_text(
            "assistant",
            &ContentBlock::Text(TextContent::new("hi there")),
        );
        let json = accumulator.into_json().unwrap();
        assert_eq!(
            json,
            r#"[{"role":"user","text":"hello world"},{"role":"assistant","text":"hi there"}]"#
        );
    }

    #[test]
    fn last_assistant_message_returns_the_final_assistant() {
        let output =
            r#"[{"role":"user","text":"p"},{"role":"assistant","text":"one"},{"role":"user","text":"p2"},{"role":"assistant","text":"two"}]"#;
        assert_eq!(last_assistant_message(Some(output)), "two");
        assert_eq!(last_assistant_message(None), "");
    }
}
