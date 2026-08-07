//! The workflow run execution engine.
//!
//! Owns frozen-graph parsing and topology, the engine persistence port, and the run engine.
//! Agent-node execution is delegated through the `NodeExecutor` port (implemented in the backend)
//! and persistence through `WorkflowRunEngineRepository` (implemented in the database layer).

// The design places the run engine in `engine/engine.rs`, so the nested module name matches the
// containing directory on purpose.
#[allow(clippy::module_inception)]
mod engine;
mod graph;
mod node_type;
mod ports;

pub use engine::{
    EngineError, NodeExecutor, WorkflowRunCallback, WorkflowRunEngine, WorkflowValidationError,
};
pub use graph::{
    AgentConfig, AgentExecutor, AgentSkill, GraphError, WorkflowGraph, WorkflowGraphNode,
};
pub use node_type::{NodeType, UnknownNodeType};
pub use ports::{
    AdvanceWorkflowRunResult, CancelWorkflowRunResult, ExecutionContext, NodeRunToStart,
    RestartWorkflowRunResult, StartWorkflowRunResult, WorkflowNodeRunIdGenerator,
    WorkflowRunEngineRepository,
};

#[cfg(test)]
mod tests;
