use tracing::debug;

use cognigraph_core::{CogniGraphError, Result};

use crate::client::{ArangoAuth, ArangoClient, ArangoError};

mod graph_backend;
mod vector_search;

#[cfg(test)]
mod tests;

/// Vector search strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VectorSearchMode {
    /// Use `APPROX_NEAR_COSINE` with a native vector index (default, Enterprise).
    /// Requires `--vector-index true` on the ArangoDB server and a vector index on
    /// the embeddings collection. Model-filtered searches require ArangoDB
    /// 3.12.6 or newer for filtering during the index lookup; use `Fallback`
    /// on older servers.
    Native,
    /// Compute cosine similarity in AQL (full collection scan — slow at scale).
    /// Use only when the vector index feature is unavailable.
    Fallback,
}

/// ArangoDB implementation of the `GraphBackend` trait.
pub struct ArangoBackend {
    client: ArangoClient,
    vector_mode: VectorSearchMode,
}

impl ArangoBackend {
    pub fn new(client: ArangoClient) -> Self {
        Self {
            client,
            vector_mode: VectorSearchMode::Native,
        }
    }

    /// Connect to ArangoDB with basic auth.
    pub fn connect(
        base_url: impl Into<String>,
        database: impl Into<String>,
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> Self {
        let client = ArangoClient::new(
            base_url,
            database,
            ArangoAuth::Basic {
                username: username.into(),
                password: password.into(),
            },
        );
        Self {
            client,
            vector_mode: VectorSearchMode::Native,
        }
    }

    /// Set the vector search mode.
    pub fn with_vector_mode(mut self, mode: VectorSearchMode) -> Self {
        self.vector_mode = mode;
        self
    }

    /// Access the underlying ArangoDB client for advanced operations.
    pub fn client(&self) -> &ArangoClient {
        &self.client
    }

    /// Create a vector index on a collection. Requires at least one document to
    /// already exist in the collection (ArangoDB needs data for the training step).
    pub async fn create_vector_index(
        &self,
        collection: &str,
        field: &str,
        dimension: usize,
        name: Option<&str>,
    ) -> Result<()> {
        let def = serde_json::json!({
            "type": "vector",
            "name": name.unwrap_or("idx_vector"),
            "fields": [field],
            "params": {
                "dimension": dimension,
                "metric": "cosine",
                "nLists": 1,
            }
        });

        match self.client.create_index(collection, &def).await {
            Ok(_) => {
                debug!(collection, field, dimension, "Vector index created");
                Ok(())
            }
            // Already exists
            Err(ArangoError::Server { code: 409, .. }) => Ok(()),
            Err(e) => Err(map_err(e)),
        }
    }
}

/// Convert ArangoError into CogniGraphError.
fn map_err(e: ArangoError) -> CogniGraphError {
    match e {
        ArangoError::Server { code: 404, message } => {
            if message.contains("collection") || message.contains("Collection") {
                CogniGraphError::CollectionNotFound(message)
            } else {
                CogniGraphError::BackendError(format!("ArangoDB 404: {message}"))
            }
        }
        ArangoError::Server { code: 401, message } | ArangoError::Server { code: 403, message } => {
            CogniGraphError::AuthError(message)
        }
        ArangoError::Server { code: 409, message } => CogniGraphError::DocumentConflict(message),
        ArangoError::Server { message, .. } => CogniGraphError::BackendError(message),
        ArangoError::Network(e) => CogniGraphError::ConnectionError(e.to_string()),
        ArangoError::Http { status, body } => {
            CogniGraphError::BackendError(format!("HTTP {status}: {body}"))
        }
        ArangoError::Deserialization { message, .. } => {
            CogniGraphError::BackendError(format!("Deserialization: {message}"))
        }
    }
}

/// Extract `_key` from an ArangoDB response JSON.
fn extract_key(val: &serde_json::Value) -> Option<String> {
    val.get("_key")
        .or_else(|| val.get("new").and_then(|n| n.get("_key")))
        .and_then(|v| v.as_str())
        .map(String::from)
}

/// Extract the document body from a response that may contain `new` wrapper.
fn extract_doc(val: serde_json::Value) -> serde_json::Value {
    if let Some(new) = val.get("new") {
        new.clone()
    } else {
        val
    }
}
