//! The construction loop over HTTP (docs/dataops/04, /05):
//!
//! - POST /api/construct/ingest — chunks + a stored space type →
//!   evidence-bound fact edges, the same `ingest_chunks` the library and
//!   examples use, with the space's ACCEPTED neurons applied (hints
//!   extend triggers, blockers veto; proposed/rejected/retired contribute
//!   nothing). Revision-safe: chunks and their independently keyed evidence
//!   occurrences are atomically replaced, while evidence from other chunks
//!   and spaces remains intact. Write scope.
//! - POST /api/construct/governed-ingest — the same bounded atomic write, but
//!   its effective configuration comes only from the exact independently
//!   approved M25 revision selected by the current promotion head. The
//!   process-local promotion lock stays held through the graph transaction so
//!   selection cannot race construction. This is invoked materialization, not
//!   an automatic graph-generation switch.
//! - POST /api/construct/evaluate — the MEASURE verb: recall (expected facts
//!   constructed) and restraint (forbidden facts NOT constructed)
//!   against the live graph. Deterministic, no model; POST for the body,
//!   but read-only in effect and mounted with read scope (its own
//!   router below), so a degradation check is a curl in a cron job.
//! - POST /api/construct/propose — the REPAIR verb: gap-directed neuron
//!   proposals (BM25 evidence, verbatim self-check) stored as `proposed`
//!   in the neurons collection — straight into the review queue with
//!   QW1 attribution. Needs a completion provider (OPENAI_API_KEY or
//!   GEMINI_API_KEY; model via COGNIGRAPH_COMPLETION_MODEL). Nothing it
//!   writes influences the graph until a human accepts it.

use axum::extract::{DefaultBodyLimit, Extension, State};
use axum::routing::post;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{Value, json};

use cognigraph_auth::User;
use cognigraph_construct::{
    AnswerOpts, Chunk, DirectedRelation, EvalSpec, Fact, Neuron, NeuronKind, NeuronSet,
    NeuronStatus, POLICY_REV, SpaceType, advise_gates, answer_eval_with, directed_ingest,
    effective_config, effective_vetoes, evaluate, ingest_chunks, judge_neuron,
    propose_neurons_via_backend, validate_neurons,
};
use cognigraph_core::CogniGraphError;

use crate::error::AppError;
use crate::jobs::JobManager;
use crate::promotions::PromotionTarget;
use crate::refusals::{RefusalContext, RefusalRow, attach, record_refusals};
use crate::state::AppState;
use crate::tenancy::current_tenant;

use super::neurons::{actor, load_accepted, load_space, now_secs, validate_acceptance};

const EVAL_SPECS: &str = "eval_specs";
const NEURONS: &str = "neurons";
const SPACE_TYPES: &str = "space_types";
const SPACE_TYPE_DRAFTS: &str = "space_type_drafts";
const MAX_GOVERNED_INGEST_CHUNKS: usize = 1_000;
/// One directed request is exactly one completion call over its chunks, so
/// the cap keeps a request inside the HTTP timeout; larger corpora are
/// submitted as consecutive slices (reconciliation is per chunk, so slicing
/// is safe and re-runs are idempotent).
const MAX_DIRECTED_CHUNKS: usize = 32;

#[cfg(test)]
#[path = "../construct_directed_tests.rs"]
pub(crate) mod directed_tests;

#[cfg(test)]
mod tests;

mod routing;
pub use routing::*;

mod directed;
use directed::*;

mod ingest;
use ingest::*;

mod governed_ingest;
use governed_ingest::*;

mod evaluation;
pub(crate) use evaluation::*;

mod draft_contracts;
pub(crate) use draft_contracts::*;

mod draft;
use draft::*;

mod draft_accept;
use draft_accept::*;

mod answer_evaluation;
use answer_evaluation::*;

mod gate_advice;
use gate_advice::*;

mod proposal;
use proposal::*;

mod review_policy;
use review_policy::*;

mod review;
use review::*;
