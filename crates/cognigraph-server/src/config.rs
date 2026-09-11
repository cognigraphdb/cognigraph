use std::net::SocketAddr;
use std::path::Path;

const DEFAULT_ARTIFACT_MAX_EVALUATION_BYTES: u64 = 1_073_741_824;
pub(crate) const DEFAULT_ARTIFACT_MAX_CUSTODY_BYTES: u64 = 2_147_483_648;

/// Server configuration loaded from environment variables.
pub struct Config {
    /// Backend type: "native" (default) or "arango"
    pub backend: String,
    /// ArangoDB connection URL
    pub arango_url: String,
    /// ArangoDB database name
    pub arango_db: String,
    /// ArangoDB username
    pub arango_user: String,
    /// ArangoDB password
    pub arango_password: String,
    /// Server listen address
    pub listen_addr: SocketAddr,
    /// Embedding provider: "openai", "ollama", or "none"
    pub embedding_provider: String,
    /// OpenAI API key (for openai provider)
    pub openai_api_key: Option<String>,
    /// OpenAI base URL override
    pub openai_base_url: Option<String>,
    /// Embedding model name override
    pub embedding_model: Option<String>,
    /// Ollama base URL override
    pub ollama_base_url: Option<String>,
    pub gemini_api_key: Option<String>,
    /// Vector search mode: "native" (APPROX_NEAR_COSINE, default) or "fallback" (AQL cosine)
    pub vector_search_mode: String,
    /// Whether the query cache is enabled
    pub cache_enabled: bool,
    /// Storage path for the native backend; in-memory when unset.
    pub native_path: Option<String>,
    /// Multi-tenant data directory (decision_multi_tenancy.md): when
    /// set, each tenant gets its own store at `<dir>/<tenant>.redb` and
    /// the control store (auth + tenant records) lives at
    /// `<dir>/_control.redb`. Mutually exclusive with the single-store
    /// COGNIGRAPH_NATIVE_PATH.
    pub data_dir: Option<String>,
    /// "embedded" (default) or "sidecar" — see docs/vector-sidecar-design.md.
    pub vector_mode: String,
    /// "resident" (default) or "paged" — see docs/redb-primary-design.md.
    pub storage_mode: String,
    /// Paged-mode document cache bound in bytes.
    pub cache_bytes: usize,
    /// Enables the read-write CGQL endpoint (POST /api/query).
    pub cgql_mutations_enabled: bool,
    /// Bearer-token auth + RBAC; open access when false.
    pub auth_enabled: bool,
    /// Bootstrap password for the admin user (used once, at startup).
    pub admin_password: Option<String>,
    /// Separate bootstrap password for the control-plane HostAdmin user.
    pub host_admin_password: Option<String>,
    /// Enables JWT sessions (POST /api/auth/login) when set.
    pub jwt_secret: Option<String>,
    pub jwt_ttl_secs: u64,
    /// Default TTL for newly issued API tokens (0 = non-expiring).
    /// Per-request `expires_in_secs` overrides it either way.
    pub token_ttl_secs: u64,
    /// Base64url (without padding) Ed25519 public key for the host governance
    /// root. M19 signing-key certificates are accepted only when they chain to
    /// this out-of-store trust anchor. The corresponding private key must never
    /// be supplied to the server.
    pub governance_root_public_key: Option<String>,
    /// External artifact consumption source: "disabled" (default) or
    /// "local-cas". Artifact bytes are never fetched from signed locations.
    pub artifact_source: String,
    /// Absolute operator-managed, read-only local CAS root. Required only when
    /// `artifact_source` is `local-cas`.
    pub artifact_cas_root: Option<String>,
    /// Maximum total artifact bytes verified by one evaluation.
    pub artifact_max_evaluation_bytes: u64,
    /// Maximum unique artifact bytes admitted to one M24 recovery plan.
    pub artifact_max_custody_bytes: u64,
    /// Chunks committed per durable construction checkpoint.
    #[cfg(feature = "enterprise")]
    pub job_ingest_batch_size: usize,
    /// Maximum number of nonterminal durable jobs across all tenants.
    #[cfg(feature = "enterprise")]
    pub job_max_active_total: usize,
    /// Host ceiling for nonterminal durable jobs in one tenant incarnation.
    #[cfg(feature = "enterprise")]
    pub job_max_active_per_tenant: usize,
    /// Age after which terminal durable jobs become eligible for archival.
    #[cfg(feature = "enterprise")]
    pub job_retention_secs: u64,
    /// Maximum terminal jobs processed by one archival request.
    #[cfg(feature = "enterprise")]
    pub job_archive_batch_size: usize,
    /// Per-query CGQL source-row cap (0 = unlimited).
    pub cgql_max_source_rows: u64,
    /// Per-query CGQL wall-clock budget in ms (0 = unlimited).
    pub cgql_time_budget_ms: u64,
    /// Lua instruction limit per script (0 = keep the engine default, 1M).
    pub lua_instruction_limit: u32,
    /// Per-IP requests per minute; 0 disables rate limiting.
    pub rate_limit_per_minute: u32,
    /// Whole-request timeout in seconds.
    pub request_timeout_secs: u64,
    /// "text" (default) or "json".
    pub log_format: String,
    /// Directory with the built console UI (ui/dist). When set, the
    /// server serves it with an SPA fallback: real files win, any other
    /// non-API path answers index.html so the console's path URLs
    /// deep-link. Unset = API-only server.
    pub ui_dist: Option<String>,
}

impl Config {
    /// Load configuration from environment variables with sensible defaults.
    ///
    /// Naming (decision_env_naming.md): every CogniGraph-owned knob is
    /// `COGNIGRAPH_*`; only ecosystem-standard third-party names stay bare
    /// (`OPENAI_API_KEY`, `GEMINI_API_KEY`, `OLLAMA_BASE_URL`, `ARANGO_*`,
    /// `RUST_LOG`). Legacy unprefixed names are NOT read — `warn_legacy_env`
    /// flags them at startup so a stale .env fails loudly, not silently.
    pub fn from_env() -> Self {
        warn_legacy_env();
        let host = env("COGNIGRAPH_HOST", "0.0.0.0");
        let port = env("COGNIGRAPH_PORT", "3000");
        let addr = format!("{host}:{port}");

        Self {
            backend: env("COGNIGRAPH_BACKEND", "native"),
            arango_url: env("ARANGO_URL", "http://localhost:8529"),
            arango_db: env("ARANGO_DB", "cognigraph"),
            arango_user: env("ARANGO_USER", "root"),
            arango_password: env("ARANGO_PASSWORD", ""),
            listen_addr: addr.parse().unwrap_or_else(|_| {
                panic!("Invalid listen address: {addr}");
            }),
            embedding_provider: env("COGNIGRAPH_EMBEDDING_PROVIDER", "none"),
            openai_api_key: env_opt("OPENAI_API_KEY"),
            openai_base_url: env_opt("OPENAI_BASE_URL"),
            embedding_model: env_opt("COGNIGRAPH_EMBEDDING_MODEL"),
            ollama_base_url: env_opt("OLLAMA_BASE_URL"),
            gemini_api_key: env_opt("GEMINI_API_KEY"),
            vector_search_mode: env("COGNIGRAPH_VECTOR_SEARCH_MODE", "native"),
            cache_enabled: env("COGNIGRAPH_QUERY_CACHE_ENABLED", "false") == "true",
            native_path: env_opt("COGNIGRAPH_NATIVE_PATH"),
            data_dir: env_opt("COGNIGRAPH_DATA_DIR"),
            vector_mode: env("COGNIGRAPH_VECTOR_MODE", "embedded"),
            storage_mode: env("COGNIGRAPH_STORAGE_MODE", "resident"),
            cache_bytes: env("COGNIGRAPH_CACHE_BYTES", "268435456")
                .parse()
                .unwrap_or(268_435_456),
            cgql_mutations_enabled: env("COGNIGRAPH_CGQL_MUTATIONS_ENABLED", "false") == "true",
            auth_enabled: env("COGNIGRAPH_AUTH_ENABLED", "false") == "true",
            admin_password: env_opt("COGNIGRAPH_ADMIN_PASSWORD"),
            host_admin_password: env_opt("COGNIGRAPH_HOST_ADMIN_PASSWORD"),
            jwt_secret: env_opt("COGNIGRAPH_JWT_SECRET"),
            jwt_ttl_secs: env("COGNIGRAPH_JWT_TTL_SECS", "3600")
                .parse()
                .unwrap_or(3600),
            token_ttl_secs: env("COGNIGRAPH_TOKEN_TTL_SECS", "0").parse().unwrap_or(0),
            governance_root_public_key: env_opt("COGNIGRAPH_GOVERNANCE_ROOT_PUBLIC_KEY"),
            artifact_source: env("COGNIGRAPH_ARTIFACT_SOURCE", "disabled"),
            artifact_cas_root: env_opt("COGNIGRAPH_ARTIFACT_CAS_ROOT"),
            artifact_max_evaluation_bytes: env(
                "COGNIGRAPH_ARTIFACT_MAX_EVALUATION_BYTES",
                &DEFAULT_ARTIFACT_MAX_EVALUATION_BYTES.to_string(),
            )
            .parse()
            .unwrap_or_else(|_| {
                panic!("COGNIGRAPH_ARTIFACT_MAX_EVALUATION_BYTES must be a positive integer")
            }),
            artifact_max_custody_bytes: env(
                "COGNIGRAPH_ARTIFACT_MAX_CUSTODY_BYTES",
                &DEFAULT_ARTIFACT_MAX_CUSTODY_BYTES.to_string(),
            )
            .parse()
            .unwrap_or_else(|_| {
                panic!("COGNIGRAPH_ARTIFACT_MAX_CUSTODY_BYTES must be a positive integer")
            }),
            #[cfg(feature = "enterprise")]
            job_ingest_batch_size: env("COGNIGRAPH_JOB_INGEST_BATCH_SIZE", "25")
                .parse()
                .unwrap_or(25),
            #[cfg(feature = "enterprise")]
            job_max_active_total: env("COGNIGRAPH_JOB_MAX_ACTIVE_TOTAL", "1000")
                .parse()
                .unwrap_or(1_000),
            #[cfg(feature = "enterprise")]
            job_max_active_per_tenant: env("COGNIGRAPH_JOB_MAX_ACTIVE_PER_TENANT", "100")
                .parse()
                .unwrap_or(100),
            #[cfg(feature = "enterprise")]
            job_retention_secs: env("COGNIGRAPH_JOB_RETENTION_SECS", "2592000")
                .parse()
                .unwrap_or(2_592_000),
            #[cfg(feature = "enterprise")]
            job_archive_batch_size: env("COGNIGRAPH_JOB_ARCHIVE_BATCH_SIZE", "100")
                .parse()
                .unwrap_or(100),
            cgql_max_source_rows: env("COGNIGRAPH_CGQL_MAX_SOURCE_ROWS", "0")
                .parse()
                .unwrap_or(0),
            cgql_time_budget_ms: env("COGNIGRAPH_CGQL_TIME_BUDGET_MS", "0")
                .parse()
                .unwrap_or(0),
            lua_instruction_limit: env("COGNIGRAPH_LUA_INSTRUCTION_LIMIT", "0")
                .parse()
                .unwrap_or(0),
            rate_limit_per_minute: env("COGNIGRAPH_RATE_LIMIT_PER_MINUTE", "0")
                .parse()
                .unwrap_or(0),
            request_timeout_secs: env("COGNIGRAPH_REQUEST_TIMEOUT_SECS", "30")
                .parse()
                .unwrap_or(30),
            log_format: env("COGNIGRAPH_LOG_FORMAT", "text"),
            ui_dist: env_opt("COGNIGRAPH_UI_DIST"),
        }
    }

    /// Reject bootstrap settings that would create credentials with no
    /// reachable authentication path. Bootstrap passwords are consumed only
    /// by the password-to-JWT login flow; without a JWT secret, those users
    /// cannot obtain a bearer credential.
    pub fn validate(&self) -> Result<(), String> {
        #[cfg(not(feature = "enterprise"))]
        if self.data_dir.is_some()
            || self.governance_root_public_key.is_some()
            || self.artifact_source != "disabled"
            || self.artifact_cas_root.is_some()
        {
            return Err("enterprise_feature_required: multi-tenancy, governance and artifact consumption require an Enterprise build".into());
        }
        validate_bootstrap_access(
            self.auth_enabled,
            self.admin_password.is_some() || self.host_admin_password.is_some(),
            self.jwt_secret.is_some(),
        )?;
        validate_artifact_config(
            &self.artifact_source,
            self.artifact_cas_root.as_deref(),
            self.artifact_max_evaluation_bytes,
            self.artifact_max_custody_bytes,
        )
    }
}

fn validate_artifact_config(
    source: &str,
    cas_root: Option<&str>,
    max_evaluation_bytes: u64,
    max_custody_bytes: u64,
) -> Result<(), String> {
    if max_evaluation_bytes == 0 {
        return Err("COGNIGRAPH_ARTIFACT_MAX_EVALUATION_BYTES must be positive".into());
    }
    if max_custody_bytes == 0 {
        return Err("COGNIGRAPH_ARTIFACT_MAX_CUSTODY_BYTES must be positive".into());
    }
    match source {
        "disabled" => Ok(()),
        "local-cas" => {
            let root = cas_root.ok_or_else(|| {
                "COGNIGRAPH_ARTIFACT_CAS_ROOT is required when COGNIGRAPH_ARTIFACT_SOURCE=local-cas"
                    .to_string()
            })?;
            if !Path::new(root).is_absolute() {
                return Err(
                    "COGNIGRAPH_ARTIFACT_CAS_ROOT must be absolute when COGNIGRAPH_ARTIFACT_SOURCE=local-cas"
                        .into(),
                );
            }
            Ok(())
        }
        other => Err(format!(
            "COGNIGRAPH_ARTIFACT_SOURCE must be `disabled` or `local-cas`, got `{other}`"
        )),
    }
}

fn validate_bootstrap_access(
    auth_enabled: bool,
    bootstrap_password_configured: bool,
    jwt_secret_configured: bool,
) -> Result<(), String> {
    if bootstrap_password_configured && !auth_enabled {
        return Err(
            "COGNIGRAPH_AUTH_ENABLED=true is required when a bootstrap password is configured"
                .into(),
        );
    }
    if bootstrap_password_configured && !jwt_secret_configured {
        return Err(
            "COGNIGRAPH_JWT_SECRET is required when COGNIGRAPH_ADMIN_PASSWORD or COGNIGRAPH_HOST_ADMIN_PASSWORD is configured"
                .into(),
        );
    }
    Ok(())
}

/// Pre-2.0 unprefixed names and their replacements. Set-but-ignored config
/// is the worst failure mode (auth silently off, budgets silently absent),
/// so each one found gets a loud stderr warning. eprintln! because this
/// runs before the tracing subscriber exists.
fn warn_legacy_env() {
    const RENAMED: &[(&str, &str)] = &[
        ("AUTH_ENABLED", "COGNIGRAPH_AUTH_ENABLED"),
        ("JWT_SECRET", "COGNIGRAPH_JWT_SECRET"),
        ("JWT_TTL_SECS", "COGNIGRAPH_JWT_TTL_SECS"),
        ("CACHE_ENABLED", "COGNIGRAPH_QUERY_CACHE_ENABLED"),
        ("CACHE_TTL_SECS", "COGNIGRAPH_QUERY_CACHE_TTL_SECS"),
        ("CACHE_MAX_ENTRIES", "COGNIGRAPH_QUERY_CACHE_MAX_ENTRIES"),
        (
            "CACHE_SIMILARITY_FLOOR",
            "COGNIGRAPH_QUERY_CACHE_SIMILARITY_FLOOR",
        ),
        (
            "CACHE_STRONG_THRESHOLD",
            "COGNIGRAPH_QUERY_CACHE_STRONG_THRESHOLD",
        ),
        ("CACHE_BACKEND", "COGNIGRAPH_QUERY_CACHE_BACKEND"),
        (
            "CGQL_MUTATIONS_ENABLED",
            "COGNIGRAPH_CGQL_MUTATIONS_ENABLED",
        ),
        ("CGQL_MAX_SOURCE_ROWS", "COGNIGRAPH_CGQL_MAX_SOURCE_ROWS"),
        ("CGQL_TIME_BUDGET_MS", "COGNIGRAPH_CGQL_TIME_BUDGET_MS"),
        ("LUA_INSTRUCTION_LIMIT", "COGNIGRAPH_LUA_INSTRUCTION_LIMIT"),
        ("RATE_LIMIT_PER_MINUTE", "COGNIGRAPH_RATE_LIMIT_PER_MINUTE"),
        ("REQUEST_TIMEOUT_SECS", "COGNIGRAPH_REQUEST_TIMEOUT_SECS"),
        ("LOG_FORMAT", "COGNIGRAPH_LOG_FORMAT"),
        ("EMBEDDING_PROVIDER", "COGNIGRAPH_EMBEDDING_PROVIDER"),
        ("EMBEDDING_MODEL", "COGNIGRAPH_EMBEDDING_MODEL"),
        ("VECTOR_SEARCH_MODE", "COGNIGRAPH_VECTOR_SEARCH_MODE"),
    ];
    for (old, new) in RENAMED {
        if std::env::var(old).is_ok() {
            eprintln!("WARNING: {old} is set but IGNORED since 2.0 — use {new}");
        }
    }
}

fn env(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

fn env_opt(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|v| !v.is_empty())
}

#[cfg(test)]
mod tests {
    use super::{validate_artifact_config, validate_bootstrap_access};

    #[test]
    fn bootstrap_passwords_require_enabled_auth_and_jwt_login() {
        assert!(validate_bootstrap_access(true, false, false).is_ok());
        assert!(validate_bootstrap_access(true, true, true).is_ok());
        assert!(validate_bootstrap_access(false, true, true).is_err());
        assert!(validate_bootstrap_access(true, true, false).is_err());
    }

    #[test]
    fn artifact_source_is_disabled_by_contract_without_a_cas_root() {
        assert!(validate_artifact_config("disabled", None, 1, 1).is_ok());
    }

    #[test]
    fn local_artifact_cas_requires_an_absolute_root_and_positive_budget() {
        assert!(validate_artifact_config("local-cas", Some("/srv/cognigraph-cas"), 1, 1).is_ok());
        assert!(validate_artifact_config("local-cas", None, 1, 1).is_err());
        assert!(validate_artifact_config("local-cas", Some("relative/cas"), 1, 1).is_err());
        assert!(validate_artifact_config("local-cas", Some("/srv/cognigraph-cas"), 0, 1).is_err());
        assert!(validate_artifact_config("remote", None, 1, 1).is_err());
    }

    #[test]
    fn custody_plan_budget_is_positive_and_independent_of_runtime_cas_mode() {
        assert!(validate_artifact_config("disabled", None, 1, 2).is_ok());
        assert!(validate_artifact_config("disabled", None, 1, 0).is_err());
        assert!(validate_artifact_config("local-cas", Some("/srv/cognigraph-cas"), 1, 2).is_ok());
    }
}
