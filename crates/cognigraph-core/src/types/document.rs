use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Exact, case-sensitive identifier for a document within a collection.
/// Canonically equivalent Unicode strings may name distinct documents.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DocumentId {
    /// Collection name
    pub collection: String,
    /// Document key within the collection
    pub key: String,
}

impl DocumentId {
    pub fn new(collection: impl Into<String>, key: impl Into<String>) -> Self {
        Self {
            collection: collection.into(),
            key: key.into(),
        }
    }

    /// Returns the full document ID: "collection/key"
    pub fn full_id(&self) -> String {
        format!("{}/{}", self.collection, self.key)
    }
}

impl std::fmt::Display for DocumentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.collection, self.key)
    }
}

/// A document stored in the knowledge graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    /// Document key (unique within collection)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,

    /// Document title
    pub title: String,

    /// Document content
    pub content: String,

    /// Category for grouping
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,

    /// Word count (computed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub word_count: Option<usize>,

    /// Arbitrary metadata
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,

    /// Creation timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime<Utc>>,

    /// Last update timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<DateTime<Utc>>,
}

impl Document {
    pub fn new(title: impl Into<String>, content: impl Into<String>) -> Self {
        let content = content.into();
        let word_count = content.split_whitespace().count();
        Self {
            key: None,
            title: title.into(),
            content,
            category: None,
            word_count: Some(word_count),
            metadata: HashMap::new(),
            created_at: None,
            updated_at: None,
        }
    }
}

/// An edge (relationship) between two documents
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    /// Edge key
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,

    /// Source document ID (collection/key format)
    #[serde(rename = "_from")]
    pub from: String,

    /// Target document ID (collection/key format)
    #[serde(rename = "_to")]
    pub to: String,

    /// Relationship type (e.g., "related_to", "similar_to", "cites")
    pub relation_type: String,

    /// Confidence score (0.0 to 1.0)
    #[serde(default = "default_confidence")]
    pub confidence: f64,

    /// Who/what created this edge
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_by: Option<String>,

    /// Creation timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime<Utc>>,
}

fn default_confidence() -> f64 {
    1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- DocumentId ---

    #[test]
    fn document_id_new() {
        let id = DocumentId::new("docs", "abc123");
        assert_eq!(id.collection, "docs");
        assert_eq!(id.key, "abc123");
    }

    #[test]
    fn document_id_new_from_string() {
        let id = DocumentId::new(String::from("col"), String::from("key"));
        assert_eq!(id.collection, "col");
        assert_eq!(id.key, "key");
    }

    #[test]
    fn document_id_full_id() {
        let id = DocumentId::new("documents", "42");
        assert_eq!(id.full_id(), "documents/42");
    }

    #[test]
    fn document_id_display() {
        let id = DocumentId::new("nodes", "x");
        assert_eq!(format!("{}", id), "nodes/x");
    }

    #[test]
    fn document_id_equality() {
        let a = DocumentId::new("c", "k");
        let b = DocumentId::new("c", "k");
        assert_eq!(a, b);
    }

    #[test]
    fn document_id_serde_roundtrip() {
        let id = DocumentId::new("col", "key");
        let json = serde_json::to_string(&id).unwrap();
        let back: DocumentId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, back);
    }

    // --- Document ---

    #[test]
    fn document_new_sets_title_and_content() {
        let doc = Document::new("My Title", "hello world foo");
        assert_eq!(doc.title, "My Title");
        assert_eq!(doc.content, "hello world foo");
    }

    #[test]
    fn document_new_computes_word_count() {
        let doc = Document::new("T", "one two three four");
        assert_eq!(doc.word_count, Some(4));
    }

    #[test]
    fn document_new_empty_content_word_count() {
        let doc = Document::new("T", "");
        assert_eq!(doc.word_count, Some(0));
    }

    #[test]
    fn document_new_optional_fields_are_none() {
        let doc = Document::new("T", "content");
        assert!(doc.key.is_none());
        assert!(doc.category.is_none());
        assert!(doc.created_at.is_none());
        assert!(doc.updated_at.is_none());
        assert!(doc.metadata.is_empty());
    }

    // --- Edge ---

    #[test]
    fn edge_default_confidence() {
        let json = r#"{
            "_from": "a/1",
            "_to": "b/2",
            "relation_type": "related_to"
        }"#;
        let edge: Edge = serde_json::from_str(json).unwrap();
        assert_eq!(edge.confidence, 1.0);
    }

    #[test]
    fn edge_custom_confidence() {
        let json = r#"{
            "_from": "a/1",
            "_to": "b/2",
            "relation_type": "cites",
            "confidence": 0.5
        }"#;
        let edge: Edge = serde_json::from_str(json).unwrap();
        assert!((edge.confidence - 0.5).abs() < f64::EPSILON);
    }
}
