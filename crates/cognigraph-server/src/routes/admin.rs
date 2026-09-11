//! Admin operations: hot backup and restore via full JSON snapshots.
//! Mounted behind the Admin scope; the snapshot format is the native
//! backend's export/import payload (see docs/operations.md).

#[cfg(feature = "enterprise")]
use axum::extract::Path;
use axum::extract::{DefaultBodyLimit, Extension, State};
use axum::routing::{get, post};
use axum::{Json, Router};

use cognigraph_auth::{Role, User};
use cognigraph_core::CogniGraphError;
#[cfg(feature = "enterprise")]
use serde::Deserialize;

use crate::error::AppError;
use crate::state::AppState;
#[cfg(feature = "enterprise")]
use crate::tenancy::current_tenant;

/// Imports carry whole databases; lift axum's 2 MB default body cap.
const IMPORT_BODY_LIMIT: usize = 1024 * 1024 * 1024;

pub fn router() -> Router<AppState> {
    let router = Router::new()
        .route("/export", get(export_snapshot))
        .route("/import", post(import_snapshot))
        .layer(DefaultBodyLimit::max(IMPORT_BODY_LIMIT))
        // Body limit only wraps the routes above it; logs takes no body.
        .route("/logs", get(recent_logs));
    #[cfg(feature = "enterprise")]
    let router = router
        .route(
            "/artifact-custody/evidence/{id}",
            get(artifact_recovery_plan),
        )
        .route("/jobs/status", get(job_status))
        .route("/jobs/reconcile", post(reconcile_jobs))
        .route("/jobs/archive", post(archive_jobs))
        .route(
            "/promotions/status",
            get(crate::routes::promotions::operator_status),
        )
        .route(
            "/promotions/reconcile",
            post(crate::routes::promotions::reconcile),
        )
        .route(
            "/promotions/recover",
            post(crate::routes::promotions::recover),
        );
    router
}

#[cfg(feature = "enterprise")]
async fn artifact_recovery_plan(
    State(state): State<AppState>,
    user: Option<Extension<User>>,
    Path(evidence_id): Path<String>,
) -> Result<Json<cognigraph_artifacts::ArtifactRecoveryPlanV1>, AppError> {
    crate::routes::governance::require_live_role(&state, user, Role::Admin).await?;
    let tenant = current_tenant();
    let incarnation = crate::jobs::JobManager::tenant_incarnation(&state, &tenant).await?;
    Ok(Json(
        crate::artifact_custody::derive_recovery_plan(
            &state.promotions,
            &tenant,
            &incarnation,
            &evidence_id,
            state.artifact_max_custody_bytes,
        )
        .await?,
    ))
}

#[cfg(feature = "enterprise")]
async fn job_status(State(state): State<AppState>) -> Result<Json<serde_json::Value>, AppError> {
    let tenant = current_tenant();
    let incarnation = crate::jobs::JobManager::tenant_incarnation(&state, &tenant).await?;
    Ok(Json(
        state
            .jobs
            .operator_status(&state, &tenant, &incarnation)
            .await?,
    ))
}

#[derive(Default, Deserialize)]
#[cfg(feature = "enterprise")]
struct ReconcileJobsRequest {
    #[serde(default)]
    dry_run: bool,
    limit: Option<usize>,
    cursor: Option<String>,
}

#[cfg(feature = "enterprise")]
async fn reconcile_jobs(
    State(state): State<AppState>,
    Json(request): Json<ReconcileJobsRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let tenant = current_tenant();
    let incarnation = crate::jobs::JobManager::tenant_incarnation(&state, &tenant).await?;
    let result = state
        .jobs
        .reconcile(
            &tenant,
            &incarnation,
            request.dry_run,
            request.limit.unwrap_or(100),
            request.cursor.as_deref(),
        )
        .await?;
    Ok(Json(
        serde_json::to_value(result).map_err(cognigraph_core::CogniGraphError::from)?,
    ))
}

#[derive(Default, Deserialize)]
#[cfg(feature = "enterprise")]
struct ArchiveJobsRequest {
    before_ms: Option<u64>,
    limit: Option<usize>,
    cursor: Option<String>,
    #[serde(default)]
    dry_run: bool,
    reason: Option<String>,
}

#[cfg(feature = "enterprise")]
async fn archive_jobs(
    State(state): State<AppState>,
    user: Option<Extension<User>>,
    Json(request): Json<ArchiveJobsRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let tenant = current_tenant();
    let incarnation = crate::jobs::JobManager::tenant_incarnation(&state, &tenant).await?;
    let result = state
        .jobs
        .archive_terminal(
            &tenant,
            &incarnation,
            crate::jobs::JobActor::request(user.as_ref().map(|Extension(user)| user)),
            request.before_ms,
            request.limit,
            request.cursor.as_deref(),
            request.dry_run,
            request.reason,
        )
        .await?;
    Ok(Json(
        serde_json::to_value(result).map_err(cognigraph_core::CogniGraphError::from)?,
    ))
}

/// Recent non-2xx responses (the console's server-logs panel). Scoped to
/// the caller's tenant plus untenanted events (pre-auth 401s, unknown
/// routes) — a tenant admin never sees another tenant's request paths.
/// With auth disabled there is no caller tenant, so everything shows.
async fn recent_logs(
    State(state): State<AppState>,
    user: Option<Extension<User>>,
) -> Json<serde_json::Value> {
    let tenant = user.as_ref().map(|Extension(user)| user.tenant.as_str());
    let events = state.recent_errors.snapshot(tenant);
    Json(serde_json::json!({
        "events": events,
        "count": events.len(),
    }))
}

async fn export_snapshot(
    State(state): State<AppState>,
    user: Option<Extension<User>>,
) -> Result<Json<serde_json::Value>, AppError> {
    require_snapshot_admin(user)?;
    Ok(Json(state.backend.export_snapshot().await?))
}

#[cfg(feature = "enterprise")]
async fn import_snapshot(
    State(state): State<AppState>,
    user: Option<Extension<User>>,
    Json(snapshot): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, AppError> {
    require_snapshot_admin(user)?;
    let _neuron_guard = state.neuron_lifecycle.lock().await;
    if snapshot_has_nonterminal_jobs(&snapshot) {
        return Err(AppError(
            cognigraph_core::CogniGraphError::DocumentConflict(
                "snapshot contains nonterminal durable jobs; complete or cancel them before import"
                    .into(),
            ),
        ));
    }
    let tenant = current_tenant();
    let incarnation = crate::jobs::JobManager::tenant_incarnation(&state, &tenant).await?;
    if !state
        .jobs
        .pause_tenant_if_idle(&tenant, &incarnation)
        .await?
    {
        return Err(AppError(
            cognigraph_core::CogniGraphError::DocumentConflict(
                "tenant has queued or running durable jobs; complete or cancel them before import"
                    .into(),
            ),
        ));
    }
    let import_result = state
        .promotions
        .import_snapshot(&tenant, &incarnation, &snapshot)
        .await;
    // Native snapshot import is additive and can have durable effects before
    // a later backend or post-import validation error. Clear cached reads
    // after every attempt, not only after the success path.
    if let Some(cache) = &state.cache {
        cache.clear().await;
    }
    if let Err(error) = import_result {
        if state.promotions.health().is_ok() {
            state.jobs.resume_tenant(&tenant);
        }
        return Err(AppError(error));
    }
    state
        .jobs
        .resume_and_recover(state.clone(), tenant.clone(), incarnation)
        .await?;
    let collections = snapshot
        .get("collections")
        .and_then(|c| c.as_object())
        .map(|c| c.len())
        .unwrap_or(0);
    Ok(Json(serde_json::json!({
        "status": "imported",
        "collections": collections,
    })))
}

#[cfg(not(feature = "enterprise"))]
async fn import_snapshot(
    State(state): State<AppState>,
    user: Option<Extension<User>>,
    Json(snapshot): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, AppError> {
    require_snapshot_admin(user)?;
    crate::edition::validate_snapshot(&snapshot)?;
    let result = state.backend.import_snapshot(&snapshot).await;
    if let Some(cache) = &state.cache {
        cache.clear().await;
    }
    result?;
    Ok(Json(
        serde_json::json!({"status": "imported", "collections": snapshot["collections"].as_object().map_or(0, |c| c.len())}),
    ))
}

fn require_snapshot_admin(user: Option<Extension<User>>) -> Result<(), AppError> {
    match user {
        Some(Extension(user)) if user.role == Role::Admin => Ok(()),
        _ => Err(AppError(CogniGraphError::Forbidden(
            "snapshot operations require authentication to be enabled and an Admin token".into(),
        ))),
    }
}

#[cfg(feature = "enterprise")]
fn snapshot_has_nonterminal_jobs(snapshot: &serde_json::Value) -> bool {
    for (collection, archived) in [
        ("_cognigraph_jobs", false),
        ("_cognigraph_job_archive", true),
    ] {
        let Some(collection) = snapshot.pointer(&format!("/collections/{collection}")) else {
            continue;
        };
        let Some(documents) = collection
            .get("documents")
            .and_then(serde_json::Value::as_object)
        else {
            return true;
        };
        if documents.values().any(|document| {
            serde_json::from_value::<crate::jobs::JobRecord>(document.clone()).map_or(true, |job| {
                !job.status.terminal()
                    || if archived {
                        crate::jobs::validate_archive_record(&job).is_err()
                    } else {
                        job.archived_at_ms.is_some()
                    }
            })
        }) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "enterprise")]
    use axum::response::IntoResponse;
    #[cfg(feature = "enterprise")]
    use cognigraph_auth::AuthProvider;
    use cognigraph_core::GraphBackend;
    use cognigraph_native::NativeBackend;
    use serde_json::json;
    #[cfg(feature = "enterprise")]
    use std::sync::Arc;

    fn admin() -> Option<Extension<User>> {
        Some(Extension(User {
            username: "admin".into(),
            role: Role::Admin,
            tenant: "default".into(),
            key: "admin".into(),
        }))
    }

    #[tokio::test]
    #[cfg(feature = "enterprise")]
    async fn artifact_recovery_plan_requires_a_live_admin_before_evidence_lookup() {
        let missing_evidence = "a".repeat(64);
        let backend = Arc::new(NativeBackend::new());
        let auth = Arc::new(AuthProvider::new(backend.clone()).await.unwrap());
        let admin = auth
            .create_user("custody-admin", "password", Role::Admin)
            .await
            .unwrap();
        let viewer = auth
            .create_user("custody-viewer", "password", Role::Viewer)
            .await
            .unwrap();
        let mut state = AppState::new_shared(backend);
        state.auth = Some(auth);

        let unauthenticated =
            artifact_recovery_plan(State(state.clone()), None, Path(missing_evidence.clone()))
                .await
                .unwrap_err()
                .into_response();
        assert_eq!(
            unauthenticated.status(),
            axum::http::StatusCode::UNAUTHORIZED
        );

        let wrong_role = artifact_recovery_plan(
            State(state.clone()),
            Some(Extension(viewer)),
            Path(missing_evidence.clone()),
        )
        .await
        .unwrap_err()
        .into_response();
        assert_eq!(wrong_role.status(), axum::http::StatusCode::FORBIDDEN);

        let missing = crate::tenancy::CURRENT_TENANT
            .scope(
                "default".into(),
                artifact_recovery_plan(
                    State(state.clone()),
                    Some(Extension(admin)),
                    Path(missing_evidence),
                ),
            )
            .await
            .unwrap_err()
            .into_response();
        assert_eq!(missing.status(), axum::http::StatusCode::NOT_FOUND);
        #[cfg(feature = "enterprise")]
        state.jobs.shutdown().await;
    }

    #[tokio::test]
    async fn export_import_roundtrip_via_routes() {
        let source = NativeBackend::new();
        source
            .create_document("docs", json!({"_key": "a", "title": "Alpha"}))
            .await
            .unwrap();
        source
            .upsert_edge("rels", "docs/a", "docs/a", "self", json!({}))
            .await
            .unwrap();
        let state = AppState::new(source);
        assert!(export_snapshot(State(state.clone()), None).await.is_err());
        let snapshot = export_snapshot(State(state), admin())
            .await
            .unwrap_or_else(|_| panic!("export failed"))
            .0;

        let target_state = AppState::new(NativeBackend::new());
        assert!(
            import_snapshot(State(target_state.clone()), None, Json(snapshot.clone()))
                .await
                .is_err()
        );
        let response = import_snapshot(State(target_state.clone()), admin(), Json(snapshot))
            .await
            .unwrap_or_else(|_| panic!("import failed"))
            .0;
        assert_eq!(response["status"], "imported");
        assert_eq!(response["collections"], 2);
        let doc = target_state
            .backend
            .get_document("docs", "a")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(doc["title"], "Alpha");
    }

    #[tokio::test]
    #[cfg(feature = "enterprise")]
    async fn snapshot_import_accepts_valid_terminal_jobs_and_rejects_unsafe_records() {
        let state = AppState::new(NativeBackend::new());
        let submission = state
            .jobs
            .submit(
                state.clone(),
                "default".into(),
                "default".into(),
                crate::jobs::JobActor::request(None),
                "snapshot-terminal-r1",
                crate::jobs::JobKind::ConstructEvaluate,
                json!({
                    "space_type": "snapshot-probe",
                    "eval": {
                        "space_id": "snapshot-probe",
                        "expected": [],
                        "forbidden": []
                    }
                }),
            )
            .await
            .unwrap();
        for _ in 0..200 {
            let job = state
                .jobs
                .get("default", "default", &submission.job.id)
                .await
                .unwrap();
            if job.status == crate::jobs::JobStatus::Succeeded {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(2)).await;
        }
        let snapshot = state.backend.export_snapshot().await.unwrap();
        assert!(!snapshot_has_nonterminal_jobs(&snapshot));

        let mut active = snapshot.clone();
        let documents = active
            .pointer_mut("/collections/_cognigraph_jobs/documents")
            .and_then(serde_json::Value::as_object_mut)
            .unwrap();
        documents.values_mut().next().unwrap()["status"] = json!("running");
        assert!(snapshot_has_nonterminal_jobs(&active));

        let mut marked_hot = snapshot.clone();
        marked_hot
            .pointer_mut("/collections/_cognigraph_jobs/documents")
            .and_then(serde_json::Value::as_object_mut)
            .unwrap()
            .values_mut()
            .next()
            .unwrap()["archived_at_ms"] = json!(1);
        assert!(snapshot_has_nonterminal_jobs(&marked_hot));

        let archived_without_event = snapshot
            .pointer("/collections/_cognigraph_jobs")
            .cloned()
            .unwrap();
        let mut archived_without_event = json!({
            "collections": {
                "_cognigraph_job_archive": archived_without_event
            }
        });
        archived_without_event
            .pointer_mut("/collections/_cognigraph_job_archive/documents")
            .and_then(serde_json::Value::as_object_mut)
            .unwrap()
            .values_mut()
            .next()
            .unwrap()["archived_at_ms"] = json!(1);
        assert!(snapshot_has_nonterminal_jobs(&archived_without_event));

        for malformed in [
            json!({"status": "succeeded"}),
            json!({"status": "unknown"}),
            json!({}),
        ] {
            let snapshot = json!({
                "collections": {
                    "_cognigraph_jobs": {"documents": {"job-1": malformed}}
                }
            });
            assert!(snapshot_has_nonterminal_jobs(&snapshot));
        }
        assert!(!snapshot_has_nonterminal_jobs(&json!({
            "collections": {"documents": {"documents": {}}}
        })));
        #[cfg(feature = "enterprise")]
        state.jobs.shutdown().await;
    }

    /// The logs endpoint shows the caller's tenant events plus untenanted
    /// ones, and hides other tenants' — the cross-tenant path leak the
    /// design forbids.
    #[tokio::test]
    async fn logs_are_scoped_to_the_callers_tenant() {
        use crate::hardening::ErrorEvent;
        use cognigraph_auth::{Role, User};

        let state = AppState::new(NativeBackend::new());
        let event = |tenant: Option<&str>, path: &str| ErrorEvent {
            seq: 0,
            at: 0,
            method: "GET".into(),
            path: path.into(),
            status: 404,
            latency_ms: 1,
            message: "not found".into(),
            tenant: tenant.map(str::to_string),
        };
        state
            .recent_errors
            .record(event(Some("acme"), "/api/documents/acme_secret/x"));
        state
            .recent_errors
            .record(event(Some("dailymed"), "/api/documents/labels/y"));
        state.recent_errors.record(event(None, "/api/auth/login")); // a 401, no tenant

        let caller = User {
            username: "admin".into(),
            role: Role::Admin,
            tenant: "acme".into(),
            key: "k".into(),
        };
        let Json(body) = recent_logs(State(state.clone()), Some(Extension(caller))).await;
        let paths: Vec<&str> = body["events"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["path"].as_str().unwrap())
            .collect();
        assert!(paths.contains(&"/api/documents/acme_secret/x"), "{paths:?}");
        assert!(paths.contains(&"/api/auth/login"), "{paths:?}");
        assert!(
            !paths.contains(&"/api/documents/labels/y"),
            "acme must not see dailymed's paths: {paths:?}"
        );

        // No caller (auth disabled) sees everything.
        let Json(all) = recent_logs(State(state), None).await;
        assert_eq!(all["count"], 3);
    }
}
