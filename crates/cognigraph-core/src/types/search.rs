use serde::{Deserialize, Serialize};

/// Options for vector similarity search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorSearchOpts {
    /// Minimum similarity threshold (0.0 to 1.0). `None` disables
    /// threshold filtering entirely.
    #[serde(default = "default_threshold")]
    pub threshold: Option<f64>,

    /// Maximum number of results
    #[serde(default = "default_limit")]
    pub limit: usize,

    /// Exact, case-sensitive embedding model filter, applied before candidate
    /// truncation. `None` searches all models; an empty string matches only
    /// rows whose model name is the empty string.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_name: Option<String>,
}

fn default_threshold() -> Option<f64> {
    Some(0.7)
}
fn default_limit() -> usize {
    10
}

/// A search result with similarity score
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    /// The matching document
    pub document: serde_json::Value,
    /// Similarity score
    pub score: f64,
    /// How the result was discovered
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

/// An embedding record stored alongside documents
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Embedding {
    /// Reference to the parent document
    pub document_id: String,
    /// Embedding vector
    pub embedding: Vec<f64>,
    /// Vector dimension
    pub dimension: usize,
    /// Model used to generate this embedding
    pub model_name: String,
    /// Chunk index (for chunked documents)
    #[serde(default)]
    pub chunk_index: usize,
    /// The text chunk this embedding represents
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunk_text: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- VectorSearchOpts ---

    #[test]
    fn vector_search_opts_serde_defaults() {
        let json = r#"{}"#;
        let opts: VectorSearchOpts = serde_json::from_str(json).unwrap();
        assert!((opts.threshold.unwrap() - 0.7).abs() < f64::EPSILON);
        assert_eq!(opts.limit, 10);
        assert!(opts.model_name.is_none());
    }

    #[test]
    fn vector_search_opts_custom_values() {
        let json = r#"{"threshold": 0.9, "limit": 5, "model_name": "ada-002"}"#;
        let opts: VectorSearchOpts = serde_json::from_str(json).unwrap();
        assert!((opts.threshold.unwrap() - 0.9).abs() < f64::EPSILON);
        assert_eq!(opts.limit, 5);
        assert_eq!(opts.model_name.as_deref(), Some("ada-002"));
    }

    // --- SearchHit ---

    #[test]
    fn search_hit_construction() {
        let hit = SearchHit {
            document: serde_json::json!({"title": "test"}),
            score: 0.95,
            source: Some("vector".to_string()),
        };
        assert!((hit.score - 0.95).abs() < f64::EPSILON);
        assert_eq!(hit.source.as_deref(), Some("vector"));
    }

    #[test]
    fn search_hit_no_source() {
        let hit = SearchHit {
            document: serde_json::json!({}),
            score: 0.5,
            source: None,
        };
        assert!(hit.source.is_none());
    }
}
