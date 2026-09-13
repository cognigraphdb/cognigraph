use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: &str = "cognigraph-webnlg-pilot-v1";

#[derive(Debug, Clone, Deserialize)]
pub struct ViewerResponse {
    pub rows: Vec<ViewerRow>,
    pub num_rows_total: usize,
    pub partial: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ViewerRow {
    pub row_idx: usize,
    pub row: SourceRow,
    #[serde(default)]
    pub truncated_cells: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SourceRow {
    pub gem_id: String,
    pub gem_parent_id: String,
    pub input: Vec<String>,
    pub target: String,
    #[serde(default)]
    pub references: Vec<String>,
    pub category: String,
    pub webnlg_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OracleTriple {
    pub subject: String,
    pub predicate: String,
    pub object: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PilotDocument {
    pub schema_version: String,
    pub document_id: String,
    pub parent_id: String,
    pub split: String,
    pub category: String,
    pub webnlg_id: String,
    pub source_row_index: usize,
    pub source_url: String,
    pub text: String,
    pub char_count: usize,
    pub text_sha256: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OracleRecord {
    pub schema_version: String,
    pub document_id: String,
    pub split: String,
    pub triples: Vec<OracleTriple>,
    pub references: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ManifestEntry {
    pub document_id: String,
    pub split: String,
    pub source_row_index: usize,
    pub category: String,
    pub webnlg_id: String,
    pub triple_count: usize,
    pub char_count: usize,
    pub text_sha256: String,
    pub documents_path: String,
    pub oracle_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunRequest {
    pub dataset: String,
    pub config: String,
    pub hub_revision: String,
    pub splits: Vec<String>,
    pub page_size: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SplitSummary {
    pub split: String,
    pub rows: usize,
    pub triples: usize,
    pub unique_predicates: usize,
    pub unique_categories: usize,
    pub raw_sha256: String,
    pub documents_sha256: String,
    pub oracle_sha256: String,
    pub manifest_sha256: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RunMetadata {
    pub schema_version: String,
    pub dataset: String,
    pub config: String,
    pub hub_revision: String,
    pub license: String,
    pub source: String,
    pub generated_at: String,
    pub splits: Vec<SplitSummary>,
    pub total_documents: usize,
    pub total_triples: usize,
    pub unique_predicates: usize,
    pub unique_categories: usize,
    pub challenge_splits_excluded: Vec<String>,
}
