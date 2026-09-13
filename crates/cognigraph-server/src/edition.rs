//! Build capabilities shared by startup and request admission.

pub const OPENAPI: &str = include_str!(concat!(env!("OUT_DIR"), "/openapi.json"));

#[cfg(not(feature = "enterprise"))]
pub fn required(capability: &str) -> crate::error::AppError {
    cognigraph_core::CogniGraphError::EnterpriseFeatureRequired(capability.into()).into()
}

pub fn require_tenant(tenant: &str) -> Result<(), crate::error::AppError> {
    #[cfg(not(feature = "enterprise"))]
    if tenant != cognigraph_auth::DEFAULT_TENANT {
        return Err(required("multi-tenancy"));
    }
    #[cfg(feature = "enterprise")]
    let _ = tenant;
    Ok(())
}

#[cfg(not(feature = "enterprise"))]
fn enterprise_collection(name: &str) -> bool {
    name.starts_with("_cognigraph_")
        || crate::system_collections::is_managed_collection(name)
        || crate::system_collections::is_generated_collection(name)
}

/// A Community process must not mutate a store whose derived or governed state
/// needs the Enterprise lifecycle fences. Empty collections retain their format.
#[cfg(not(feature = "enterprise"))]
pub async fn validate_store(
    backend: &dyn cognigraph_core::GraphBackend,
) -> cognigraph_core::Result<()> {
    for collection in backend.list_collections().await? {
        if enterprise_collection(&collection.name)
            && !backend
                .list_documents(&collection.name, Some(1), None)
                .await?
                .is_empty()
        {
            return Err(cognigraph_core::CogniGraphError::EnterpriseFeatureRequired(
                "the existing store contains governed or generated records; open it with an Enterprise build".into(),
            ));
        }
    }
    Ok(())
}

#[cfg(not(feature = "enterprise"))]
pub fn validate_snapshot(snapshot: &serde_json::Value) -> Result<(), crate::error::AppError> {
    let collections = snapshot
        .get("collections")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| {
            cognigraph_core::CogniGraphError::ValidationError(
                "snapshot requires a collections object".into(),
            )
        })?;
    for (name, collection) in collections {
        if enterprise_collection(name)
            && collection
                .get("documents")
                .and_then(serde_json::Value::as_object)
                .is_none_or(|docs| !docs.is_empty())
        {
            return Err(required("importing governed or generated records"));
        }
    }
    Ok(())
}
