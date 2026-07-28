use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Typed events pushed from the backend toward the frontend.
///
/// Each variant carries its own payload and serializes as a tagged JSON object
/// (`{ "type": "...", ... }`) so the frontend can discriminate on `event.type`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export_to = "northbound.ts")]
pub enum Northbound {
    /// The agent CLI has generated a new session title (or renamed an existing one).
    SessionTitleUpdated { session_id: String, title: String },
}

/// Exports every TypeScript binding declared in this module into the target directory.
pub(crate) fn export(config: &ts_rs::Config) -> Result<(), ts_rs::ExportError> {
    Northbound::export(config)?;
    Ok(())
}
