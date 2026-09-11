//! Blinded domain-expert reference tooling for the DailyMed clinical pass.
//!
//! Candidate generation is mechanical and cannot assign gold labels. Two
//! experts label independently; validation and adjudication must succeed
//! before grounding or judge quality can be measured.

mod calibration;
mod matcher;
mod model;
mod prepare;
mod review;
mod score;
mod typing;
mod vocabulary;

pub use calibration::{
    CALIBRATION_SCHEMA_VERSION, CalibrationArtifact, CalibrationCase, calibration_digest,
    load_calibration,
};
pub use matcher::{ClinicalAssertion, MATCHER_VERSION, match_packet, match_section};
pub use model::*;
pub use prepare::{assert_workspace_fresh, prepare_reference, product_from_title};
pub use review::{compare_annotations, compile_eval_spec, read_jsonl, validate_annotations};
pub use score::{
    FROZEN_GENERIC_VOCABULARY, JUDGE_POLICY_THRESHOLD, VocabularyLane, chunks_for_packet,
    finalize_judge_score, grounded_concepts, neuron_for_case, score_grounding,
};
pub use typing::{DrugLexicon, build_drug_lexicon, is_condition};
pub use vocabulary::{
    ClinicalVocabulary, DEFAULT_MIN_DOCUMENT_FREQUENCY, LossReport, build_vocabulary,
    compute_losses,
};
