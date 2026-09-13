use axum::Extension;
use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use cognigraph_auth::{Scope, User};
use serde::Deserialize;

use crate::error::AppError;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/execute", post(execute_script))
}

#[derive(Deserialize)]
struct ExecuteRequest {
    script: String,
}

/// Dropping the HTTP future must signal its already-running blocking worker.
struct CancelOnDrop(cognigraph_lua::LuaExecutionControl);

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.cancel();
    }
}

async fn execute_script(
    State(state): State<AppState>,
    user: Option<Extension<User>>,
    Json(req): Json<ExecuteRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    // The engine runs on a blocking thread, OUTSIDE the request's
    // CURRENT_TENANT task-local — an unpinned handle would silently route
    // every graph call to the DEFAULT tenant (cross-tenant read/write).
    // Capture the tenant here, while still on the request path.
    let tenant = crate::tenancy::current_tenant();
    let context = crate::tenancy::TenantContext::capture(tenant.clone());
    #[cfg(not(feature = "enterprise"))]
    let backend = state.backend.clone();
    #[cfg(feature = "enterprise")]
    let backend: std::sync::Arc<dyn cognigraph_core::GraphBackend> = std::sync::Arc::new(
        crate::tenancy::TenantScoped::new(tenant.clone(), state.backend.clone()),
    );
    // Typed mutations and CGQL writes require documents:write. All query
    // text is parsed; roles cannot enable an opaque query passthrough.
    let instruction_limit = state.lua_instruction_limit;
    let allow_writes = user
        .as_ref()
        .is_some_and(|Extension(user)| user.role.grants(Scope::DocumentsWrite));
    let control = cognigraph_lua::LuaExecutionControl::new(state.cgql_budget);
    let _cancel_on_drop = CancelOnDrop(control.clone());
    // The supervisor survives an HTTP timeout long enough to join the worker
    // and invalidate cached results even after partial writes or a panic.
    let execution = tokio::spawn(context.scope(async move {
        let execution = tokio::task::spawn_blocking(move || {
            let handle = tokio::runtime::Handle::current();
            let engine = cognigraph_lua::LuaEngine::with_backend_control(
                backend,
                handle,
                allow_writes,
                control,
            )
            .map_err(|e| cognigraph_core::CogniGraphError::LuaError(e.to_string()))?;
            if instruction_limit > 0 {
                engine.set_instruction_limit(instruction_limit);
            }
            engine.execute(&req.script).map_err(|error| {
                let message = error.to_string();
                if cognigraph_lua::is_access_denied(&error)
                    || cognigraph_lua::is_forbidden_backend_access(&error)
                {
                    cognigraph_core::CogniGraphError::Forbidden(message)
                } else if cognigraph_lua::is_backend_unavailable(&error) {
                    cognigraph_core::CogniGraphError::ConnectionError(message)
                } else {
                    cognigraph_core::CogniGraphError::LuaError(message)
                }
            })
        })
        .await;

        // Lua is non-transactional: a script may mutate successfully and then
        // error. Sweep result rows after every write-capable attempt, including a
        // failed or panicked one, so cached documents cannot outlive that write.
        if allow_writes {
            state.invalidate_search_results().await;
        }
        execution
    }))
    .await
    .map_err(|e| cognigraph_core::CogniGraphError::LuaError(e.to_string()))?;
    let result =
        execution.map_err(|e| cognigraph_core::CogniGraphError::LuaError(e.to_string()))??;

    Ok(Json(serde_json::json!({
        "result": result,
    })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use axum::response::IntoResponse;
    use cognigraph_auth::Role;
    use cognigraph_core::GraphBackend;
    use cognigraph_native::NativeBackend;

    fn user(role: Role, tenant: &str) -> Extension<User> {
        Extension(User {
            key: format!("{role:?}"),
            username: format!("{role:?}"),
            role,
            tenant: tenant.into(),
        })
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn query_budget_applies_to_read_only_and_write_capable_scripts() {
        let mut state = AppState::new(NativeBackend::new());
        state.cgql_budget.max_source_rows = Some(3);
        for caller in [
            None,
            Some(user(Role::ScriptRunner, "default")),
            Some(user(Role::Editor, "default")),
        ] {
            let err = execute_script(
                State(state.clone()),
                caller.clone(),
                Json(ExecuteRequest {
                    script: r#"return graph.query("FOR n IN [1,2,3,4,5] RETURN n")"#.into(),
                }),
            )
            .await
            .unwrap_err();
            assert!(err.0.to_string().contains("row budget"), "{err:?}");
            let Json(result) = execute_script(
                State(state.clone()),
                caller,
                Json(ExecuteRequest {
                    script: r#"return graph.query("FOR n IN [1,2] RETURN n")"#.into(),
                }),
            )
            .await
            .unwrap();
            assert_eq!(result["result"], serde_json::json!([1, 2]));
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    #[cfg(feature = "enterprise")]
    async fn dropped_request_stops_worker_and_invalidates_the_callers_cache() {
        use crate::tenancy::{CURRENT_TENANT, RoutedBackend, RoutedCache, TenantRegistry};
        use cognigraph_cache::{CacheConfig, QueryCache};
        use std::sync::Arc;
        use std::time::Duration;

        let config = CacheConfig::default();
        let registry = Arc::new(TenantRegistry::new(
            Box::new(|_| Ok(Arc::new(NativeBackend::new()) as Arc<dyn GraphBackend>)),
            Some(config.clone()),
        ));
        let cache = Arc::new(RoutedCache::new(registry.clone(), config));
        let mut state = AppState::new_shared(Arc::new(RoutedBackend::new(registry.clone())));
        state.cache = Some(cache.clone());
        state.lua_instruction_limit = u32::MAX;
        let acme = registry.store("acme").unwrap();
        acme.ensure_collection("notes", cognigraph_core::CollectionType::Document)
            .await
            .unwrap();
        let task = tokio::spawn(CURRENT_TENANT.scope("acme".into(), async move {
            execute_script(
                State(state),
                Some(user(Role::Editor, "acme")),
                Json(ExecuteRequest {
                    script: r#"
                    graph.create_document("notes", {_key="started"})
                    local n=0; for i=1,1000000000 do n=n+i end
                    graph.create_document("notes", {_key="late"})
                    return n
                "#
                    .into(),
                }),
            )
            .await
        }));
        tokio::time::timeout(Duration::from_secs(2), async {
            while acme
                .get_document("notes", "started")
                .await
                .unwrap()
                .is_none()
            {
                tokio::time::sleep(Duration::from_millis(2)).await;
            }
        })
        .await
        .unwrap();
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        // Invalidation happens only AFTER joining the stopped worker, and must
        // retain the caller's tenant even though the HTTP future is gone.
        tokio::time::timeout(
            Duration::from_secs(2),
            CURRENT_TENANT.scope("acme".into(), async {
                while cache.result_generation() == 0 {
                    tokio::time::sleep(Duration::from_millis(2)).await;
                }
            }),
        )
        .await
        .unwrap();
        assert!(acme.get_document("notes", "late").await.unwrap().is_none());
        assert_eq!(
            cache.result_generation(),
            0,
            "default tenant cache untouched"
        );
    }

    /// Regression: the Lua engine runs on a blocking thread, outside the
    /// request's CURRENT_TENANT task-local — before TenantScoped, a
    /// tenant's script silently read and WROTE the default tenant's store.
    /// A write from inside tenant `acme`'s request scope must land in
    /// acme's store and nowhere else.
    #[tokio::test(flavor = "multi_thread")]
    #[cfg(feature = "enterprise")]
    async fn scripts_stay_inside_the_callers_tenant() {
        use crate::tenancy::{CURRENT_TENANT, RoutedBackend, TenantRegistry};
        use std::sync::Arc;

        let registry = Arc::new(TenantRegistry::new(
            Box::new(|_tenant| Ok(Arc::new(NativeBackend::new()) as Arc<dyn GraphBackend>)),
            None,
        ));
        let state =
            crate::state::AppState::new_shared(Arc::new(RoutedBackend::new(registry.clone())));

        CURRENT_TENANT
            .scope("acme".to_string(), async {
                let Json(response) = execute_script(
                    State(state.clone()),
                    Some(user(Role::Editor, "acme")),
                    Json(ExecuteRequest {
                        script: r#"return graph.create_document("notes", { body = "acme only" })"#
                            .into(),
                    }),
                )
                .await
                .unwrap();
                assert!(response["result"]["_id"].is_string(), "{response}");
            })
            .await;

        let acme = registry.store("acme").unwrap();
        assert_eq!(
            acme.list_documents("notes", None, None)
                .await
                .unwrap()
                .len(),
            1
        );
        let default = registry.store("default").unwrap();
        assert!(
            default.list_documents("notes", None, None).await.is_err(),
            "the default tenant must never see another tenant's Lua writes"
        );
    }

    /// ScriptRunner can execute read-only Lua but cannot use direct CRUD,
    /// edge, batch, or opaque backend-native query paths to bypass RBAC.
    /// Editor retains the authorized mutation behavior.
    #[tokio::test(flavor = "multi_thread")]
    async fn lua_mutations_follow_documents_write_scope() {
        let state = crate::state::AppState::new(NativeBackend::new());
        let script = r#"return graph.create_document("notes", { _key = "n1", body = "ok" })"#;

        let err = execute_script(
            State(state.clone()),
            Some(user(Role::ScriptRunner, "default")),
            Json(ExecuteRequest {
                script: script.into(),
            }),
        )
        .await
        .unwrap_err();
        assert!(
            err.0.to_string().contains("documents:write"),
            "ScriptRunner mutation error: {err:?}"
        );
        assert!(matches!(
            err.0,
            cognigraph_core::CogniGraphError::Forbidden(_)
        ));
        assert_eq!(err.into_response().status(), StatusCode::FORBIDDEN);
        assert!(
            state
                .backend
                .list_documents("notes", None, None)
                .await
                .is_err(),
            "denied Lua must not create the collection"
        );

        let Json(response) = execute_script(
            State(state.clone()),
            Some(user(Role::Editor, "default")),
            Json(ExecuteRequest {
                script: script.into(),
            }),
        )
        .await
        .unwrap();
        assert_eq!(response["result"]["_id"], "notes/n1");
        assert!(
            state
                .backend
                .get_document("notes", "n1")
                .await
                .unwrap()
                .is_some()
        );

        // Opaque queries must be rejected before any storage access.
        let opaque_backend =
            std::sync::Arc::new(cognigraph_core::contract::NoAccessBackend::default());
        let opaque_state = crate::state::AppState::new_shared(opaque_backend.clone());
        let err = execute_script(
            State(opaque_state.clone()),
            Some(user(Role::ScriptRunner, "default")),
            Json(ExecuteRequest {
                script: r#"return graph.query("REMOVE 'n1' IN notes", {})"#.into(),
            }),
        )
        .await
        .unwrap_err();
        assert!(err.0.to_string().contains("parsed CGQL support"), "{err:?}");

        let err = execute_script(
            State(opaque_state.clone()),
            Some(user(Role::Editor, "default")),
            Json(ExecuteRequest {
                script: r#"return graph.query("REMOVE 'n1' IN notes", {})"#.into(),
            }),
        )
        .await
        .unwrap_err();
        assert!(
            matches!(err.0, cognigraph_core::CogniGraphError::Forbidden(_)),
            "Editor must not receive opaque query authority: {err:?}"
        );

        let err = execute_script(
            State(opaque_state),
            Some(user(Role::Admin, "default")),
            Json(ExecuteRequest {
                script: r#"return graph.query("FOR d IN `_cognigraph_\\u006aobs` RETURN d", {})"#
                    .into(),
            }),
        )
        .await
        .unwrap_err();
        assert!(
            matches!(err.0, cognigraph_core::CogniGraphError::Forbidden(_)),
            "Admin must not receive opaque query authority: {err:?}"
        );
        assert!(
            err.0.to_string().contains("parsed CGQL support"),
            "Admin denial must not imply that the Admin role enables opaque queries: {err:?}"
        );
        opaque_backend.assert_unused();
    }

    /// Lua scripts talk to `state.backend` — the guarded facade — so a
    /// script can no more read `_users` than an HTTP caller can
    /// (decision_system_collections.md). Multi-thread runtime: the Lua
    /// engine blocks on backend futures from a blocking task.
    #[tokio::test(flavor = "multi_thread")]
    async fn system_collections_are_unreachable_from_scripts() {
        let raw = NativeBackend::new();
        raw.create_document(
            "_users",
            serde_json::json!({"_key": "admin", "password_hash": "h"}),
        )
        .await
        .unwrap();
        let state = crate::state::AppState::new(raw);

        for script in [
            r#"return graph.get_document("_users", "admin")"#,
            r#"return graph.find_documents("_users")"#,
            r#"return graph.query("FOR u IN _users RETURN u")"#,
        ] {
            let err = execute_script(
                State(state.clone()),
                None,
                Json(ExecuteRequest {
                    script: script.into(),
                }),
            )
            .await
            .unwrap_err();
            assert!(
                err.0.to_string().contains("system-reserved"),
                "script `{script}` gave {err:?}"
            );
            assert_eq!(err.into_response().status(), StatusCode::FORBIDDEN);
        }
    }
}
