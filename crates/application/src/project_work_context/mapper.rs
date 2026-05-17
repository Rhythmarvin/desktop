use ora_contracts::{
    ProjectWorkContext as ContractProjectWorkContext,
    ProjectWorkContextSurface as ContractProjectWorkContextSurface,
};
use ora_domain::{
    ProjectWorkContext as DomainProjectWorkContext,
    ProjectWorkContextSurface as DomainProjectWorkContextSurface,
};

/// Converts one domain project work context into its shared contract projection.
pub fn map_project_work_context(context: DomainProjectWorkContext) -> ContractProjectWorkContext {
    ContractProjectWorkContext {
        id: context.id.to_string(),
        surface: map_project_work_context_surface(context.surface),
        window_id: context.window_id,
        project_id: context.project_id.to_string(),
        lease_expires_at: context.lease_expires_at,
    }
}

/// Converts one contract surface enum into the matching domain value.
pub fn map_project_work_context_surface_to_domain(
    surface: ContractProjectWorkContextSurface,
) -> DomainProjectWorkContextSurface {
    match surface {
        ContractProjectWorkContextSurface::Web => DomainProjectWorkContextSurface::Web,
        ContractProjectWorkContextSurface::Tauri => DomainProjectWorkContextSurface::Tauri,
    }
}

/// Converts one domain surface enum into the matching contract value.
fn map_project_work_context_surface(
    surface: DomainProjectWorkContextSurface,
) -> ContractProjectWorkContextSurface {
    match surface {
        DomainProjectWorkContextSurface::Web => ContractProjectWorkContextSurface::Web,
        DomainProjectWorkContextSurface::Tauri => ContractProjectWorkContextSurface::Tauri,
    }
}
