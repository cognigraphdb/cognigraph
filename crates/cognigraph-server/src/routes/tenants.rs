//! Tenant lifecycle (decision_multi_tenancy.md): control-store records
//! managed under the dedicated `TenantAdmin` scope — host-admin manages
//! tenants, it does not read their graphs. Deletion follows
//! decision_tenant_deletion.md: record + users go, the store handle is
//! evicted and the data files quarantined.

use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{Value, json};

use cognigraph_auth::{DEFAULT_TENANT, TenantStatus};
use cognigraph_core::CogniGraphError;

use crate::error::AppError;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list).post(create))
        .route("/{name}/admin", post(bootstrap_admin))
        .route("/{name}/quotas", post(update_quotas))
        .route("/{name}", post(update).delete(remove))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BootstrapAdminRequest {
    username: String,
    password: String,
}

async fn bootstrap_admin(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(req): Json<BootstrapAdminRequest>,
) -> Result<Json<Value>, AppError> {
    let _guard = state.tenant_lifecycle_lock.lock().await;
    require_isolated_tenant_storage(&state, &name)?;
    let user = provider(&state)?
        .bootstrap_tenant_admin(&name, &req.username, &req.password)
        .await?;
    Ok(Json(
        serde_json::to_value(user).map_err(CogniGraphError::from)?,
    ))
}

fn provider(state: &AppState) -> Result<&crate::state::AuthArc, AppError> {
    state.auth.as_ref().ok_or_else(|| {
        AppError(CogniGraphError::AuthError(
            "tenant management requires auth to be enabled".into(),
        ))
    })
}

fn require_isolated_tenant_storage(state: &AppState, name: &str) -> Result<(), AppError> {
    if name != DEFAULT_TENANT && state.tenant_registry.is_none() {
        return Err(AppError(CogniGraphError::ValidationError(
            "non-default tenant lifecycle requires isolated stores configured by COGNIGRAPH_DATA_DIR"
                .into(),
        )));
    }
    Ok(())
}

async fn list(State(state): State<AppState>) -> Result<Json<Value>, AppError> {
    let tenants = provider(&state)?.list_tenants().await?;
    let open = state
        .tenant_registry
        .as_ref()
        .map(|registry| registry.open_tenants())
        .unwrap_or_default();
    let tenants: Vec<Value> = tenants
        .into_iter()
        .map(|tenant| {
            let mut doc = serde_json::to_value(&tenant).unwrap_or_default();
            doc["store_open"] = json!(open.contains(&tenant.name));
            doc
        })
        .collect();
    Ok(Json(json!({
        "tenants": tenants,
        "count": tenants.len(),
        "open_stores": open.len(),
    })))
}

#[derive(Deserialize)]
struct CreateRequest {
    name: String,
    /// Extensible quota schema; M17 enforces `max_active_jobs`.
    quotas: Option<Value>,
}

async fn create(
    State(state): State<AppState>,
    Json(req): Json<CreateRequest>,
) -> Result<Json<Value>, AppError> {
    let _guard = state.tenant_lifecycle_lock.lock().await;
    require_isolated_tenant_storage(&state, &req.name)?;
    validate_quotas(&state, req.quotas.as_ref())?;
    let tenant = provider(&state)?
        .create_tenant(&req.name, req.quotas)
        .await?;
    state.jobs.resume_tenant(&req.name);
    state.promotions.resume_tenant(&req.name);
    Ok(Json(
        serde_json::to_value(tenant).map_err(CogniGraphError::from)?,
    ))
}

#[derive(Deserialize)]
struct UpdateRequest {
    status: TenantStatus,
}

async fn update(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(req): Json<UpdateRequest>,
) -> Result<Json<Value>, AppError> {
    let _guard = state.tenant_lifecycle_lock.lock().await;
    require_isolated_tenant_storage(&state, &name)?;
    let previous = provider(&state)?.get_tenant(&name).await?.ok_or_else(|| {
        AppError(CogniGraphError::DocumentNotFound {
            collection: "tenants".into(),
            key: name.clone(),
        })
    })?;
    if req.status == TenantStatus::Active && previous.status == TenantStatus::Suspended {
        // Keep admissions fenced across the control-store status flip. The
        // atomic resume/recovery path removes this fence only after rebuilding
        // durable active counts and the scheduler.
        state.jobs.pause_tenant(&name).await;
        state.promotions.pause_tenant(&name).await;
    }
    let tenant = provider(&state)?
        .set_tenant_status(&name, req.status)
        .await?;
    match tenant.status {
        TenantStatus::Suspended => {
            state.jobs.pause_tenant(&name).await;
            state.promotions.pause_tenant(&name).await;
        }
        TenantStatus::Active if previous.status == TenantStatus::Suspended => {
            let incarnation = if tenant.name == cognigraph_auth::DEFAULT_TENANT {
                cognigraph_auth::DEFAULT_TENANT.to_string()
            } else if tenant.incarnation.is_empty() {
                format!("legacy:{}", tenant.created_at)
            } else {
                tenant.incarnation.clone()
            };
            if let Err(error) = state
                .jobs
                .resume_and_recover(state.clone(), name.clone(), incarnation.clone())
                .await
            {
                provider(&state)?
                    .set_tenant_status(&name, TenantStatus::Suspended)
                    .await
                    .map_err(|rollback| {
                        AppError(CogniGraphError::BackendError(format!(
                            "tenant `{name}` activation recovery failed: {error}; status rollback failed: {rollback}"
                        )))
                    })?;
                return Err(AppError(error));
            }
            if let Err(error) = crate::tenancy::CURRENT_TENANT
                .scope(name.clone(), async {
                    let recovery = state.promotions.recover_tenant(&name, &incarnation).await;
                    // Recovery is a sequence of durable acts and may fail
                    // after an earlier repair committed. Always invalidate
                    // before propagating its result.
                    state.invalidate_search_results().await;
                    recovery.map(|_| ())
                })
                .await
            {
                state.jobs.pause_tenant(&name).await;
                provider(&state)?
                    .set_tenant_status(&name, TenantStatus::Suspended)
                    .await
                    .map_err(|rollback| {
                        AppError(CogniGraphError::BackendError(format!(
                            "tenant `{name}` promotion recovery failed: {error}; status rollback failed: {rollback}"
                        )))
                    })?;
                return Err(AppError(error));
            }
            state.promotions.resume_tenant(&name);
        }
        TenantStatus::Active => {}
    }
    Ok(Json(
        serde_json::to_value(tenant).map_err(CogniGraphError::from)?,
    ))
}

async fn update_quotas(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(update): Json<Value>,
) -> Result<Json<Value>, AppError> {
    let _guard = state.tenant_lifecycle_lock.lock().await;
    require_isolated_tenant_storage(&state, &name)?;
    let update = update.as_object().ok_or_else(|| {
        AppError(CogniGraphError::ValidationError(
            "tenant quotas must be a JSON object".into(),
        ))
    })?;
    let tenant = state
        .jobs
        .merge_tenant_quotas(provider(&state)?, &name, update)
        .await?;
    Ok(Json(
        serde_json::to_value(tenant).map_err(CogniGraphError::from)?,
    ))
}

fn validate_quotas(state: &AppState, quotas: Option<&Value>) -> Result<(), AppError> {
    let Some(quotas) = quotas else {
        return Ok(());
    };
    let object = quotas.as_object().ok_or_else(|| {
        AppError(CogniGraphError::ValidationError(
            "tenant quotas must be a JSON object".into(),
        ))
    })?;
    if let Some(value) = object.get("max_active_jobs") {
        let limit = value.as_u64().ok_or_else(|| {
            AppError(CogniGraphError::ValidationError(
                "quota `max_active_jobs` must be a non-negative integer".into(),
            ))
        })?;
        let host_limit = state.jobs.max_active_per_tenant() as u64;
        if limit > host_limit {
            return Err(AppError(CogniGraphError::ValidationError(format!(
                "quota `max_active_jobs` cannot exceed host limit {host_limit}"
            ))));
        }
    }
    Ok(())
}

async fn remove(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<Value>, AppError> {
    let _guard = state.tenant_lifecycle_lock.lock().await;
    require_isolated_tenant_storage(&state, &name)?;
    if name == DEFAULT_TENANT {
        return Err(AppError(CogniGraphError::AuthError(
            "the default tenant cannot be deleted — suspend it instead".into(),
        )));
    }
    // Fence new requests first, then wait until the worker has checkpointed
    // and released this tenant. Only then may the store be quarantined.
    if provider(&state)?.get_tenant(&name).await?.is_some() {
        provider(&state)?
            .set_tenant_status(&name, TenantStatus::Suspended)
            .await?;
    }
    state.jobs.pause_tenant(&name).await;
    state.promotions.pause_tenant(&name).await;
    let deleted = provider(&state)?.delete_tenant(&name).await?;
    let quarantined = match &state.tenant_registry {
        Some(registry) => registry.retire_store(&name)?,
        None => Vec::new(),
    };
    if deleted {
        state.jobs.retire_tenant(&name);
        state.promotions.retire_tenant(&name);
    }
    Ok(Json(
        json!({ "deleted": deleted, "quarantined": quarantined }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cognigraph_auth::AuthProvider;
    use cognigraph_native::NativeBackend;
    use std::sync::Arc;

    async fn state() -> AppState {
        let backend = Arc::new(NativeBackend::new());
        let auth = AuthProvider::new(backend.clone()).await.unwrap();
        let mut state = AppState::new_shared(backend);
        state.auth = Some(Arc::new(auth));
        state
    }

    #[tokio::test]
    async fn tenant_crud_over_routes() {
        use crate::tenancy::TenantRegistry;
        use cognigraph_core::GraphBackend;

        let mut state = state().await;
        state.tenant_registry = Some(Arc::new(TenantRegistry::new(
            Box::new(|_| Ok(Arc::new(NativeBackend::new()) as Arc<dyn GraphBackend>)),
            None,
        )));
        let Json(created) = create(
            State(state.clone()),
            Json(CreateRequest {
                name: "acme".into(),
                quotas: Some(serde_json::json!({ "max_documents": 100000 })),
            }),
        )
        .await
        .unwrap();
        assert_eq!(created["name"], "acme");
        assert_eq!(created["status"], "active");
        assert_eq!(created["quotas"]["max_documents"], 100000);

        let Json(admin) = bootstrap_admin(
            State(state.clone()),
            Path("acme".into()),
            Json(BootstrapAdminRequest {
                username: "acme-admin".into(),
                password: "acme-secret-123".into(),
            }),
        )
        .await
        .unwrap();
        assert_eq!(admin["role"], "admin");
        assert_eq!(admin["tenant"], "acme");
        assert!(
            bootstrap_admin(
                State(state.clone()),
                Path("acme".into()),
                Json(BootstrapAdminRequest {
                    username: "acme-admin-2".into(),
                    password: "acme-secret-456".into(),
                }),
            )
            .await
            .is_err()
        );

        let Json(quotas) = update_quotas(
            State(state.clone()),
            Path("acme".into()),
            Json(json!({ "max_active_jobs": 7 })),
        )
        .await
        .unwrap();
        assert_eq!(quotas["quotas"]["max_active_jobs"], 7);
        assert_eq!(quotas["quotas"]["max_documents"], 100000);
        assert!(
            update_quotas(
                State(state.clone()),
                Path("acme".into()),
                Json(json!({ "max_active_jobs": 101 })),
            )
            .await
            .is_err()
        );

        let Json(all) = list(State(state.clone())).await.unwrap();
        assert_eq!(all["count"], 1);

        let Json(updated) = update(
            State(state.clone()),
            Path("acme".into()),
            Json(UpdateRequest {
                status: TenantStatus::Suspended,
            }),
        )
        .await
        .unwrap();
        assert_eq!(updated["status"], "suspended");
        // The middleware gate now refuses this tenant's users.
        assert!(
            state
                .auth
                .as_ref()
                .unwrap()
                .tenant_allowed("acme")
                .await
                .is_err()
        );

        let Json(gone) = remove(State(state), Path("acme".into())).await.unwrap();
        assert_eq!(gone["deleted"], true);
    }

    #[tokio::test]
    async fn non_default_tenant_lifecycle_requires_isolated_stores() {
        let state = state().await;
        assert!(
            create(
                State(state.clone()),
                Json(CreateRequest {
                    name: "acme".into(),
                    quotas: None,
                }),
            )
            .await
            .is_err()
        );
        assert!(
            bootstrap_admin(
                State(state),
                Path("acme".into()),
                Json(BootstrapAdminRequest {
                    username: "acme-admin".into(),
                    password: "secret".into(),
                }),
            )
            .await
            .is_err()
        );
    }

    #[tokio::test]
    async fn delete_tenant_retires_its_store() {
        use crate::tenancy::{TenantRegistry, safe_tenant_file};
        use cognigraph_core::GraphBackend;

        let dir = std::env::temp_dir().join(format!("cg-tenants-route-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let factory_dir = dir.clone();
        let registry = Arc::new(
            TenantRegistry::new(
                Box::new(move |tenant| {
                    let path = factory_dir.join(safe_tenant_file(tenant)?);
                    Ok(Arc::new(NativeBackend::open(&path)?) as Arc<dyn GraphBackend>)
                }),
                None,
            )
            .with_data_dir(dir.clone()),
        );
        let mut state = state().await;
        state.tenant_registry = Some(registry.clone());

        let _ = create(
            State(state.clone()),
            Json(CreateRequest {
                name: "acme".into(),
                quotas: None,
            }),
        )
        .await
        .unwrap();
        registry
            .store("acme")
            .unwrap()
            .create_document("docs", serde_json::json!({ "_key": "d1" }))
            .await
            .unwrap();

        let Json(gone) = remove(State(state.clone()), Path("acme".into()))
            .await
            .unwrap();
        assert_eq!(gone["deleted"], true);
        assert_eq!(gone["quarantined"].as_array().unwrap().len(), 1);
        assert!(registry.open_tenants().is_empty());
        assert!(!dir.join("acme.redb").exists());

        // Same name recreated: a fresh empty store, no reattachment to
        // the deleted tenant's data.
        let _ = create(
            State(state.clone()),
            Json(CreateRequest {
                name: "acme".into(),
                quotas: None,
            }),
        )
        .await
        .unwrap();
        assert!(
            registry
                .store("acme")
                .unwrap()
                .get_document("docs", "d1")
                .await
                .unwrap()
                .is_none()
        );

        // The implicit default tenant cannot be deleted over the API.
        assert!(remove(State(state), Path("default".into())).await.is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
