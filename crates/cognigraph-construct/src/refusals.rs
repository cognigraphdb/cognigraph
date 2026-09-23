//! Structured gate refusals (CG-90). A refusal is what a deterministic gate
//! declined to build, with enough identity to be stored and traced: the gate,
//! the nominated triple, the chunk and the evidence quote as submitted (NFC),
//! plus the human-readable reason that callers already received as a string.

use serde::{Deserialize, Serialize};

/// The directed-construction gate that refused a proposal, in gate order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DirectedGate {
    RelationNotInTaxonomy,
    ChunkNotInRequest,
    EmptyEndpoint,
    UnusableEndpointIdentity,
    EvidenceNotVerbatim,
    SourceNotInSentence,
    TargetNotInSentence,
    VocabularyNotAffirmed,
}

impl DirectedGate {
    /// Stable snake_case identifier, the value stored on ledger rows.
    pub fn code(self) -> &'static str {
        match self {
            Self::RelationNotInTaxonomy => "relation_not_in_taxonomy",
            Self::ChunkNotInRequest => "chunk_not_in_request",
            Self::EmptyEndpoint => "empty_endpoint",
            Self::UnusableEndpointIdentity => "unusable_endpoint_identity",
            Self::EvidenceNotVerbatim => "evidence_not_verbatim",
            Self::SourceNotInSentence => "source_not_in_sentence",
            Self::TargetNotInSentence => "target_not_in_sentence",
            Self::VocabularyNotAffirmed => "vocabulary_not_affirmed",
        }
    }
}

/// One refused nomination. Fields hold the proposal as the gate saw it:
/// NFC-canonical text, untrimmed, so the record matches what was judged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Refusal {
    pub gate: DirectedGate,
    pub source: String,
    pub relation: String,
    pub target: String,
    pub chunk_id: String,
    pub evidence: String,
    /// The pre-existing human-readable skip text, unchanged.
    pub reason: String,
}
