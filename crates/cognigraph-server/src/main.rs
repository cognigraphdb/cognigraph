#[cfg(feature = "enterprise")]
mod artifact_attestations;
#[cfg(feature = "enterprise")]
mod artifact_cas;
#[cfg(feature = "enterprise")]
mod artifact_consumption;
#[cfg(feature = "enterprise")]
mod artifact_custody;
mod auth_middleware;
#[cfg(all(test, not(feature = "enterprise")))]
mod community_tests;
mod config;
mod container_init;
mod edition;
mod error;
#[cfg(feature = "enterprise")]
mod governance;
#[cfg(all(test, feature = "enterprise"))]
mod governance_tests;
mod hardening;
#[cfg(feature = "enterprise")]
mod jobs;
#[cfg(feature = "enterprise")]
mod materialized_repairs;
#[cfg(feature = "enterprise")]
mod neuron_lifecycle;
#[cfg(test)]
mod openapi_drift;
#[cfg(feature = "enterprise")]
mod promotions;
#[cfg(feature = "enterprise")]
mod refusals;
mod routes;
#[cfg(feature = "enterprise")]
mod semantic_repairs;
#[cfg(feature = "enterprise")]
mod side_views;
mod spa;
mod state;
mod system_collections;
#[cfg(feature = "enterprise")]
mod tenancy;
#[cfg(not(feature = "enterprise"))]
#[path = "tenancy/community.rs"]
mod tenancy;

use axum::Router;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

use cognigraph_cache::{CacheConfig, InMemoryCache};
use cognigraph_core::GraphBackend;
use cognigraph_native::NativeBackend;

use crate::config::Config;
use crate::state::AppState;

fn main() {
    // Root start-up in the container must finish while the process is still
    // single-threaded, so it runs before the Tokio runtime exists (CG-81).
    container_init::apply_or_exit();
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Tokio runtime")
        .block_on(serve());
}

async fn serve() {
    // Load .env file if present (before reading config)
    dotenvy::dotenv().ok();

    let config = Config::from_env();
    config
        .validate()
        .unwrap_or_else(|error| panic!("Invalid CogniGraph configuration: {error}"));

    // Validate completion and judge lanes before opening stores or binding HTTP.
    // An unconfigured lane is optional; explicit configuration errors are fatal.
    #[cfg(feature = "enterprise")]
    let completion_config = cognigraph_embeddings::completion::CompletionConfig::from_env();
    #[cfg(feature = "enterprise")]
    let completion = completion_config
        .completion_provider()
        .unwrap_or_else(|error| panic!("Invalid completion configuration: {error:#}"));
    #[cfg(feature = "enterprise")]
    let sideviews_completion = completion_config
        .sideviews_provider()
        .unwrap_or_else(|error| panic!("Invalid side-view configuration: {error:#}"));
    #[cfg(feature = "enterprise")]
    let judge = completion_config
        .judge_provider()
        .unwrap_or_else(|error| panic!("Invalid judge configuration: {error:#}"));
    #[cfg(feature = "enterprise")]
    let judge_partner = completion_config
        .judge_partner_provider()
        .unwrap_or_else(|error| panic!("Invalid partner judge configuration: {error:#}"));

    let subscriber = tracing_subscriber::fmt().with_env_filter(
        EnvFilter::from_default_env().add_directive("cognigraph=info".parse().unwrap()),
    );
    if config.log_format == "json" {
        subscriber.json().init();
    } else {
        subscriber.init();
    }

    // Multi-tenant mode (decision_multi_tenancy.md M2): one store per
    // tenant under COGNIGRAPH_DATA_DIR, control store separate, and the
    // RoutedBackend facade as the single structural choke point. Auth
    // over the control store is wired further below (it replaces the
    // data-store AuthProvider in this mode).
    #[cfg(feature = "enterprise")]
    let tenant_registry: Option<std::sync::Arc<tenancy::TenantRegistry>> =
        config.data_dir.as_ref().map(|dir| {
            let dir = std::path::PathBuf::from(dir);
            std::fs::create_dir_all(&dir).expect("Failed to create COGNIGRAPH_DATA_DIR");
            let mode = match config.vector_mode.as_str() {
                "sidecar" => cognigraph_native::VectorMode::Sidecar,
                _ => cognigraph_native::VectorMode::Embedded,
            };
            let storage = match config.storage_mode.as_str() {
                "paged" => cognigraph_native::StorageMode::Paged,
                _ => cognigraph_native::StorageMode::Resident,
            };
            let cache_bytes = config.cache_bytes;
            let factory_dir = dir.clone();
            let cache_config = config
                .cache_enabled
                .then(cognigraph_cache::CacheConfig::from_env);
            tracing::info!(dir = %dir.display(), "Multi-tenant mode: per-tenant stores");
            std::sync::Arc::new(
                tenancy::TenantRegistry::new(
                    Box::new(move |tenant| {
                        let file = tenancy::safe_tenant_file(tenant)?;
                        let path = factory_dir.join(file);
                        let backend = cognigraph_native::NativeBackend::open_with_modes(
                            path.to_string_lossy().as_ref(),
                            mode,
                            storage,
                            cache_bytes,
                        )?;
                        Ok(std::sync::Arc::new(backend) as std::sync::Arc<dyn GraphBackend>)
                    }),
                    cache_config,
                )
                .with_data_dir(dir),
            )
        });

    // The raw single-store backend handle. AppState wraps its backend in
    // the system-collection guard (state.rs), so the AuthProvider — which
    // legitimately owns `_users`/`_tokens`/`_tenants` — must be built on
    // this unguarded handle, never on `state.backend`.
    let mut single_store_backend: Option<std::sync::Arc<dyn GraphBackend>> = None;
    #[cfg(feature = "enterprise")]
    let registry_state = tenant_registry.as_ref().map(|registry| {
        let mut state = AppState::new_shared(std::sync::Arc::new(tenancy::RoutedBackend::new(
            registry.clone(),
        )));
        state.tenant_registry = Some(registry.clone());
        state
    });
    #[cfg(not(feature = "enterprise"))]
    let registry_state: Option<AppState> = None;
    let mut state = if let Some(state) = registry_state {
        state
    } else {
        let backend = match &config.native_path {
            Some(path) => {
                let mode = match config.vector_mode.as_str() {
                    "sidecar" => cognigraph_native::VectorMode::Sidecar,
                    _ => cognigraph_native::VectorMode::Embedded,
                };
                let storage = match config.storage_mode.as_str() {
                    "paged" => cognigraph_native::StorageMode::Paged,
                    _ => cognigraph_native::StorageMode::Resident,
                };
                tracing::info!(path, vector_mode = ?mode, storage_mode = ?storage, "Opening native backend with persistent storage");
                NativeBackend::open_with_modes(path, mode, storage, config.cache_bytes)
                    .unwrap_or_else(|e| panic!("Failed to open native storage at {path}: {e}"))
            }
            None => {
                tracing::info!("Opening native in-memory backend");
                NativeBackend::new()
            }
        };
        let backend: std::sync::Arc<dyn GraphBackend> = std::sync::Arc::new(backend);
        #[cfg(not(feature = "enterprise"))]
        edition::validate_store(backend.as_ref())
            .await
            .unwrap_or_else(|error| panic!("{error}"));
        single_store_backend = Some(backend.clone());
        AppState::new_shared(backend)
    };

    #[cfg(feature = "enterprise")]
    {
        let (artifact_cas, artifact_executable_digest) = match config.artifact_source.as_str() {
            "disabled" => {
                tracing::info!("Verified artifact consumption disabled");
                (None, None)
            }
            "local-cas" => {
                let root = config
                    .artifact_cas_root
                    .as_deref()
                    .expect("validated local artifact CAS root");
                let cas = artifact_cas::LocalArtifactCas::open(
                    root,
                    config.artifact_max_evaluation_bytes,
                )
                .unwrap_or_else(|error| {
                    panic!("Failed to open local artifact CAS at {root}: {error}")
                });
                tracing::info!(
                    root,
                    max_evaluation_bytes = config.artifact_max_evaluation_bytes,
                    "Verified artifact consumption uses the local read-only CAS"
                );
                let executable_digest = artifact_consumption::current_executable_digest()
                    .await
                    .unwrap_or_else(|error| {
                        panic!("Failed to pin the running executable identity: {error}")
                    });
                (Some(std::sync::Arc::new(cas)), Some(executable_digest))
            }
            _ => unreachable!("artifact source was validated before server initialization"),
        };
        state.artifact_cas = artifact_cas;
        state.artifact_max_custody_bytes = config.artifact_max_custody_bytes;
        state.artifact_executable_digest = artifact_executable_digest;
    }

    // Configure embedding provider
    match config.embedding_provider.as_str() {
        "openai" => {
            let api_key = config
                .openai_api_key
                .expect("OPENAI_API_KEY required when COGNIGRAPH_EMBEDDING_PROVIDER=openai");
            let provider = cognigraph_embeddings::openai::OpenAiProvider::new(
                api_key,
                config.openai_base_url,
                config.embedding_model,
            )
            .expect("Failed to create OpenAI embedding provider");
            tracing::info!("Embedding provider: OpenAI");
            state = state.with_embedder(provider);
        }
        "ollama" => {
            let provider = cognigraph_embeddings::ollama::OllamaProvider::new(
                config.ollama_base_url,
                config.embedding_model,
            )
            .expect("Failed to create Ollama embedding provider");
            tracing::info!("Embedding provider: Ollama");
            state = state.with_embedder(provider);
        }
        "gemini" => {
            let api_key = config
                .gemini_api_key
                .expect("GEMINI_API_KEY required when COGNIGRAPH_EMBEDDING_PROVIDER=gemini");
            let provider = cognigraph_embeddings::gemini::GeminiProvider::new(
                api_key,
                None,
                config.embedding_model,
                None,
            )
            .expect("Failed to create Gemini embedding provider");
            tracing::info!("Embedding provider: Gemini");
            state = state.with_embedder(provider);
        }
        "none" | "" => {
            tracing::info!("No embedding provider configured (semantic search disabled)");
        }
        other => {
            tracing::warn!(
                provider = other,
                "Unknown COGNIGRAPH_EMBEDDING_PROVIDER, semantic search disabled"
            );
        }
    }

    #[cfg(feature = "enterprise")]
    if let Some(provider) = completion {
        tracing::info!(
            model = provider.model_name(),
            "Completion provider configured"
        );
        state.completion = Some(std::sync::Arc::from(provider));
    }
    #[cfg(feature = "enterprise")]
    if let Some(provider) = sideviews_completion {
        tracing::info!(
            model = provider.model_name(),
            "Side-view generation provider configured"
        );
        state = state.with_sideviews_completion(std::sync::Arc::from(provider));
    }

    // Review judge (POST /api/construct/review): a dedicated model via
    // COGNIGRAPH_JUDGE_MODEL for separation of duties; otherwise the
    // review route falls back to the completion provider.
    #[cfg(feature = "enterprise")]
    if let Some(provider) = judge {
        tracing::info!("Review judge: dedicated model (COGNIGRAPH_JUDGE_MODEL)");
        state.judge = Some(std::sync::Arc::from(provider));
    }

    // Agreement-lane partner judge (Lane A+): a second model whose
    // measured blind spots are disjoint from the primary's
    // (decision_agreement_lane.md). Absent = Lane A+ degrades to queue.
    #[cfg(feature = "enterprise")]
    if let Some(provider) = judge_partner {
        tracing::info!("Agreement-lane partner judge: COGNIGRAPH_JUDGE_PARTNER_MODEL");
        state.judge_partner = Some(std::sync::Arc::from(provider));
    }

    // Per-query CGQL budgets (0 = unlimited)
    state.cgql_budget = cognigraph_query::ExecutionBudget {
        max_source_rows: (config.cgql_max_source_rows > 0).then_some(config.cgql_max_source_rows),
        time_budget_ms: (config.cgql_time_budget_ms > 0).then_some(config.cgql_time_budget_ms),
    };
    if state.cgql_budget.max_source_rows.is_some() || state.cgql_budget.time_budget_ms.is_some() {
        tracing::info!(
            max_source_rows = ?state.cgql_budget.max_source_rows,
            time_budget_ms = ?state.cgql_budget.time_budget_ms,
            "CGQL execution budgets enabled"
        );
    }

    state.lua_instruction_limit = config.lua_instruction_limit;
    state.token_ttl_secs = config.token_ttl_secs;
    #[cfg(feature = "enterprise")]
    {
        state.governance_root_public_key = config.governance_root_public_key.clone();
        state
            .promotions
            .configure_governance_root(config.governance_root_public_key.as_deref())
            .expect("invalid COGNIGRAPH_GOVERNANCE_ROOT_PUBLIC_KEY");
        state.jobs.set_batch_size(config.job_ingest_batch_size);
        state.jobs.configure_governance(
            config.job_max_active_total,
            config.job_max_active_per_tenant,
            config.job_retention_secs,
            config.job_archive_batch_size,
        );
    }

    // Configure auth. In multi-tenant mode the control store holds
    // _users/_tokens/_tenants SEPARATE from any tenant's data (D2):
    // restoring or deleting a tenant store never touches credentials.
    if config.auth_enabled {
        let auth_backend: std::sync::Arc<dyn GraphBackend> = match &config.data_dir {
            #[cfg(feature = "enterprise")]
            Some(dir) => {
                let path = std::path::Path::new(dir).join("_control.redb");
                std::sync::Arc::new(
                    NativeBackend::open(path.to_string_lossy().as_ref())
                        .expect("Failed to open the tenancy control store"),
                )
            }
            _ => single_store_backend
                .clone()
                .expect("single-store mode keeps a raw backend handle for auth"),
        };
        let auth = cognigraph_auth::AuthProvider::new(auth_backend)
            .await
            .expect("Failed to initialize auth collections");
        if let Some(password) = &config.admin_password {
            auth.bootstrap_admin(password)
                .await
                .expect("Failed to bootstrap admin user");
        }
        if let Some(password) = &config.host_admin_password {
            auth.bootstrap_host_admin(password)
                .await
                .expect("Failed to bootstrap host-admin user");
        }
        tracing::info!("Auth enabled (bearer tokens + RBAC)");
        state = state.with_auth(auth);
        if let Some(secret) = &config.jwt_secret {
            tracing::info!(ttl_secs = config.jwt_ttl_secs, "JWT sessions enabled");
            state = state.with_jwt(secret.clone(), config.jwt_ttl_secs);
        }
    }

    // Configure query cache. Multi-tenant mode: per-tenant caches via
    // the RoutedCache facade (D3 — a shared cache is a cross-tenant
    // side channel), always in-memory per tenant.
    #[cfg(feature = "enterprise")]
    if config.cache_enabled && tenant_registry.is_some() {
        let cache_config = CacheConfig::from_env();
        tracing::info!("Query cache enabled (per-tenant, in-memory)");
        state = state.with_cache(tenancy::RoutedCache::new(
            tenant_registry.clone().expect("registry present"),
            cache_config,
        ));
    }
    if config.cache_enabled && state.cache.is_none() {
        let cache_config = CacheConfig::from_env();
        tracing::info!(
            backend = %cache_config.backend,
            ttl_secs = cache_config.ttl_secs,
            max_entries = cache_config.max_entries,
            similarity_floor = cache_config.similarity_floor,
            strong_threshold = cache_config.strong_threshold,
            "Query cache enabled"
        );
        match cache_config.backend.as_str() {
            "persistent" => {
                let path = cache_config
                    .path
                    .clone()
                    .expect("COGNIGRAPH_QUERY_CACHE_PATH required when COGNIGRAPH_QUERY_CACHE_BACKEND=persistent");
                let cache = cognigraph_cache::PersistentCache::open(cache_config, &path)
                    .unwrap_or_else(|e| panic!("{e}"));
                state = state.with_cache(cache);
            }
            "memory" => {
                state = state.with_cache(InMemoryCache::new(cache_config));
            }
            other => {
                tracing::warn!(
                    backend = other,
                    "Unknown COGNIGRAPH_QUERY_CACHE_BACKEND, using the in-memory cache"
                );
                state = state.with_cache(InMemoryCache::new(cache_config));
            }
        }
    }

    // Reconcile the durable queue only after auth, tenant routing, and cache
    // wiring are complete. Running records mean the prior process stopped
    // before its terminal checkpoint and are returned to queued here.
    #[cfg(feature = "enterprise")]
    {
        let mut recovery_targets = Vec::new();
        if let Some(auth) = &state.auth {
            match auth.list_tenants().await {
                Ok(tenants) => {
                    let mut saw_default = false;
                    for tenant in tenants {
                        if tenant.name == cognigraph_auth::DEFAULT_TENANT {
                            saw_default = true;
                        }
                        let paused = tenant.status == cognigraph_auth::TenantStatus::Suspended;
                        let incarnation = if tenant.name == cognigraph_auth::DEFAULT_TENANT {
                            cognigraph_auth::DEFAULT_TENANT.to_string()
                        } else if tenant.incarnation.is_empty() {
                            format!("legacy:{}", tenant.created_at)
                        } else {
                            tenant.incarnation
                        };
                        recovery_targets.push((tenant.name, incarnation, paused));
                    }
                    if !saw_default {
                        recovery_targets.push((
                            cognigraph_auth::DEFAULT_TENANT.to_string(),
                            cognigraph_auth::DEFAULT_TENANT.to_string(),
                            false,
                        ));
                    }
                }
                Err(error) => {
                    panic!("failed to enumerate tenants for durable job recovery: {error}")
                }
            }
        } else {
            recovery_targets.push((
                cognigraph_auth::DEFAULT_TENANT.to_string(),
                cognigraph_auth::DEFAULT_TENANT.to_string(),
                false,
            ));
        }
        for (tenant, incarnation, paused) in recovery_targets {
            if paused {
                // Suspended work still consumes global admission capacity. Fence
                // it before rebuilding the authoritative active set so a later
                // activation cannot create an admission window.
                state.jobs.pause_tenant(&tenant).await;
            }
            match state
                .jobs
                .recover_tenant(state.clone(), tenant.clone(), incarnation.clone())
                .await
            {
                Ok(count) if count > 0 => {
                    tracing::info!(tenant, count, paused, "recovered durable jobs")
                }
                Ok(_) => {}
                Err(error) => {
                    panic!("durable job recovery failed for tenant `{tenant}`: {error}")
                }
            }
            match tenancy::CURRENT_TENANT
                .scope(
                    tenant.clone(),
                    state.promotions.recover_tenant(&tenant, &incarnation),
                )
                .await
            {
                Ok(count) if count > 0 => {
                    tracing::info!(tenant, count, "repaired derived promotion heads")
                }
                Ok(_) => {}
                Err(error) => {
                    panic!("promotion recovery failed for tenant `{tenant}`: {error}")
                }
            }
        }
    }

    use auth_middleware::ScopePolicy;
    use cognigraph_auth::Scope;
    let guard = |read: Scope, write: Scope| {
        axum::middleware::from_fn_with_state(
            (state.clone(), ScopePolicy { read, write }),
            auth_middleware::check,
        )
    };
    // The application API lives under `/api` so the UI can own `/` and serve
    // its built assets without shadowing an endpoint. Operational routes
    // (`/health`, `/metrics`) and the spec (`/openapi.yaml`) stay at the root by
    // convention — Prometheus scrapers and load balancers expect fixed paths,
    // and they are exact routes matched before any future SPA fallback.
    let mut api = Router::new()
        .nest("/auth", routes::auth::router(state.clone()))
        .nest(
            "/cache",
            routes::cache::router().route_layer(guard(Scope::Admin, Scope::Admin)),
        )
        .nest(
            "/users",
            routes::users::router().route_layer(guard(Scope::Admin, Scope::Admin)),
        )
        .nest(
            "/admin",
            routes::admin::router().route_layer(guard(Scope::Admin, Scope::Admin)),
        )
        .nest(
            "/tenants",
            routes::tenants::router().route_layer(guard(Scope::TenantAdmin, Scope::TenantAdmin)),
        )
        .nest(
            "/documents",
            routes::documents::router()
                .route_layer(guard(Scope::DocumentsRead, Scope::DocumentsWrite)),
        )
        .nest(
            "/collections",
            routes::collections::router()
                .route_layer(guard(Scope::DocumentsRead, Scope::DocumentsWrite)),
        )
        .nest(
            "/graph",
            routes::graph::router().route_layer(guard(Scope::GraphRead, Scope::GraphWrite)),
        )
        .nest(
            "/search",
            routes::search::router().route_layer(guard(Scope::Search, Scope::Search)),
        )
        .nest(
            "/batch",
            routes::batch::router()
                .route_layer(guard(Scope::DocumentsWrite, Scope::DocumentsWrite)),
        )
        .nest(
            "/lua",
            routes::lua::router().route_layer(guard(Scope::LuaExecute, Scope::LuaExecute)),
        );
    #[cfg(feature = "enterprise")]
    {
        api = api
            .nest(
                "/jobs",
                routes::jobs::router().route_layer(guard(Scope::GraphRead, Scope::GraphWrite)),
            )
            .nest(
                "/promotions",
                routes::promotions::router()
                    .route_layer(guard(Scope::PromotionRead, Scope::PromotionDecide)),
            )
            .nest(
                "/governance",
                routes::governance::router()
                    .route_layer(guard(Scope::PromotionRead, Scope::PromotionRead)),
            )
            .nest(
                "/semantic-repairs",
                routes::semantic_repairs::router()
                    .route_layer(guard(Scope::PromotionRead, Scope::PromotionRead)),
            )
            .nest(
                "/neurons",
                routes::neurons::router().route_layer(guard(Scope::GraphRead, Scope::GraphWrite)),
            )
            .nest(
                "/construct",
                routes::construct::router()
                    .route_layer(guard(Scope::GraphRead, Scope::GraphWrite))
                    // evaluate is a POST (spec in the body) but read-only in
                    // effect — viewers may measure.
                    .merge(
                        routes::construct::eval_router()
                            .route_layer(guard(Scope::GraphRead, Scope::GraphRead)),
                    ),
            )
            .nest(
                "/sideviews",
                routes::sideviews::router().route_layer(guard(Scope::GraphRead, Scope::GraphWrite)),
            );
    }
    if config.cgql_mutations_enabled {
        tracing::info!("CGQL mutations enabled (POST /api/query, read-write)");
        api = api.nest(
            "/query",
            routes::query::router()
                .route_layer(guard(Scope::DocumentsWrite, Scope::DocumentsWrite)),
        );
    }
    // Unknown /api paths answer JSON 404 — they must never fall through
    // to the SPA fallback and come back as 200 text/html.
    let api = api.fallback(spa::api_not_found);
    let mut app = Router::new()
        .nest("/health", routes::health::router())
        .nest("/api", api);
    app = app.route(
        "/openapi.yaml",
        axum::routing::get(|| async {
            (
                [(axum::http::header::CONTENT_TYPE, "application/yaml")],
                edition::OPENAPI,
            )
        }),
    );
    let metrics = std::sync::Arc::new(hardening::Metrics::new());
    let metrics_route = metrics.clone();
    #[cfg(feature = "enterprise")]
    let job_metrics_route = state.jobs.clone();
    #[cfg(feature = "enterprise")]
    let promotion_metrics_route = state.promotions.clone();
    app = app.route(
        "/metrics",
        axum::routing::get(move || {
            let metrics = metrics_route.clone();
            #[cfg(feature = "enterprise")]
            let jobs = job_metrics_route.clone();
            #[cfg(feature = "enterprise")]
            let promotions = promotion_metrics_route.clone();
            async move {
                #[cfg(not(feature = "enterprise"))]
                {
                    metrics.render()
                }
                #[cfg(feature = "enterprise")]
                {
                    format!(
                        "{}{}{}",
                        metrics.render(),
                        jobs.metrics_text(),
                        promotions.metrics_text()
                    )
                }
            }
        }),
    );
    // The metrics layer and the /api/admin/logs handler share one
    // recent-errors ring: the layer writes it, the handler reads it.
    let observability = hardening::Observability {
        metrics,
        errors: state.recent_errors.clone(),
    };

    // Serve the built console when configured (COGNIGRAPH_UI_DIST):
    // exact routes above always win; everything else is the SPA. The Bun
    // dev server mirrors this fallback during development.
    if let Some(dist) = &config.ui_dist {
        if std::path::Path::new(dist).join("index.html").is_file() {
            tracing::info!(dist = %dist, "Console UI enabled (static files + SPA fallback)");
            app = app.fallback_service(spa::ui_service(dist));
        } else {
            tracing::warn!(
                dist = %dist,
                "COGNIGRAPH_UI_DIST has no index.html — console serving disabled"
            );
        }
    }

    let mut app = app
        .layer(axum::middleware::from_fn_with_state(
            observability,
            hardening::record_metrics,
        ))
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .layer(tower_http::timeout::TimeoutLayer::with_status_code(
            axum::http::StatusCode::REQUEST_TIMEOUT,
            std::time::Duration::from_secs(config.request_timeout_secs),
        ));
    if config.rate_limit_per_minute > 0 {
        tracing::info!(
            per_minute = config.rate_limit_per_minute,
            "Rate limiting enabled"
        );
        let limiter = std::sync::Arc::new(hardening::RateLimiter::new(
            config.rate_limit_per_minute,
            60,
        ));
        app = app.layer(axum::middleware::from_fn_with_state(
            limiter,
            hardening::rate_limit,
        ));
    }
    #[cfg(feature = "enterprise")]
    let jobs = state.jobs.clone();
    let app = app.with_state(state);

    let listener = tokio::net::TcpListener::bind(config.listen_addr)
        .await
        .expect("Failed to bind");

    tracing::info!(
        addr = %config.listen_addr,
        backend = "native",
        "CogniGraph server listening"
    );

    #[cfg(feature = "enterprise")]
    let shutdown_jobs = jobs.clone();
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(async move {
        let ctrl_c = tokio::signal::ctrl_c();
        #[cfg(unix)]
        {
            let mut term =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                    .expect("failed to install SIGTERM handler");
            tokio::select! {
                _ = ctrl_c => {},
                _ = term.recv() => {},
            }
        }
        #[cfg(not(unix))]
        {
            ctrl_c.await.ok();
        }
        #[cfg(feature = "enterprise")]
        shutdown_jobs.begin_shutdown();
        tracing::info!("shutdown signal received, draining connections and checkpointing jobs");
    })
    .await
    .expect("Server error");
    #[cfg(feature = "enterprise")]
    jobs.shutdown().await;
}
