//! The workflow run execution engine.
//!
//! Owns frozen-graph parsing and topology, the engine persistence port, and the run engine.
//! Agent-node execution is delegated through the `NodeExecutor` port (implemented in the backend)
//! and persistence through `WorkflowRunEngineRepository` (implemented in the database layer).

mod graph;
mod node_type;

pub use graph::{
    AgentConfig, AgentExecutor, AgentSkill, GraphError, WorkflowGraph, WorkflowGraphNode,
};
pub use node_type::{NodeType, UnknownNodeType};

#[cfg(test)]
mod tests;
