//! M18 evaluation-promotion contract and durable control-plane manager.
//!
//! Evaluation jobs remain measurements. A promotion evidence bundle freezes
//! four succeeded deterministic runs (candidate + baseline, each with an
//! independent replay), evaluates an authored policy without floating point,
//! and can then receive an attributed Admin decision. Immutable evidence and
//! decisions are authority; the current target head is a repairable projection.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use cognigraph_core::{
    CogniGraphError, CollectionType, FieldPredicate, GraphBackend, IndexDef, IndexType,
    PredicateOp, QueryLanguage,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use unicode_normalization::{UnicodeNormalization, is_nfc};

use crate::jobs::{JOB_ARCHIVE_COLLECTION, JOBS_COLLECTION, JobManager};

pub const EVIDENCE_COLLECTION: &str = "_cognigraph_evaluation_evidence";
pub const DECISIONS_COLLECTION: &str = "_cognigraph_promotion_decisions";
pub const HEADS_COLLECTION: &str = "_cognigraph_promotion_heads";
pub const PROMOTION_CONTEXT_SCHEMA_VERSION: u32 = 1;
pub const M19_PROMOTION_CONTEXT_SCHEMA_VERSION: u32 = 2;
pub const M20_PROMOTION_CONTEXT_SCHEMA_VERSION: u32 = 3;
pub const M21_PROMOTION_CONTEXT_SCHEMA_VERSION: u32 = 4;
pub const M22_PROMOTION_CONTEXT_SCHEMA_VERSION: u32 = 5;
pub const M23_PROMOTION_CONTEXT_SCHEMA_VERSION: u32 = 6;
pub const PROMOTION_POLICY_SCHEMA_VERSION: u32 = 1;
pub const PROMOTION_EVIDENCE_SCHEMA_VERSION: u32 = 1;
pub const M19_PROMOTION_EVIDENCE_SCHEMA_VERSION: u32 = 2;
pub const M20_PROMOTION_EVIDENCE_SCHEMA_VERSION: u32 = 3;
pub const M21_PROMOTION_EVIDENCE_SCHEMA_VERSION: u32 = 4;
pub const M22_PROMOTION_EVIDENCE_SCHEMA_VERSION: u32 = 5;
pub const M23_PROMOTION_EVIDENCE_SCHEMA_VERSION: u32 = 6;
pub const PROMOTION_DECISION_SCHEMA_VERSION: u32 = 1;
pub const M19_PROMOTION_DECISION_SCHEMA_VERSION: u32 = 2;
pub const M20_PROMOTION_DECISION_SCHEMA_VERSION: u32 = 3;
pub const M21_PROMOTION_DECISION_SCHEMA_VERSION: u32 = 4;
pub const M22_PROMOTION_DECISION_SCHEMA_VERSION: u32 = 5;
pub const M23_PROMOTION_DECISION_SCHEMA_VERSION: u32 = 6;
pub const PROMOTION_HEAD_SCHEMA_VERSION: u32 = 1;
pub const M21_PROMOTION_HEAD_SCHEMA_VERSION: u32 = 2;
pub const M22_PROMOTION_HEAD_SCHEMA_VERSION: u32 = 3;
pub const M23_PROMOTION_HEAD_SCHEMA_VERSION: u32 = 4;
pub const DIGEST_ALGORITHM: &str = "cognigraph-canonical-json-nfc-v1+sha256";
pub const M18_EVALUATOR_ID: &str = "construct.evaluate";
pub const M18_METRIC_SEMANTICS_VERSION: &str = "cognigraph.distinct-fact-set.v1";
pub const M18_SCORER_ID: &str = "cognigraph-construct.evaluate";
pub const M18_SCORER_VERSION: &str = "1";
pub const M18_ORACLE_VERIFIER_NAME: &str = "cognigraph.oracle-separation-manifest";
pub const M18_ORACLE_VERIFIER_VERSION: &str = "1";

const M18_SCORER_ARTIFACT_IDENTITY: &str = "cognigraph-construct.evaluate/distinct-fact-set/v1";
const M18_ORACLE_VERIFIER_ARTIFACT_IDENTITY: &str = "cognigraph.oracle-separation-manifest/v1";

const MAX_IDENTIFIER_BYTES: usize = 256;
const MAX_REASON_BYTES: usize = 1024;
const MAX_URI_BYTES: usize = 2048;
const MAX_PAGE_SIZE: usize = 200;
pub(crate) const MAX_RECONCILE_DECISIONS: usize = 10_000;

const EVIDENCE_FIELDS: &[&str] = &[
    "_key",
    "schema_version",
    "digest_algorithm",
    "id",
    "tenant",
    "tenant_incarnation",
    "target",
    "created_at_ms",
    "created_by",
    "idempotency_key_hash",
    "request_digest",
    "evidence_digest",
    "expected_head_decision_id",
    "rollback_target_evidence_id",
    "candidate_digest",
    "baseline_candidate_digest",
    "policy",
    "policy_digest",
    "governance",
    "artifact_attestations",
    "artifact_consumption",
    "runs",
    "gates",
];

const DECISION_FIELDS: &[&str] = &[
    "_key",
    "schema_version",
    "digest_algorithm",
    "id",
    "tenant",
    "tenant_incarnation",
    "target",
    "action",
    "actor",
    "reason",
    "created_at_ms",
    "idempotency_key_hash",
    "request_digest",
    "decision_digest",
    "evidence_id",
    "evidence_digest",
    "policy_digest",
    "gate_assessment_digest",
    "expected_head_decision_id",
    "predecessor_decision_id",
    "resulting_selection",
    "governance",
];

const HEAD_FIELDS: &[&str] = &[
    "_key",
    "schema_version",
    "digest_algorithm",
    "tenant",
    "tenant_incarnation",
    "target",
    "applied_decision_id",
    "selection",
    "updated_at_ms",
    "projection_digest",
];

#[cfg(test)]
mod tests;

mod identity;
pub use identity::*;

mod policy;
pub use policy::*;

mod reproducibility;
pub use reproducibility::*;

mod oracle;
pub use oracle::*;

mod governance_binding;
pub use governance_binding::*;

mod context;
pub use context::*;

mod evaluation;
pub use evaluation::*;

mod evidence_contracts;
pub use evidence_contracts::*;

mod decision_contracts;
pub use decision_contracts::*;

mod reconciliation_contracts;
pub use reconciliation_contracts::*;

mod assessment;
pub use assessment::*;

mod comparison;
use comparison::*;

mod canonical;
pub use canonical::*;

mod validation;
pub use validation::*;

mod record_identity;
pub use record_identity::*;

mod manager;
pub use manager::*;

mod repository;

mod evidence_registration;

mod evidence_queries;

mod promotion;

mod decision_transition;

mod rollback;

mod decision_queries;

mod reconciliation;

mod recovery;

mod status;

mod snapshot_import;

mod snapshot_validation;

mod evidence_validation;

mod decision_validation;

mod chain_validation;

mod heads;

mod storage;

mod target_history;

mod generation_boundaries;
use generation_boundaries::*;

mod head_validation;
pub(crate) use head_validation::*;
