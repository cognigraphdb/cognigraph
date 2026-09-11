//! Tenant identity.

use super::*;

pub(super) fn effective_incarnation(tenant: &cognigraph_auth::Tenant) -> String {
    if tenant.name == DEFAULT_TENANT {
        DEFAULT_TENANT.into()
    } else if tenant.incarnation.is_empty() {
        format!("legacy:{}", tenant.created_at)
    } else {
        tenant.incarnation.clone()
    }
}
pub(super) async fn tenant_identity_active(
    state: &AppState,
    tenant: &str,
    incarnation: &str,
) -> Result<bool, CogniGraphError> {
    let Some(auth) = &state.auth else {
        return Ok(true);
    };
    match auth.get_tenant(tenant).await? {
        Some(record) => {
            Ok(record.status == TenantStatus::Active
                && effective_incarnation(&record) == incarnation)
        }
        None => Ok(tenant == DEFAULT_TENANT && incarnation == DEFAULT_TENANT),
    }
}
pub(super) async fn runtime_identity_active(
    runtime: &JobRuntime,
    job: &JobRecord,
) -> Result<bool, CogniGraphError> {
    let Some(auth) = &runtime.auth else {
        return Ok(true);
    };
    match auth.get_tenant(&job.tenant).await? {
        Some(record) => Ok(record.status == TenantStatus::Active
            && effective_incarnation(&record) == job.tenant_incarnation),
        None => Ok(job.tenant == DEFAULT_TENANT && job.tenant_incarnation == DEFAULT_TENANT),
    }
}
