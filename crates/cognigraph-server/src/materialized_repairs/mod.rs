//! M26 verified Semantic Repair generations and signed atomic deployment.
//!
//! M25 authorizes one exact construction candidate. M26 consumes the
//! selected M22/M23 prepared corpus from the verified local CAS, proves that
//! the resulting semantic fact projection is byte-for-byte the evaluated
//! graph, and freezes the complete bounded occurrence projection. A separate
//! Promoter signature is required before those rows replace one served space.

use std::collections::{BTreeMap, BTreeSet};

use cognigraph_auth::Role;
use cognigraph_construct::{
    Chunk, MaterializationError, MaterializationOptions, MaterializedChunkRow,
    MaterializedEntityRow, MaterializedFactOccurrenceRow, MaterializedGraphProjection,
    MaterializedMentionRow, SpaceType, VetoRule, derive_materialized_graph, ingest_chunks,
};
use cognigraph_core::{
    BatchOp, CogniGraphError, CollectionType, FieldPredicate, IndexDef, IndexType, PredicateOp,
};
use cognigraph_governance::{GovernanceStatement, KeyPurpose, SignatureEnvelope};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::artifact_cas::LocalArtifactCas;
use crate::artifact_consumption::{
    CORPUS_ENTRYPOINT, PreparedChunkCorpusArtifact, VerifiedGraphFact,
};
use crate::governance::{
    GOVERNANCE_KEY_REVOCATIONS_COLLECTION, GOVERNANCE_KEYS_COLLECTION, GovernanceActor,
    GovernanceKeyRecord, GovernanceKeyRevocation, key_revocation_id, validate_historical_key_use,
};
use crate::promotions::{
    DECISIONS_COLLECTION, DIGEST_ALGORITHM, EVIDENCE_COLLECTION, EvidenceRunRole,
    MAX_RECONCILE_DECISIONS, PromotionAction, PromotionDecision, PromotionEvidence, PromotionHead,
    PromotionManager, PromotionTarget, canonical_digest, canonical_json_bytes, digest_bytes,
    new_record_id, now_millis, record_digest, same_stored_record, scoped_key,
    validate_idempotency_key, validate_reason,
};
use crate::semantic_repairs::{
    ResolvedSemanticRepairAuthority, SEMANTIC_REPAIR_REVIEWS_COLLECTION,
    SEMANTIC_REPAIR_REVISIONS_COLLECTION, SemanticRepairReviewDecision, SemanticRepairReviewRecord,
    SemanticRepairRevisionRecord, semantic_repair_review_id, semantic_repair_revision_id,
};

pub const SEMANTIC_REPAIR_GENERATIONS_COLLECTION: &str = "_cognigraph_semantic_repair_generations";
pub const SEMANTIC_REPAIR_DEPLOYMENT_DECISIONS_COLLECTION: &str =
    "_cognigraph_semantic_repair_deployment_decisions";
pub const SEMANTIC_REPAIR_DEPLOYMENT_HEADS_COLLECTION: &str =
    "_cognigraph_semantic_repair_deployment_heads";

pub const SEMANTIC_REPAIR_DEPLOYMENT_INTENT_DOMAIN: &str =
    "cognigraph.semantic-repair-deployment-intent.v1";
pub const MATERIALIZATION_RECORD_SCHEMA_VERSION: u32 = 1;

const M26_PROTECTED_COLLECTIONS: [&str; 3] = [
    SEMANTIC_REPAIR_GENERATIONS_COLLECTION,
    SEMANTIC_REPAIR_DEPLOYMENT_DECISIONS_COLLECTION,
    SEMANTIC_REPAIR_DEPLOYMENT_HEADS_COLLECTION,
];

pub const MAX_M26_CHUNKS: usize = 1_000;
pub const MAX_M26_SEMANTIC_FACTS: usize = 10_000;
pub const MAX_M26_TOTAL_ROWS: usize = 50_000;
pub const MAX_M26_CANONICAL_GENERATION_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_M26_GENERATIONS_PER_TENANT: usize = 64;
pub const MAX_M26_GENERATIONS_PER_SPACE: usize = 16;
pub const MAX_M26_GENERATION_BYTES_PER_TENANT: usize = 256 * 1024 * 1024;
pub const MAX_M26_DEPLOYMENT_DECISIONS_PER_SPACE: usize = 10_000;
pub const MAX_M26_LIST_PAGE_SIZE: usize = 25;

const MAX_M26_ENTITIES: usize = 10_000;
const MAX_M26_MENTIONS: usize = 30_000;
const MAX_M26_FACT_OCCURRENCES: usize = 30_000;
const MAX_M26_DEPLOYMENT_SPACES_PER_TENANT: usize = MAX_M26_GENERATIONS_PER_TENANT;
const MAX_M26_DEPLOYMENT_DECISIONS_PER_TENANT: usize =
    MAX_M26_DEPLOYMENT_SPACES_PER_TENANT * MAX_M26_DEPLOYMENT_DECISIONS_PER_SPACE;
const MAX_CLOCK_SKEW_MS: u64 = 5 * 60 * 1_000;
const MATERIALIZER_ID: &str = "cognigraph.semantic-repair-materializer";
const MATERIALIZER_VERSION: &str = "1";
const MATERIALIZATION_ABI: &str = "cognigraph.complete-target-occurrence-projection.v1";
const MATERIALIZER_SEMANTICS: &str = "verified-canonical-prepared-corpus/exact-m22-m23-semantic-facts/deterministic-entity-chunk-mention-fact-occurrences/trigger-byte-spans/provenance-attribution/sorted-unique-keys/candidate-baseline-set-impact/atomic-target-replacement/v1";

#[cfg(test)]
mod tests;

mod projection;
pub use projection::*;

mod impact;
pub use impact::*;

mod generation_contracts;
pub use generation_contracts::*;

mod deployment_contracts;
pub use deployment_contracts::*;

mod authority_contracts;
pub use authority_contracts::*;

mod record_helpers;
use record_helpers::*;

mod backend_capability;
use backend_capability::*;

mod repository;

mod authority_pinning;

mod generation_build;

mod generation_queries;

mod generation_capacity;

mod generation_validation;

mod generation_size;
use generation_size::*;

mod deployment;

mod deployment_queries;

mod status;

mod recovery;

mod legacy_ingest;

mod snapshot_authority;

mod snapshot_preflight;

mod snapshot_projection;

mod deployment_chains;

mod target_projection;

mod deployment_validation;

mod capacity;
use capacity::*;

mod chain_validation;
use chain_validation::*;

mod snapshot_records;
use snapshot_records::*;

mod intent_validation;
use intent_validation::*;
