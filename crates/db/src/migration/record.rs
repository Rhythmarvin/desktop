/// Represents one applied migration row loaded from the SQLite bookkeeping table.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppliedMigration {
    pub version: String,
    pub executed_at: i64,
}

impl AppliedMigration {
    /// Builds a testable value object from the persisted version and execution timestamp.
    pub fn new(version: impl Into<String>, executed_at: i64) -> Self {
        Self {
            version: version.into(),
            executed_at,
        }
    }
}
