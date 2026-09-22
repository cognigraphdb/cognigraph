//! Governed knowledge construction for CogniGraph — the Semantic Neurons
//! pattern ported from the research (see
//! docs/architecture/design-notes/semantic-neurons-port.md and the paper it
//! cites).
//!
//! An LLM reliably extracts entities but not relations; this crate supplies
//! the construction layer: an ontology (space types), governed
//! edge-construction operators (neurons: proposed → accepted | rejected, and
//! later retired; graduation is a leave-one-out redundancy report, not a
//! status), negation-aware evidence grounding, and an evaluation loop that
//! measures both recall (expected facts built) and restraint (forbidden facts
//! NOT built).

pub mod advisor;
pub mod answers;
pub mod clinical_reference;
pub mod derivation;
pub mod directed;
pub mod draft;
pub mod eval;
mod evidence;
pub mod grounding;
pub mod ingest;
pub mod judge;
pub mod materialization;
pub mod preparation;
pub mod propose;
pub mod rank;
pub mod report;
pub mod sideviews;
pub mod types;
pub mod validate;
pub mod webnlg;

pub use advisor::{
    BlockedSample, GateAdvice, GateAdvisorReport, GateOutcome, SemanticsSample, advise_gates,
};
pub use answers::{AnswerOpts, AnswerOutcome, QuestionScore, answer_eval, answer_eval_with};
pub use derivation::{DerivationError, DerivationOptions, DerivedFactRow, derive_fact_rows};
pub use directed::{DirectedOutcome, DirectedRelation, directed_ingest};
pub use draft::{
    DRAFT_REV, DraftReport, draft_space_type, draft_space_type_per_document,
    empty_per_document_draft, finalize_draft, merge_drafted,
};
pub use eval::{EvalOutcome, evaluate, evaluate_facts};
pub use grounding::{
    GroundedFact, SEMANTICS_REV, VetoRule, affirms_phrase, effective_config, effective_vetoes,
    ground_chunk, mentions, relation_semantics_signals,
};
pub use ingest::{FACT_SEMANTICS_COLLECTION, ingest_chunks};
pub use judge::{
    JUDGE_SYSTEM, JudgeVerdict, POLICY_REV, SCREEN_SYSTEM, judge_neuron, judge_schema,
    screen_schema,
};
pub use materialization::{
    MaterializationError, MaterializationOptions, MaterializedChunkRow, MaterializedEntityRow,
    MaterializedFactOccurrenceRow, MaterializedGraphProjection, MaterializedMentionRow,
    derive_materialized_graph,
};
pub use preparation::{
    PREPARATION_UNICODE_VERSION, PreparationError, PreparationOptions, PreparedChunkRow,
    RawDocumentBytes, prepare_documents,
};
pub use propose::{
    BlockerCoverage, BlockerProposalReport, FactCoverage, IteratedBlockerReport, ProposalReport,
    ProposalSkip, propose_blockers_covering, propose_blockers_report, propose_neurons,
    propose_neurons_report, propose_neurons_via_backend,
};
pub use rank::{EdgeSelectOpts, GraphEdge, rank_boosts, relation_score, select_graph_edges};
pub use report::{
    AblationEntry, AblationStatus, BlockerEntry, BlockerStatus, DegradedPathway,
    GraduationCandidate, GraduationReason, PathwayKind, ablation_report, blocker_report,
    degradation_report, graduation_report,
};
pub use sideviews::{
    QaPair, SIDEVIEWS_SYSTEM, generate_sideviews, sideviews_schema, sideviews_user,
};
pub use types::*;
pub use validate::validate_neurons;
pub use webnlg::{Corpus, Lane, PredicateScore, WebnlgScore, load_corpus, score_documents};
