//! Endpoint declarations for the process-wide runtimeLogLevel client namespace.

use crate::frontend::{FrontendEndpoint, FrontendHttpMethod, NO_PATH_PARAMS};
use ora_contracts::RUNTIME_LOG_LEVEL_PATH;

const NAMESPACE: &str = "runtimeLogLevel";

pub(super) const ENDPOINTS: &[FrontendEndpoint] = &[
    FrontendEndpoint {
        operation_name: "getRuntimeLogLevel",
        namespace: NAMESPACE,
        member_name: "get",
        method: FrontendHttpMethod::Get,
        path_template: RUNTIME_LOG_LEVEL_PATH,
        request_type: "GetRuntimeLogLevelRequest",
        response_type: "RuntimeLogLevelStateResponse",
        path_params: NO_PATH_PARAMS,
        has_json_body: false,
    },
    FrontendEndpoint {
        operation_name: "setRuntimeLogLevel",
        namespace: NAMESPACE,
        member_name: "set",
        method: FrontendHttpMethod::Put,
        path_template: RUNTIME_LOG_LEVEL_PATH,
        request_type: "SetRuntimeLogLevelRequest",
        response_type: "RuntimeLogLevelStateResponse",
        path_params: NO_PATH_PARAMS,
        has_json_body: true,
    },
];
