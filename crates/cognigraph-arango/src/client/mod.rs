use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use serde::Deserialize;
use std::collections::HashMap;

/// Minimal ArangoDB HTTP client.
///
/// Inspired by the team's Lua ArangoDB client — bare minimum to make things work.
/// Wraps ArangoDB's REST API directly without ORM overhead.
pub struct ArangoClient {
    http: reqwest::Client,
    base_url: String,
    database: String,
    auth_header: HeaderValue,
}

/// Authentication method for ArangoDB
pub enum ArangoAuth {
    /// Basic authentication with username and password
    Basic { username: String, password: String },
    /// Bearer token (JWT) authentication
    Bearer(String),
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ArangoResponse<T> {
    #[serde(default)]
    error: bool,
    #[serde(default)]
    code: u16,
    #[serde(rename = "errorMessage")]
    #[serde(default)]
    error_message: Option<String>,
    #[serde(flatten)]
    result: Option<T>,
}

#[derive(Debug, Deserialize)]
struct CursorResponse {
    result: Vec<serde_json::Value>,
    #[serde(rename = "hasMore")]
    #[serde(default)]
    has_more: bool,
    #[serde(default)]
    id: Option<String>,
}

impl ArangoClient {
    /// Create a new ArangoDB client.
    ///
    /// # Arguments
    /// * `base_url` - ArangoDB server URL (e.g., "http://localhost:8529")
    /// * `database` - Database name to use
    /// * `auth` - Authentication credentials
    pub fn new(base_url: impl Into<String>, database: impl Into<String>, auth: ArangoAuth) -> Self {
        let auth_header = match &auth {
            ArangoAuth::Basic { username, password } => {
                use base64::Engine;
                let encoded = base64::engine::general_purpose::STANDARD
                    .encode(format!("{username}:{password}"));
                HeaderValue::from_str(&format!("Basic {encoded}")).unwrap()
            }
            ArangoAuth::Bearer(token) => HeaderValue::from_str(&format!("bearer {token}")).unwrap(),
        };

        Self {
            http: reqwest::Client::new(),
            base_url: base_url.into().trim_end_matches('/').to_string(),
            database: database.into(),
            auth_header,
        }
    }

    /// Build the database-prefixed URL for an API endpoint.
    fn db_url(&self, path: &str) -> String {
        format!("{}/_db/{}{}", self.base_url, self.database, path)
    }

    /// Build a server-level URL (no database prefix).
    fn server_url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    /// Get default headers for requests.
    fn headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, self.auth_header.clone());
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers
    }

    // --- Document operations ---

    /// Create a document in a collection.
    pub async fn create_document(
        &self,
        collection: &str,
        document: &serde_json::Value,
    ) -> Result<serde_json::Value, ArangoError> {
        let url = self.db_url(&format!("/_api/document/{collection}"));
        let resp = self
            .http
            .post(&url)
            .headers(self.headers())
            .query(&[("returnNew", "true")])
            .json(document)
            .send()
            .await?;
        self.parse_response(resp).await
    }

    /// Get a document by key.
    pub async fn get_document(
        &self,
        collection: &str,
        key: &str,
    ) -> Result<serde_json::Value, ArangoError> {
        let url = self.db_url(&format!("/_api/document/{collection}/{key}"));
        let resp = self.http.get(&url).headers(self.headers()).send().await?;
        self.parse_response(resp).await
    }

    /// Update a document (partial, PATCH).
    pub async fn update_document(
        &self,
        collection: &str,
        key: &str,
        update: &serde_json::Value,
    ) -> Result<serde_json::Value, ArangoError> {
        let url = self.db_url(&format!("/_api/document/{collection}/{key}"));
        let resp = self
            .http
            .patch(&url)
            .headers(self.headers())
            .query(&[("returnNew", "true"), ("mergeObjects", "true")])
            .json(update)
            .send()
            .await?;
        self.parse_response(resp).await
    }

    /// Replace a document entirely (PUT).
    pub async fn replace_document(
        &self,
        collection: &str,
        key: &str,
        document: &serde_json::Value,
    ) -> Result<serde_json::Value, ArangoError> {
        let url = self.db_url(&format!("/_api/document/{collection}/{key}"));
        let resp = self
            .http
            .put(&url)
            .headers(self.headers())
            .query(&[("returnNew", "true")])
            .json(document)
            .send()
            .await?;
        self.parse_response(resp).await
    }

    /// Delete a document by key.
    pub async fn delete_document(
        &self,
        collection: &str,
        key: &str,
    ) -> Result<serde_json::Value, ArangoError> {
        let url = self.db_url(&format!("/_api/document/{collection}/{key}"));
        let resp = self
            .http
            .delete(&url)
            .headers(self.headers())
            .send()
            .await?;
        self.parse_response(resp).await
    }

    // --- AQL Query ---

    /// Execute an AQL query with bind variables.
    /// Automatically follows cursors to fetch all results.
    pub async fn query(
        &self,
        aql: &str,
        bind_vars: HashMap<String, serde_json::Value>,
    ) -> Result<Vec<serde_json::Value>, ArangoError> {
        let url = self.db_url("/_api/cursor");
        let body = serde_json::json!({
            "query": aql,
            "bindVars": bind_vars,
        });

        let resp = self
            .http
            .post(&url)
            .headers(self.headers())
            .json(&body)
            .send()
            .await?;

        let cursor: CursorResponse = self.parse_response(resp).await?;
        let mut results = cursor.result;

        // Follow cursor pagination
        if cursor.has_more
            && let Some(cursor_id) = cursor.id
        {
            let mut has_more = true;
            while has_more {
                let next_url = self.db_url(&format!("/_api/cursor/{cursor_id}"));
                let next_resp = self
                    .http
                    .put(&next_url)
                    .headers(self.headers())
                    .send()
                    .await?;
                let next: CursorResponse = self.parse_response(next_resp).await?;
                results.extend(next.result);
                has_more = next.has_more;
            }
        }

        Ok(results)
    }

    // --- Collection management ---

    /// Create a collection. `collection_type`: 2 = document, 3 = edge.
    pub async fn create_collection(
        &self,
        name: &str,
        collection_type: u8,
    ) -> Result<serde_json::Value, ArangoError> {
        let url = self.db_url("/_api/collection");
        let body = collection_create_body(name, collection_type);
        let resp = self
            .http
            .post(&url)
            .headers(self.headers())
            .json(&body)
            .send()
            .await?;
        self.parse_response(resp).await
    }

    /// List all collections (excluding system collections).
    pub async fn list_collections(&self) -> Result<serde_json::Value, ArangoError> {
        let url = self.db_url("/_api/collection");
        let resp = self
            .http
            .get(&url)
            .headers(self.headers())
            .query(&[("excludeSystem", "true")])
            .send()
            .await?;
        self.parse_response(resp).await
    }

    /// Drop a collection.
    pub async fn drop_collection(&self, name: &str) -> Result<serde_json::Value, ArangoError> {
        let url = self.db_url(&collection_drop_path(name));
        let resp = self
            .http
            .delete(&url)
            .headers(self.headers())
            .send()
            .await?;
        self.parse_response(resp).await
    }

    /// Truncate a collection (remove all documents).
    pub async fn truncate_collection(&self, name: &str) -> Result<serde_json::Value, ArangoError> {
        let url = self.db_url(&format!("/_api/collection/{name}/truncate"));
        let resp = self.http.put(&url).headers(self.headers()).send().await?;
        self.parse_response(resp).await
    }

    // --- Index management ---

    /// Create an index on a collection.
    pub async fn create_index(
        &self,
        collection: &str,
        index_def: &serde_json::Value,
    ) -> Result<serde_json::Value, ArangoError> {
        let url = self.db_url(&format!("/_api/index?collection={collection}"));
        let resp = self
            .http
            .post(&url)
            .headers(self.headers())
            .json(index_def)
            .send()
            .await?;
        self.parse_response(resp).await
    }

    /// List indexes on a collection.
    pub async fn list_indexes(&self, collection: &str) -> Result<serde_json::Value, ArangoError> {
        let url = self.db_url(&format!("/_api/index?collection={collection}"));
        let resp = self.http.get(&url).headers(self.headers()).send().await?;
        self.parse_response(resp).await
    }

    // --- Server info ---

    /// Get server version.
    pub async fn version(&self) -> Result<serde_json::Value, ArangoError> {
        let url = self.server_url("/_api/version");
        let resp = self.http.get(&url).headers(self.headers()).send().await?;
        self.parse_response(resp).await
    }

    // --- Internal helpers ---

    async fn parse_response<T: serde::de::DeserializeOwned>(
        &self,
        resp: reqwest::Response,
    ) -> Result<T, ArangoError> {
        let status = resp.status();
        let body = resp.text().await?;

        if status.is_client_error() || status.is_server_error() {
            // Try to extract ArangoDB error message
            if let Ok(err_resp) = serde_json::from_str::<ArangoErrorResponse>(&body) {
                return Err(ArangoError::Server {
                    code: err_resp.code,
                    message: err_resp.error_message,
                });
            }
            return Err(ArangoError::Http {
                status: status.as_u16(),
                body,
            });
        }

        serde_json::from_str(&body).map_err(|e| ArangoError::Deserialization {
            message: e.to_string(),
            body,
        })
    }
}

/// ArangoDB reserves underscore-prefixed names for system collections. Its
/// collection API requires an explicit flag both when creating and dropping
/// one; ordinary collection requests retain their original shape.
fn collection_create_body(name: &str, collection_type: u8) -> serde_json::Value {
    let mut body = serde_json::json!({
        "name": name,
        "type": collection_type,
    });
    if name.starts_with('_') {
        body["isSystem"] = serde_json::Value::Bool(true);
    }
    body
}

fn collection_drop_path(name: &str) -> String {
    let path = format!("/_api/collection/{name}");
    if name.starts_with('_') {
        format!("{path}?isSystem=true")
    } else {
        path
    }
}

/// Switch database context
impl ArangoClient {
    /// Create a new client pointing to a different database.
    pub fn use_database(&self, database: impl Into<String>) -> Self {
        Self {
            http: self.http.clone(),
            base_url: self.base_url.clone(),
            database: database.into(),
            auth_header: self.auth_header.clone(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct ArangoErrorResponse {
    #[serde(default)]
    code: u16,
    #[serde(rename = "errorMessage", default)]
    error_message: String,
}

/// Errors from the ArangoDB client
#[derive(Debug, thiserror::Error)]
pub enum ArangoError {
    #[error("ArangoDB server error ({code}): {message}")]
    Server { code: u16, message: String },

    #[error("HTTP error ({status}): {body}")]
    Http { status: u16, body: String },

    #[error("Deserialization error: {message}\nBody: {body}")]
    Deserialization { message: String, body: String },

    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
}

#[cfg(test)]
mod tests;
