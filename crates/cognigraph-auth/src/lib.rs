//! Backend-agnostic authentication and RBAC.
//!
//! Users and API tokens are stored in the `_users` / `_tokens` collections
//! through the `GraphBackend` trait, so every backend gets auth for free.
//! Passwords are argon2-hashed; tokens are random `cg_…` strings of which
//! only the SHA-256 hash is stored.

use std::collections::HashMap;
use std::sync::Arc;

use argon2::Argon2;
use argon2::password_hash::{PasswordHasher, PasswordVerifier, phc::PasswordHash};
use cognigraph_core::{CogniGraphError, CollectionType, GraphBackend, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub mod jwt;

pub const USERS_COLLECTION: &str = "_users";
pub const TOKENS_COLLECTION: &str = "_tokens";
/// Tenant records (decision_multi_tenancy.md). Lives beside users and
/// tokens in the CONTROL store — never inside tenant data.
pub const TENANTS_COLLECTION: &str = "_tenants";
/// The implicit tenant: deployments that never create tenant records
/// behave exactly as before tenancy existed.
pub const DEFAULT_TENANT: &str = "default";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Role {
    Admin,
    Editor,
    Viewer,
    ScriptRunner,
    /// Tenant-local M19 role that authors immutable promotion-policy drafts.
    /// It deliberately cannot approve policy or make promotion decisions.
    PolicyAuthor,
    /// Tenant-local M19 role that independently approves signed policy.
    /// It deliberately cannot author policy or make promotion decisions.
    PolicyApprover,
    /// Tenant-local M19 role that makes promotion decisions under an
    /// independently approved policy.
    Promoter,
    /// Tenant-local M20 role that attests external evaluation-artifact
    /// content. It deliberately cannot author or approve policy, decide
    /// promotions, or access tenant data.
    ArtifactAttestor,
    /// Control-store-level role: tenant lifecycle ONLY (create, suspend,
    /// delete, quotas). Deliberately has no data scopes — host-admin
    /// manages tenants, it does not read their graphs
    /// (decision_multi_tenancy.md, D4).
    HostAdmin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Scope {
    DocumentsRead,
    DocumentsWrite,
    GraphRead,
    GraphWrite,
    Search,
    LuaExecute,
    Admin,
    /// Read tenant-local promotion evidence, policy, decisions, and heads.
    PromotionRead,
    /// Author immutable promotion-policy drafts.
    PolicyAuthor,
    /// Approve promotion policy independently of its author.
    PolicyApprove,
    /// Promote, reject, or roll back evaluated candidates.
    PromotionDecide,
    /// Bootstrap and operate the tenant governance trust registry.
    GovernanceTrust,
    /// Submit content-addressed attestations for external evaluation artifacts.
    ArtifactAttest,
    /// Tenant lifecycle management (host-admin only).
    TenantAdmin,
}

impl Role {
    pub fn scopes(self) -> &'static [Scope] {
        match self {
            Role::Admin => &[
                Scope::DocumentsRead,
                Scope::DocumentsWrite,
                Scope::GraphRead,
                Scope::GraphWrite,
                Scope::Search,
                Scope::LuaExecute,
                Scope::Admin,
                Scope::PromotionRead,
                Scope::GovernanceTrust,
            ],
            Role::Editor => &[
                Scope::DocumentsRead,
                Scope::DocumentsWrite,
                Scope::GraphRead,
                Scope::GraphWrite,
                Scope::Search,
                Scope::LuaExecute,
            ],
            Role::Viewer => &[Scope::DocumentsRead, Scope::GraphRead, Scope::Search],
            Role::ScriptRunner => &[
                Scope::DocumentsRead,
                Scope::GraphRead,
                Scope::Search,
                Scope::LuaExecute,
            ],
            Role::PolicyAuthor => &[Scope::PromotionRead, Scope::PolicyAuthor],
            Role::PolicyApprover => &[Scope::PromotionRead, Scope::PolicyApprove],
            Role::Promoter => &[Scope::PromotionRead, Scope::PromotionDecide],
            Role::ArtifactAttestor => &[Scope::PromotionRead, Scope::ArtifactAttest],
            Role::HostAdmin => &[Scope::TenantAdmin],
        }
    }

    pub fn grants(self, scope: Scope) -> bool {
        self.scopes().contains(&scope)
    }
}

fn default_tenant() -> String {
    DEFAULT_TENANT.to_string()
}

pub(crate) fn default_tenant_string() -> String {
    DEFAULT_TENANT.to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub key: String,
    pub username: String,
    pub role: Role,
    /// The one tenant this user belongs to (v1: exactly one). Legacy
    /// user documents without the field are the implicit default
    /// tenant — back-compat is absolute.
    #[serde(default = "default_tenant")]
    pub tenant: String,
}

/// A tenant record in the control store. Quotas remain an extensible JSON
/// object; the durable-job scheduler enforces `max_active_jobs` while unknown
/// keys remain round-trippable for later governance phases.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tenant {
    pub name: String,
    /// Immutable lifecycle identity. Tenant names may be reused after
    /// deletion, so durable work must bind to this value rather than the
    /// display name alone. Legacy records deserialize with an empty value;
    /// callers derive a stable legacy identity from `created_at`.
    #[serde(default)]
    pub incarnation: String,
    pub status: TenantStatus,
    #[serde(default)]
    pub created_at: u64,
    /// Extensible quota object. `max_active_jobs` is enforced by M17; other
    /// keys are stored verbatim until their owning subsystem adopts them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quotas: Option<Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TenantStatus {
    Active,
    Suspended,
}

/// Authentication and user management over any `GraphBackend`.
pub struct AuthProvider {
    backend: Arc<dyn GraphBackend>,
    lifecycle_lock: tokio::sync::Mutex<()>,
}

fn auth_err(message: impl Into<String>) -> CogniGraphError {
    CogniGraphError::AuthError(message.into())
}

fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    // digest 0.11 dropped LowerHex on the output array; encode manually.
    hasher
        .finalize()
        .iter()
        .fold(String::with_capacity(64), |mut hex, byte| {
            use std::fmt::Write;
            let _ = write!(hex, "{byte:02x}");
            hex
        })
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// A freshly issued (or rotated) API token. The plaintext is shown exactly
/// once and never stored — only its SHA-256 hash is.
#[derive(Debug, Clone, Serialize)]
pub struct TokenGrant {
    /// The token record's key (used to revoke or rotate it later).
    pub key: String,
    /// The plaintext `cg_…` token.
    pub token: String,
    /// Unix seconds after which the token stops validating; `None` = never.
    pub expires_at: Option<u64>,
}

impl AuthProvider {
    pub async fn new(backend: Arc<dyn GraphBackend>) -> Result<Self> {
        backend
            .ensure_collection(USERS_COLLECTION, CollectionType::Document)
            .await?;
        backend
            .ensure_collection(TOKENS_COLLECTION, CollectionType::Document)
            .await?;
        backend
            .ensure_collection(TENANTS_COLLECTION, CollectionType::Document)
            .await?;
        Ok(Self {
            backend,
            lifecycle_lock: tokio::sync::Mutex::new(()),
        })
    }

    /// Ensure an `admin` user exists with the given password (bootstrap).
    pub async fn bootstrap_admin(&self, password: &str) -> Result<()> {
        if self.find_by_username("admin").await?.is_none() {
            self.create_user("admin", password, Role::Admin).await?;
            tracing::info!("bootstrap: created admin user");
        }
        Ok(())
    }

    /// Ensure the control-plane-only bootstrap HostAdmin exists. Its password
    /// is configured separately from the default tenant Admin so the two
    /// authorities do not share credentials.
    pub async fn bootstrap_host_admin(&self, password: &str) -> Result<()> {
        match self.find_by_username("host-admin").await? {
            Some(document)
                if user_from_doc(&document).is_some_and(|user| user.role == Role::HostAdmin) =>
            {
                Ok(())
            }
            Some(_) => Err(CogniGraphError::DocumentConflict(
                "bootstrap username `host-admin` exists with another role or is malformed".into(),
            )),
            None => {
                self.create_user("host-admin", password, Role::HostAdmin)
                    .await?;
                tracing::info!("bootstrap: created host-admin user");
                Ok(())
            }
        }
    }

    /// Provision the first tenant-local Admin through HostAdmin control-plane
    /// authority. The lock prevents concurrent requests from creating two
    /// initial Admins; once one exists, ordinary tenant-local user management
    /// owns all further provisioning.
    pub async fn bootstrap_tenant_admin(
        &self,
        tenant: &str,
        username: &str,
        password: &str,
    ) -> Result<User> {
        let _guard = self.lifecycle_lock.lock().await;
        self.tenant_allowed(tenant).await?;
        if self
            .list_users()
            .await?
            .iter()
            .any(|user| user.tenant == tenant && user.role == Role::Admin)
        {
            return Err(CogniGraphError::DocumentConflict(format!(
                "tenant `{tenant}` already has an Admin"
            )));
        }
        self.create_user_in_unlocked(username, password, Role::Admin, tenant)
            .await
    }

    pub async fn create_user(&self, username: &str, password: &str, role: Role) -> Result<User> {
        self.create_user_in(username, password, role, DEFAULT_TENANT)
            .await
    }

    pub async fn create_user_in(
        &self,
        username: &str,
        password: &str,
        role: Role,
        tenant: &str,
    ) -> Result<User> {
        let _guard = self.lifecycle_lock.lock().await;
        if role != Role::HostAdmin {
            self.tenant_allowed(tenant).await?;
        }
        self.create_user_in_unlocked(username, password, role, tenant)
            .await
    }

    async fn create_user_in_unlocked(
        &self,
        username: &str,
        password: &str,
        role: Role,
        tenant: &str,
    ) -> Result<User> {
        if self.find_by_username(username).await?.is_some() {
            return Err(CogniGraphError::DocumentConflict(format!(
                "user `{username}` already exists"
            )));
        }
        // Argon2 generates a fresh salt with the OS RNG and retains PHC parameters.
        let hash = Argon2::default()
            .hash_password(password.as_bytes())
            .map_err(|e| auth_err(format!("password hashing failed: {e}")))?
            .to_string();
        let id = self
            .backend
            .create_document(
                USERS_COLLECTION,
                json!({
                    "username": username,
                    "password_hash": hash,
                    "role": serde_json::to_value(role)?,
                    "tenant": tenant,
                }),
            )
            .await?;
        Ok(User {
            key: id.key,
            username: username.to_string(),
            role,
            tenant: tenant.to_string(),
        })
    }

    // ---- Tenant lifecycle (control-store records; TenantAdmin scope) ----

    pub async fn create_tenant(&self, name: &str, quotas: Option<Value>) -> Result<Tenant> {
        let _guard = self.lifecycle_lock.lock().await;
        if name != name.to_lowercase()
            || name.is_empty()
            || name.len() > 63
            || !name
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
            || name.starts_with('-')
        {
            return Err(auth_err(
                "tenant name must be 1-63 chars of [a-z0-9-], not starting with '-'",
            ));
        }
        let tenant = Tenant {
            name: name.to_string(),
            incarnation: if name == DEFAULT_TENANT {
                DEFAULT_TENANT.to_string()
            } else {
                Uuid::new_v4().to_string()
            },
            status: TenantStatus::Active,
            created_at: now_secs(),
            quotas,
        };
        let mut doc = serde_json::to_value(&tenant)?;
        doc["_key"] = json!(name);
        self.backend
            .create_document(TENANTS_COLLECTION, doc)
            .await?;
        Ok(tenant)
    }

    pub async fn get_tenant(&self, name: &str) -> Result<Option<Tenant>> {
        Ok(self
            .backend
            .get_document(TENANTS_COLLECTION, name)
            .await?
            .and_then(|doc| serde_json::from_value(doc).ok()))
    }

    pub async fn list_tenants(&self) -> Result<Vec<Tenant>> {
        Ok(self
            .backend
            .list_documents(TENANTS_COLLECTION, None, None)
            .await?
            .into_iter()
            .filter_map(|doc| serde_json::from_value(doc).ok())
            .collect())
    }

    pub async fn set_tenant_status(&self, name: &str, status: TenantStatus) -> Result<Tenant> {
        let _guard = self.lifecycle_lock.lock().await;
        let mut tenant = self
            .get_tenant(name)
            .await?
            .ok_or_else(|| auth_err(format!("no tenant `{name}`")))?;
        tenant.status = status;
        self.backend
            .update_document(
                TENANTS_COLLECTION,
                name,
                json!({ "status": serde_json::to_value(status)? }),
            )
            .await?;
        Ok(tenant)
    }

    pub async fn set_tenant_quotas(&self, name: &str, quotas: Option<Value>) -> Result<Tenant> {
        let _guard = self.lifecycle_lock.lock().await;
        let mut tenant = self
            .get_tenant(name)
            .await?
            .ok_or_else(|| auth_err(format!("no tenant `{name}`")))?;
        tenant.quotas = quotas.clone();
        self.backend
            .update_document(
                TENANTS_COLLECTION,
                name,
                json!({ "quotas": quotas.unwrap_or(Value::Null) }),
            )
            .await?;
        Ok(tenant)
    }

    /// Delete the tenant record AND its users (with their tokens).
    /// Leaving the users would revive their credentials the moment the
    /// name is recreated (decision_tenant_deletion.md). The tenant's
    /// data store is the server's to retire — the control store knows
    /// nothing about files.
    pub async fn delete_tenant(&self, name: &str) -> Result<bool> {
        let _guard = self.lifecycle_lock.lock().await;
        if name == DEFAULT_TENANT {
            return Err(auth_err(
                "the default tenant cannot be deleted — suspend it instead",
            ));
        }
        for user in self.list_users().await? {
            if user.tenant == name {
                self.delete_user(&user.key).await?;
            }
        }
        self.backend.delete_document(TENANTS_COLLECTION, name).await
    }

    /// The per-request tenant gate (middleware): the implicit default
    /// tenant is always allowed unless a record explicitly suspends it;
    /// any other tenant must have an ACTIVE record.
    pub async fn tenant_allowed(&self, tenant: &str) -> Result<()> {
        match self.get_tenant(tenant).await? {
            Some(record) if record.status == TenantStatus::Active => Ok(()),
            Some(_) => Err(CogniGraphError::Forbidden(format!(
                "tenant `{tenant}` is suspended"
            ))),
            None if tenant == DEFAULT_TENANT => Ok(()),
            None => Err(CogniGraphError::Forbidden(format!(
                "unknown tenant `{tenant}`"
            ))),
        }
    }

    pub async fn list_users(&self) -> Result<Vec<User>> {
        let docs = self
            .backend
            .list_documents(USERS_COLLECTION, None, None)
            .await?;
        Ok(docs.iter().filter_map(user_from_doc).collect())
    }

    /// Resolve a user by its stable control-store key.
    ///
    /// Privileged routes use this to revalidate JWT-embedded identity against
    /// current user state before acting. API-token authentication already does
    /// the same lookup as part of token validation.
    pub async fn get_user(&self, key: &str) -> Result<Option<User>> {
        Ok(self
            .backend
            .get_document(USERS_COLLECTION, key)
            .await?
            .as_ref()
            .and_then(user_from_doc))
    }

    pub async fn delete_user(&self, key: &str) -> Result<bool> {
        // Revoke the user's tokens as well.
        for token in self
            .backend
            .list_documents(TOKENS_COLLECTION, None, None)
            .await?
        {
            if token.get("user_key").and_then(Value::as_str) == Some(key)
                && let Some(token_key) = token.get("_key").and_then(Value::as_str)
            {
                self.backend
                    .delete_document(TOKENS_COLLECTION, token_key)
                    .await?;
            }
        }
        self.backend.delete_document(USERS_COLLECTION, key).await
    }

    async fn find_by_username(&self, username: &str) -> Result<Option<Value>> {
        let docs = self
            .backend
            .list_documents(USERS_COLLECTION, None, None)
            .await?;
        Ok(docs
            .into_iter()
            .find(|doc| doc.get("username").and_then(Value::as_str) == Some(username)))
    }

    pub async fn verify_password(&self, username: &str, password: &str) -> Result<User> {
        let doc = self
            .find_by_username(username)
            .await?
            .ok_or_else(|| auth_err("invalid credentials"))?;
        let stored = doc
            .get("password_hash")
            .and_then(Value::as_str)
            .ok_or_else(|| auth_err("user record is corrupt"))?;
        let parsed =
            PasswordHash::new(stored).map_err(|e| auth_err(format!("stored hash invalid: {e}")))?;
        Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .map_err(|_| auth_err("invalid credentials"))?;
        user_from_doc(&doc).ok_or_else(|| auth_err("user record is corrupt"))
    }

    /// Create an API token for a user. `ttl_secs` of `None` or `Some(0)`
    /// means non-expiring (the policy default lives with the caller — the
    /// server resolves `COGNIGRAPH_TOKEN_TTL_SECS` before calling this).
    pub async fn create_token(
        &self,
        user_key: &str,
        name: &str,
        ttl_secs: Option<u64>,
    ) -> Result<TokenGrant> {
        self.backend
            .get_document(USERS_COLLECTION, user_key)
            .await?
            .ok_or_else(|| auth_err(format!("user `{user_key}` not found")))?;
        let plaintext = format!("cg_{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        let created_at = now_secs();
        let expires_at = ttl_secs.filter(|ttl| *ttl > 0).map(|ttl| created_at + ttl);
        let id = self
            .backend
            .create_document(
                TOKENS_COLLECTION,
                json!({
                    "user_key": user_key,
                    "name": name,
                    "token_hash": hash_token(&plaintext),
                    "created_at": created_at,
                    "expires_at": expires_at,
                }),
            )
            .await?;
        Ok(TokenGrant {
            key: id.key,
            token: plaintext,
            expires_at,
        })
    }

    /// Replace a token's secret in place: same record and key, fresh
    /// plaintext and expiry window. The old plaintext stops validating the
    /// moment the update lands, so references to the token key stay valid
    /// while the credential itself is rotated.
    pub async fn rotate_token(&self, token_key: &str, ttl_secs: Option<u64>) -> Result<TokenGrant> {
        self.backend
            .get_document(TOKENS_COLLECTION, token_key)
            .await?
            .ok_or_else(|| auth_err(format!("token `{token_key}` not found")))?;
        let plaintext = format!("cg_{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        let created_at = now_secs();
        let expires_at = ttl_secs.filter(|ttl| *ttl > 0).map(|ttl| created_at + ttl);
        self.backend
            .update_document(
                TOKENS_COLLECTION,
                token_key,
                json!({
                    "token_hash": hash_token(&plaintext),
                    "created_at": created_at,
                    "expires_at": expires_at,
                }),
            )
            .await?;
        Ok(TokenGrant {
            key: token_key.to_string(),
            token: plaintext,
            expires_at,
        })
    }

    pub async fn revoke_token(&self, token_key: &str) -> Result<bool> {
        self.backend
            .delete_document(TOKENS_COLLECTION, token_key)
            .await
    }

    /// Return the stable user key that owns a token record.
    ///
    /// The plaintext token and its hash are never exposed. A corrupt record is
    /// an authentication-store error rather than an apparent absence.
    pub async fn token_owner(&self, token_key: &str) -> Result<Option<String>> {
        let Some(record) = self
            .backend
            .get_document(TOKENS_COLLECTION, token_key)
            .await?
        else {
            return Ok(None);
        };
        let owner = record
            .get("user_key")
            .and_then(Value::as_str)
            .ok_or_else(|| auth_err("token record is corrupt"))?;
        Ok(Some(owner.to_string()))
    }

    /// Resolve a bearer token to its user, or fail with an auth error.
    /// Expired tokens fail exactly like revoked ones (no oracle for
    /// attackers, and the record stays visible in `list_tokens` for audit).
    pub async fn validate_token(&self, token: &str) -> Result<User> {
        let wanted = hash_token(token);
        let tokens = self
            .backend
            .list_documents(TOKENS_COLLECTION, None, None)
            .await?;
        let record = tokens
            .into_iter()
            .find(|doc| doc.get("token_hash").and_then(Value::as_str) == Some(wanted.as_str()))
            .ok_or_else(|| auth_err("invalid or revoked token"))?;
        if let Some(expires_at) = record.get("expires_at").and_then(Value::as_u64)
            && now_secs() > expires_at
        {
            return Err(auth_err("invalid or revoked token"));
        }
        let user_key = record
            .get("user_key")
            .and_then(Value::as_str)
            .ok_or_else(|| auth_err("token record is corrupt"))?;
        let doc = self
            .backend
            .get_document(USERS_COLLECTION, user_key)
            .await?
            .ok_or_else(|| auth_err("token user no longer exists"))?;
        user_from_doc(&doc).ok_or_else(|| auth_err("user record is corrupt"))
    }

    pub async fn list_tokens(&self, user_key: &str) -> Result<Vec<HashMap<String, Value>>> {
        let docs = self
            .backend
            .list_documents(TOKENS_COLLECTION, None, None)
            .await?;
        Ok(docs
            .into_iter()
            .filter(|doc| doc.get("user_key").and_then(Value::as_str) == Some(user_key))
            .map(|doc| {
                // Never expose the hash.
                let expires_at = doc.get("expires_at").and_then(Value::as_u64);
                [
                    (
                        "key".to_string(),
                        doc.get("_key").cloned().unwrap_or(Value::Null),
                    ),
                    (
                        "name".to_string(),
                        doc.get("name").cloned().unwrap_or(Value::Null),
                    ),
                    (
                        "created_at".to_string(),
                        doc.get("created_at").cloned().unwrap_or(Value::Null),
                    ),
                    (
                        "expires_at".to_string(),
                        expires_at.map(Value::from).unwrap_or(Value::Null),
                    ),
                    (
                        "expired".to_string(),
                        Value::Bool(expires_at.is_some_and(|at| now_secs() > at)),
                    ),
                ]
                .into_iter()
                .collect()
            })
            .collect())
    }
}

fn user_from_doc(doc: &Value) -> Option<User> {
    Some(User {
        key: doc.get("_key")?.as_str()?.to_string(),
        username: doc.get("username")?.as_str()?.to_string(),
        role: serde_json::from_value(doc.get("role")?.clone()).ok()?,
        tenant: doc
            .get("tenant")
            .and_then(Value::as_str)
            .unwrap_or(DEFAULT_TENANT)
            .to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn tenant_gate_and_lifecycle() {
        let backend = std::sync::Arc::new(cognigraph_native::NativeBackend::new());
        let auth = AuthProvider::new(backend).await.unwrap();

        // Implicit default tenant: always allowed with no record.
        auth.tenant_allowed(DEFAULT_TENANT).await.unwrap();
        let default_record = auth.create_tenant(DEFAULT_TENANT, None).await.unwrap();
        assert_eq!(default_record.incarnation, DEFAULT_TENANT);
        // Unknown non-default tenant: refused.
        assert!(auth.tenant_allowed("ghost").await.is_err());

        // Create, gate passes; suspend, gate refuses; reactivate.
        let first = auth.create_tenant("acme", None).await.unwrap();
        assert!(!first.incarnation.is_empty());
        assert!(auth.create_tenant("acme", None).await.is_err()); // conflict
        assert!(auth.create_tenant("Bad Name", None).await.is_err()); // charset
        auth.tenant_allowed("acme").await.unwrap();
        auth.set_tenant_status("acme", TenantStatus::Suspended)
            .await
            .unwrap();
        assert!(auth.tenant_allowed("acme").await.is_err());
        auth.set_tenant_status("acme", TenantStatus::Active)
            .await
            .unwrap();
        auth.tenant_allowed("acme").await.unwrap();

        // Users carry their tenant; legacy docs default.
        let user = auth
            .create_user_in("vera", "pw12345678", Role::Viewer, "acme")
            .await
            .unwrap();
        assert_eq!(user.tenant, "acme");
        let legacy = user_from_doc(&json!({
            "_key": "u9", "username": "old", "role": "editor"
        }))
        .unwrap();
        assert_eq!(legacy.tenant, DEFAULT_TENANT);

        assert!(auth.delete_tenant("acme").await.unwrap());
        assert!(
            auth.create_user_in("late", "pw12345678", Role::Viewer, "acme")
                .await
                .is_err()
        );
        let recreated = auth.create_tenant("acme", None).await.unwrap();
        assert_ne!(first.incarnation, recreated.incarnation);
        assert!(auth.list_users().await.unwrap().iter().all(|user| {
            user.tenant != "acme" || (user.username != "vera" && user.username != "late")
        }));
    }

    #[tokio::test]
    async fn delete_tenant_purges_its_users_and_refuses_default() {
        let backend = std::sync::Arc::new(cognigraph_native::NativeBackend::new());
        let auth = AuthProvider::new(backend).await.unwrap();
        auth.create_tenant("acme", None).await.unwrap();
        let vera = auth
            .create_user_in("vera", "pw12345678", Role::Viewer, "acme")
            .await
            .unwrap();
        let grant = auth.create_token(&vera.key, "ci", None).await.unwrap();
        auth.create_user_in("root", "pw12345678", Role::Admin, DEFAULT_TENANT)
            .await
            .unwrap();

        assert!(auth.delete_tenant("acme").await.unwrap());

        // The tenant's users and their tokens die with it — otherwise
        // recreating the name would revive the old credentials.
        let users = auth.list_users().await.unwrap();
        assert!(users.iter().all(|u| u.tenant != "acme"));
        assert!(users.iter().any(|u| u.username == "root"));
        assert!(auth.validate_token(&grant.token).await.is_err());

        // The implicit default tenant is not deletable (its gate never
        // closes, so deletion would destroy data while requests keep
        // flowing) — suspend it instead.
        assert!(auth.delete_tenant(DEFAULT_TENANT).await.is_err());
    }

    #[tokio::test]
    async fn separate_host_bootstrap_can_provision_exactly_one_initial_tenant_admin() {
        let backend = std::sync::Arc::new(cognigraph_native::NativeBackend::new());
        let auth = AuthProvider::new(backend).await.unwrap();
        auth.bootstrap_admin("tenant-secret").await.unwrap();
        auth.bootstrap_host_admin("host-secret").await.unwrap();
        let users = auth.list_users().await.unwrap();
        assert!(
            users
                .iter()
                .any(|user| user.username == "admin" && user.role == Role::Admin)
        );
        assert!(
            users
                .iter()
                .any(|user| user.username == "host-admin" && user.role == Role::HostAdmin)
        );

        auth.create_tenant("acme", None).await.unwrap();
        let first = auth
            .bootstrap_tenant_admin("acme", "acme-admin", "acme-secret")
            .await
            .unwrap();
        assert_eq!(first.role, Role::Admin);
        assert_eq!(first.tenant, "acme");
        assert!(
            auth.bootstrap_tenant_admin("acme", "other-admin", "other-secret")
                .await
                .is_err()
        );
        assert!(
            auth.bootstrap_tenant_admin("unknown", "ghost-admin", "ghost-secret")
                .await
                .is_err()
        );
    }

    #[test]
    fn role_scope_matrix() {
        assert!(Role::Admin.grants(Scope::Admin));
        assert!(Role::Admin.grants(Scope::PromotionRead));
        assert!(Role::Admin.grants(Scope::GovernanceTrust));
        assert!(!Role::Admin.grants(Scope::PolicyAuthor));
        assert!(!Role::Admin.grants(Scope::PolicyApprove));
        assert!(!Role::Admin.grants(Scope::PromotionDecide));
        assert!(!Role::Admin.grants(Scope::ArtifactAttest));
        assert!(Role::Editor.grants(Scope::DocumentsWrite));
        assert!(!Role::Editor.grants(Scope::Admin));
        assert!(Role::Viewer.grants(Scope::Search));
        assert!(!Role::Viewer.grants(Scope::DocumentsWrite));
        assert!(Role::ScriptRunner.grants(Scope::LuaExecute));
        assert!(!Role::ScriptRunner.grants(Scope::GraphWrite));
        assert_eq!(
            Role::PolicyAuthor.scopes(),
            &[Scope::PromotionRead, Scope::PolicyAuthor]
        );
        assert_eq!(
            Role::PolicyApprover.scopes(),
            &[Scope::PromotionRead, Scope::PolicyApprove]
        );
        assert_eq!(
            Role::Promoter.scopes(),
            &[Scope::PromotionRead, Scope::PromotionDecide]
        );
        assert_eq!(
            Role::ArtifactAttestor.scopes(),
            &[Scope::PromotionRead, Scope::ArtifactAttest]
        );
        for role in [
            Role::PolicyAuthor,
            Role::PolicyApprover,
            Role::Promoter,
            Role::ArtifactAttestor,
        ] {
            assert!(!role.grants(Scope::Admin));
            assert!(!role.grants(Scope::TenantAdmin));
            assert!(!role.grants(Scope::DocumentsRead));
            assert!(!role.grants(Scope::GraphWrite));
        }
        assert!(!Role::ArtifactAttestor.grants(Scope::PolicyAuthor));
        assert!(!Role::ArtifactAttestor.grants(Scope::PolicyApprove));
        assert!(!Role::ArtifactAttestor.grants(Scope::PromotionDecide));
        // Host-admin: tenant lifecycle ONLY — no data scopes at all.
        assert!(Role::HostAdmin.grants(Scope::TenantAdmin));
        assert!(!Role::HostAdmin.grants(Scope::DocumentsRead));
        assert!(!Role::HostAdmin.grants(Scope::Admin));
        assert!(!Role::Admin.grants(Scope::TenantAdmin));
    }

    #[test]
    fn artifact_attestor_role_and_scope_use_stable_wire_names() {
        assert_eq!(
            serde_json::to_string(&Role::ArtifactAttestor).unwrap(),
            "\"artifact-attestor\""
        );
        assert_eq!(
            serde_json::from_str::<Role>("\"artifact-attestor\"").unwrap(),
            Role::ArtifactAttestor
        );
        assert_eq!(
            serde_json::to_string(&Scope::ArtifactAttest).unwrap(),
            "\"artifact-attest\""
        );
        assert_eq!(
            serde_json::from_str::<Scope>("\"artifact-attest\"").unwrap(),
            Scope::ArtifactAttest
        );
    }

    #[tokio::test]
    async fn user_and_token_owner_lookups_are_key_bound() {
        let backend = std::sync::Arc::new(cognigraph_native::NativeBackend::new());
        let auth = AuthProvider::new(backend).await.unwrap();
        let user = auth
            .create_user("approver", "pw12345678", Role::PolicyApprover)
            .await
            .unwrap();
        let token = auth
            .create_token(&user.key, "approval", None)
            .await
            .unwrap();

        assert_eq!(
            auth.get_user(&user.key).await.unwrap().unwrap().role,
            Role::PolicyApprover
        );
        assert_eq!(
            auth.token_owner(&token.key).await.unwrap().as_deref(),
            Some(user.key.as_str())
        );
        assert!(auth.get_user("missing").await.unwrap().is_none());
        assert!(auth.token_owner("missing").await.unwrap().is_none());
    }

    #[test]
    fn token_hashing_is_stable_and_blind() {
        let a = hash_token("cg_abc");
        assert_eq!(a, hash_token("cg_abc"));
        assert_ne!(a, hash_token("cg_abd"));
        assert_eq!(a.len(), 64);
    }
}
