use serde::{Deserialize, Serialize};

/// One operation in an atomic batch (see `GraphBackend::execute_batch`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "lowercase")]
pub enum BatchOp {
    Insert {
        collection: String,
        doc: serde_json::Value,
    },
    /// Partial update (merge), like `update_document`.
    Update {
        collection: String,
        key: String,
        merge: serde_json::Value,
    },
    Replace {
        collection: String,
        key: String,
        doc: serde_json::Value,
    },
    Delete {
        collection: String,
        key: String,
    },
}

/// Index definition for a collection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexDef {
    /// Index type
    pub index_type: IndexType,
    /// Fields to index
    pub fields: Vec<String>,
    /// Whether the index is unique
    #[serde(default)]
    pub unique: bool,
    /// Whether the index is sparse
    #[serde(default)]
    pub sparse: bool,
    /// Optional index name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// Supported index types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IndexType {
    Persistent,
    Hash,
    Fulltext,
    Geo,
    Ttl,
    Inverted,
    Vector,
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- IndexType ---

    #[test]
    fn index_type_serde_lowercase() {
        let json = serde_json::to_string(&IndexType::Persistent).unwrap();
        assert_eq!(json, r#""persistent""#);

        let json = serde_json::to_string(&IndexType::Vector).unwrap();
        assert_eq!(json, r#""vector""#);

        let json = serde_json::to_string(&IndexType::Fulltext).unwrap();
        assert_eq!(json, r#""fulltext""#);
    }

    #[test]
    fn index_type_deserialize_lowercase() {
        let t: IndexType = serde_json::from_str(r#""hash""#).unwrap();
        assert!(matches!(t, IndexType::Hash));

        let t: IndexType = serde_json::from_str(r#""geo""#).unwrap();
        assert!(matches!(t, IndexType::Geo));

        let t: IndexType = serde_json::from_str(r#""ttl""#).unwrap();
        assert!(matches!(t, IndexType::Ttl));

        let t: IndexType = serde_json::from_str(r#""inverted""#).unwrap();
        assert!(matches!(t, IndexType::Inverted));
    }
}
