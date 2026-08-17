//! Endpoint declarations for the developerMode client namespace.

use crate::frontend::{FrontendEndpoint, FrontendHttpMethod, NO_PATH_PARAMS};
use ora_contracts::DEVELOPER_MODE_PATH;

const NAMESPACE: &str = "developerMode";

pub(super) const ENDPOINTS: &[FrontendEndpoint] = &[
    FrontendEndpoint {
        operation_name: "getDeveloperMode",
        namespace: NAMESPACE,
        member_name: "get",
        method: FrontendHttpMethod::Get,
        path_template: DEVELOPER_MODE_PATH,
        request_type: "GetDeveloperModeRequest",
        response_type: "DeveloperModeResponse",
        path_params: NO_PATH_PARAMS,
        has_json_body: false,
    },
    FrontendEndpoint {
        operation_name: "setDeveloperMode",
        namespace: NAMESPACE,
        member_name: "set",
        method: FrontendHttpMethod::Put,
        path_template: DEVELOPER_MODE_PATH,
        request_type: "SetDeveloperModeRequest",
        response_type: "DeveloperModeResponse",
        path_params: NO_PATH_PARAMS,
        has_json_body: true,
    },
];
