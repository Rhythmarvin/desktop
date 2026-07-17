pub mod agent;
pub mod frame;
pub mod identity;
pub mod json_rpc;
pub mod lifecycle;
pub mod limits;
pub mod manifest;

pub use frame::{
    FrameError, FrameType, HEADER_LEN, MAX_PAYLOAD_BYTES, decode_header, encode_frame,
};
pub use identity::{AgentProviderId, AgentProviderKey, PluginId};
pub use json_rpc::{
    check_duplicate_keys, is_session_fatal, validate_notification_envelope,
    validate_request_envelope, validate_response_envelope, RpcValidationError,
    AGENT_BUSINESS_ERROR, INVALID_PARAMS, INVALID_REQUEST, INTERNAL_ERROR, MAX_JSON_DEPTH,
    METHOD_NOT_FOUND, PARSE_ERROR, REQUEST_CANCELLED, SERVER_BUSY, PluginAddParams,
    PluginJsonRpcError, PluginJsonRpcErrorResponse, PluginJsonRpcRequest,
    PluginJsonRpcSuccessResponse,
};
pub use lifecycle::{
    ActivateParams, ActivateProvider, ActivateReason, ActivateResult, CancelRequestParams,
    DeactivateParams, DeactivateReason, DeclaredAgent, InitializeLimits, InitializeParams,
    InitializePaths, InitializePluginEcho, InitializePluginIdentity, InitializeResult,
    StreamParams,
};
pub use limits::PluginLimits;
pub use manifest::*;

use std::path::Path;
use ts_rs::{Config, ExportError, TS};

/// Exports plugin protocol DTOs for TypeScript SDK packages that speak the same wire format.
pub fn export_typescript_bindings_to(
    output_directory: impl AsRef<Path>,
) -> Result<(), ExportError> {
    let config = Config::new().with_out_dir(output_directory.as_ref());

    // Legacy types
    PluginAddParams::export(&config)?;
    PluginJsonRpcRequest::export(&config)?;
    PluginJsonRpcSuccessResponse::export(&config)?;
    PluginJsonRpcError::export(&config)?;
    PluginJsonRpcErrorResponse::export(&config)?;

    // Identity types
    PluginId::export(&config)?;
    AgentProviderId::export(&config)?;
    AgentProviderKey::export(&config)?;

    // Manifest types
    PluginRelativePath::export(&config)?;
    PluginKindManifest::export(&config)?;
    AgentContributions::export(&config)?;
    AgentContribution::export(&config)?;
    WorkbenchContributions::export(&config)?;
    WorkbenchContributionSet::export(&config)?;
    PluginEngines::export(&config)?;
    PluginManifest::export(&config)?;
    PluginKind::export(&config)?;
    ManifestContributes::export(&config)?;
    ManifestDiagnostic::export(&config)?;

    // Limits
    PluginLimits::export(&config)?;

    // Lifecycle (handshake) DTOs
    InitializeParams::export(&config)?;
    InitializePluginIdentity::export(&config)?;
    InitializePaths::export(&config)?;
    DeclaredAgent::export(&config)?;
    InitializeLimits::export(&config)?;
    InitializeResult::export(&config)?;
    InitializePluginEcho::export(&config)?;
    ActivateParams::export(&config)?;
    ActivateReason::export(&config)?;
    ActivateResult::export(&config)?;
    ActivateProvider::export(&config)?;
    DeactivateParams::export(&config)?;
    DeactivateReason::export(&config)?;
    CancelRequestParams::export(&config)?;
    StreamParams::export(&config)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::export_typescript_bindings_to;
    use std::fs;
    use tempfile::TempDir;

    /// Verifies SDK protocol bindings export successfully.
    #[test]
    fn exports_typescript_protocol_bindings() {
        let output_directory = TempDir::new().unwrap_or_else(|error| {
            panic!("failed to create protocol export directory: {error}");
        });

        export_typescript_bindings_to(output_directory.path()).unwrap_or_else(|error| {
            panic!("expected protocol export to succeed: {error}");
        });

        let generated_source =
            fs::read_to_string(output_directory.path().join("plugin-protocol.ts"))
                .unwrap_or_else(|error| panic!("failed to read protocol export: {error}"));
        let exported_types: Vec<&str> = generated_source
            .lines()
            .filter(|line| line.starts_with("export type "))
            .collect();

        // Legacy types still present
        assert!(
            exported_types.iter().any(|l| l.contains("PluginAddParams")),
            "PluginAddParams should be exported"
        );

        // New identity types
        assert!(
            exported_types.iter().any(|l| l.contains("PluginId")),
            "PluginId should be exported"
        );
        assert!(
            exported_types.iter().any(|l| l.contains("AgentProviderId")),
            "AgentProviderId should be exported"
        );

        // Lifecycle types
        assert!(
            exported_types
                .iter()
                .any(|l| l.contains("InitializeParams")),
            "InitializeParams should be exported"
        );
        assert!(
            exported_types.iter().any(|l| l.contains("ActivateParams")),
            "ActivateParams should be exported"
        );

        // Manifest types
        assert!(
            exported_types
                .iter()
                .any(|l| l.contains("PluginKindManifest")),
            "PluginKindManifest should be exported"
        );

        // Limits
        assert!(
            exported_types.iter().any(|l| l.contains("PluginLimits")),
            "PluginLimits should be exported"
        );

        assert!(
            exported_types.len() > 20,
            "expected >20 exported types, got {}",
            exported_types.len()
        );
    }
}
