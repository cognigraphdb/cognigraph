use serde::{Deserialize, Serialize};

/// Direction for edge traversal
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    #[default]
    Outbound,
    Inbound,
    Any,
}

impl std::fmt::Display for Direction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Direction::Outbound => write!(f, "outbound"),
            Direction::Inbound => write!(f, "inbound"),
            Direction::Any => write!(f, "any"),
        }
    }
}

/// Collection type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CollectionType {
    /// Regular document collection
    Document,
    /// Edge collection for relationships
    Edge,
}

impl CollectionType {
    /// Lowercase wire name, matching the snapshot format ("document"/"edge").
    pub fn as_str(&self) -> &'static str {
        match self {
            CollectionType::Document => "document",
            CollectionType::Edge => "edge",
        }
    }
}

/// One entry of a backend's collection catalog (`list_collections`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionInfo {
    pub name: String,
    /// "document" or "edge" — the snapshot format's lowercase names.
    pub collection_type: String,
    /// Number of stored documents/edges.
    pub count: u64,
}

/// Query language accepted by a backend's raw query entry point.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum QueryLanguage {
    /// ArangoDB Query Language.
    Aql,
    /// CogniGraph Query Language.
    Cgql,
    /// Backend-native query language not represented by a first-class variant.
    BackendNative(String),
}

impl QueryLanguage {
    pub fn as_str(&self) -> &str {
        match self {
            QueryLanguage::Aql => "aql",
            QueryLanguage::Cgql => "cgql",
            QueryLanguage::BackendNative(name) => name.as_str(),
        }
    }
}

/// Options for graph traversal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraversalOpts {
    /// Maximum traversal depth
    #[serde(default = "default_max_depth")]
    pub max_depth: u32,

    /// Minimum traversal depth
    #[serde(default = "default_min_depth")]
    pub min_depth: u32,

    /// Traversal direction
    #[serde(default)]
    pub direction: Direction,

    /// Edge collection to traverse
    pub edge_collection: String,

    /// Inclusive minimum confidence for every edge in a path. A failing edge
    /// excludes that path and all extensions, including before `min_depth`.
    /// Missing, null, and nonnumeric confidence count as 1.0. Depth zero is
    /// unaffected because it has no edges. This does not filter path scores.
    #[serde(default)]
    pub min_confidence: Option<f64>,

    /// Per-hop score decay factor. The path score is the product of edge
    /// confidences multiplied by this factor once per traversed edge.
    #[serde(default = "default_path_decay")]
    pub path_decay: f64,
}

fn default_max_depth() -> u32 {
    3
}
fn default_min_depth() -> u32 {
    1
}
fn default_path_decay() -> f64 {
    0.8
}

/// A path discovered during graph traversal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraversalPath {
    /// Vertices along the path
    pub vertices: Vec<serde_json::Value>,
    /// Edges along the path
    pub edges: Vec<serde_json::Value>,
    /// Path length (number of edges)
    pub depth: usize,
    /// `product(edge confidence, default 1.0) * path_decay.pow(depth)`.
    pub score: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Direction ---

    #[test]
    fn direction_display() {
        assert_eq!(format!("{}", Direction::Outbound), "outbound");
        assert_eq!(format!("{}", Direction::Inbound), "inbound");
        assert_eq!(format!("{}", Direction::Any), "any");
    }

    #[test]
    fn direction_default_is_outbound() {
        assert_eq!(Direction::default(), Direction::Outbound);
    }

    #[test]
    fn direction_serde_roundtrip() {
        for dir in [Direction::Outbound, Direction::Inbound, Direction::Any] {
            let json = serde_json::to_string(&dir).unwrap();
            let back: Direction = serde_json::from_str(&json).unwrap();
            assert_eq!(dir, back);
        }
    }

    #[test]
    fn direction_serde_lowercase() {
        let json = serde_json::to_string(&Direction::Outbound).unwrap();
        assert_eq!(json, r#""outbound""#);
        let json = serde_json::to_string(&Direction::Inbound).unwrap();
        assert_eq!(json, r#""inbound""#);
        let json = serde_json::to_string(&Direction::Any).unwrap();
        assert_eq!(json, r#""any""#);
    }

    // --- TraversalOpts ---

    #[test]
    fn traversal_opts_serde_defaults() {
        let json = r#"{"edge_collection": "edges"}"#;
        let opts: TraversalOpts = serde_json::from_str(json).unwrap();
        assert_eq!(opts.max_depth, 3);
        assert_eq!(opts.min_depth, 1);
        assert!((opts.path_decay - 0.8).abs() < f64::EPSILON);
        assert_eq!(opts.direction, Direction::Outbound);
        assert!(opts.min_confidence.is_none());
    }

    // --- CollectionType ---

    #[test]
    fn collection_type_variants() {
        let doc = CollectionType::Document;
        let edge = CollectionType::Edge;
        assert_eq!(doc, CollectionType::Document);
        assert_eq!(edge, CollectionType::Edge);
        assert_ne!(doc, edge);
    }

    #[test]
    fn collection_type_serde_roundtrip() {
        for ct in [CollectionType::Document, CollectionType::Edge] {
            let json = serde_json::to_string(&ct).unwrap();
            let back: CollectionType = serde_json::from_str(&json).unwrap();
            assert_eq!(ct, back);
        }
    }
}
