use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: &str = "dailymed-clinical-reference-v1";
pub const TREATS: &str = "TREATS";
pub const CONTRAINDICATED_IN: &str = "CONTRAINDICATED_IN";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewPacket {
    pub schema_version: String,
    pub set_id: String,
    pub spl_version: u64,
    pub selection_rank: u64,
    pub title: String,
    pub product: String,
    pub source_url: String,
    pub split: ReviewSplit,
    pub sections: Vec<ClinicalSection>,
    #[serde(default)]
    pub candidates: Vec<CandidateFact>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewSplit {
    Calibration,
    Evaluation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClinicalSection {
    pub code: String,
    pub title: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateFact {
    pub relation: String,
    pub condition: String,
    pub section_code: String,
    pub evidence: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnotationRecord {
    pub schema_version: String,
    pub set_id: String,
    pub annotator: String,
    pub complete: bool,
    #[serde(default)]
    pub facts: Vec<ClinicalFactLabel>,
    #[serde(default)]
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClinicalFactLabel {
    pub relation: String,
    pub condition: String,
    pub verdict: ClinicalVerdict,
    pub section_code: String,
    pub evidence: String,
    #[serde(default)]
    pub notes: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClinicalVerdict {
    Unreviewed,
    True,
    False,
    Uncertain,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClinicalScore {
    /// Which condition vocabulary the grounder was given. Never omit this:
    /// an oracle-vocabulary score is not comparable to an end-to-end one.
    #[serde(default)]
    pub lane: String,
    #[serde(default)]
    pub end_to_end: bool,
    pub documents: usize,
    pub true_positive: usize,
    pub false_negative: usize,
    pub true_negative: usize,
    pub false_positive: usize,
    pub uncertain_excluded: usize,
    pub recall: Option<f64>,
    pub restraint: Option<f64>,
    pub precision: Option<f64>,
    pub by_relation: std::collections::BTreeMap<String, RelationScore>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RelationScore {
    pub true_positive: usize,
    pub false_negative: usize,
    pub true_negative: usize,
    pub false_positive: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JudgeCaseResult {
    pub set_id: String,
    pub relation: String,
    pub condition: String,
    pub gold: ClinicalVerdict,
    pub verdict: String,
    pub confidence: f64,
    pub accepted: bool,
    pub reasoning: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JudgeScore {
    pub policy_threshold: f64,
    pub cases: usize,
    pub true_accepted: usize,
    pub true_rejected: usize,
    pub false_rejected: usize,
    pub false_accepted: usize,
    pub needs_human: usize,
    pub accepted_precision: Option<f64>,
    pub results: Vec<JudgeCaseResult>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ManifestEntry {
    pub set_id: String,
    pub spl_version: u64,
    pub title: String,
    pub source_url: String,
    pub document_path: String,
    pub selection_rank: u64,
}

#[derive(Debug, Deserialize)]
pub(crate) struct NormalizedDoc {
    pub sections: Vec<ClinicalSection>,
}
