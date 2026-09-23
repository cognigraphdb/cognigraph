use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use super::*;
use crate::artifact_attestations::{
    ARTIFACT_ATTESTATION_DOMAIN, ArtifactAttestationBinding, ArtifactAttestationPayload,
    ArtifactAttestationRecord, ArtifactAttestationSet, ArtifactKind, ArtifactLocationObservation,
    ArtifactManifest, ArtifactManifestEntry, ArtifactSubject, CreateArtifactAttestationRequest,
    ResolveArtifactBindingsRequest, artifact_attestation_id,
};
use crate::artifact_cas::{LocalArtifactCas, tenant_scope_hex};
use crate::artifact_consumption::{
    ArtifactConsumptionPlan, CANDIDATE_ENTRYPOINT, CORPUS_ARTIFACT_FORMAT, CORPUS_ENTRYPOINT,
    ConstructionCandidateArtifact, ConstructionEntity, ConstructionRelationRule,
    ConstructionSpaceType, DOCUMENTS_ENTRYPOINT, EXECUTABLE_ARTIFACT_FORMAT, EXECUTABLE_ENTRYPOINT,
    EvaluationGraphArtifact, GRAPH_ARTIFACT_FORMAT, GRAPH_ENTRYPOINT,
    M21_CONSUMPTION_RECEIPT_SCHEMA_VERSION, M22_CONSUMPTION_RECEIPT_SCHEMA_VERSION,
    M22_CORPUS_ARTIFACT_FORMAT, M22_GRAPH_ARTIFACT_FORMAT, M23_CONSUMPTION_RECEIPT_SCHEMA_VERSION,
    M23_CORPUS_ARTIFACT_FORMAT, M23_DERIVATION_RECEIPT_SCHEMA_VERSION,
    M23_PREPARATION_RECEIPT_SCHEMA_VERSION, ORACLE_ARTIFACT_FORMAT, ORACLE_ENTRYPOINT,
    PreparedChunkArtifact, PreparedChunkCorpusArtifact, PromotionOracleArtifact,
    RawDocumentArtifact, RawDocumentSetArtifact, ReproducibleEvaluationGraphArtifact,
    ResolvedConstructionConfig, VerifiedGraphFact, current_executable_digest,
};
use crate::governance::{
    ApprovePolicyRevisionRequest, CreatePolicyRevisionRequest, GOVERNANCE_KEYS_COLLECTION,
    GOVERNANCE_RECORD_SCHEMA_VERSION, GovernanceActor, GovernanceKeyRecord,
    KEY_REGISTRATION_DOMAIN, KEY_REVOCATION_DOMAIN, KeyRegistrationPayload, KeyRevocationPayload,
    M20_KEY_REGISTRATION_DOMAIN, M21_PROMOTION_INTENT_DOMAIN, M22_PROMOTION_INTENT_DOMAIN,
    M23_PROMOTION_INTENT_DOMAIN, POLICY_APPROVAL_DOMAIN, POLICY_REVISION_DOMAIN,
    PROMOTION_INTENT_DOMAIN, PolicyApprovalPayload, PolicyRevisionPayload, PromotionIntentPayload,
    PromotionIntentSubmission, RegisterGovernanceKeyRequest, RevokeGovernanceKeyRequest,
    approval_id, key_registration_id, key_revocation_id, policy_revision_id,
};
use crate::jobs::{JobActor, JobKind, JobRecord, JobStatus};
use crate::materialized_repairs::{
    BuildSemanticRepairGenerationRequest, SEMANTIC_REPAIR_DEPLOYMENT_DECISIONS_COLLECTION,
    SEMANTIC_REPAIR_DEPLOYMENT_HEADS_COLLECTION, SEMANTIC_REPAIR_DEPLOYMENT_INTENT_DOMAIN,
    SEMANTIC_REPAIR_GENERATIONS_COLLECTION, SemanticRepairDeploymentAction,
    SemanticRepairDeploymentIntentPayload, SemanticRepairDeploymentIntentSubmission,
    SemanticRepairGenerationRecord,
};
use crate::semantic_repairs::{
    CreateSemanticRepairRevisionRequest, ReviewSemanticRepairRevisionRequest,
    SEMANTIC_REPAIR_REVIEW_DOMAIN, SEMANTIC_REPAIR_REVIEWS_COLLECTION,
    SEMANTIC_REPAIR_REVISION_DOMAIN, SEMANTIC_REPAIR_REVISIONS_COLLECTION,
    SemanticRepairReviewDecision, SemanticRepairReviewPayload, SemanticRepairReviewRecord,
    SemanticRepairRevisionPayload, SemanticRepairRevisionRecord, semantic_repair_review_id,
    semantic_repair_revision_id,
};
use crate::state::AppState;
use async_trait::async_trait;
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use cognigraph_auth::{AuthProvider, Role, User};
use cognigraph_construct::{Chunk, EvalQuestion, EvalSpec, effective_config, effective_vetoes};
use cognigraph_core::{
    BatchOp, CollectionInfo, CollectionType, Direction, DocumentId, GraphBackend,
    Result as BackendResult, SearchHit, TraversalOpts, TraversalPath, VectorSearchOpts,
};
use cognigraph_governance::{GovernanceStatement, KeyPurpose, SigningKeyMaterial};
use cognigraph_native::NativeBackend;
use serde_json::{Map, json};

const TENANT: &str = "default";
const INCARNATION: &str = "default";
const SPACE: &str = "pharma";

/// Runs a whole-server lifecycle flow on a thread with an explicit stack.
///
/// These flows drive every promotion stage inside one future. `#[tokio::test]`
/// would pin that future on the default 2 MiB test thread, and in debug
/// builds the nested state machine plus poll frames exceed it on x86_64
/// (CG-96); release builds need under 512 KiB. The runtime matches
/// `#[tokio::test]`: current-thread, all drivers enabled.
pub(super) fn lifecycle<F>(flow: impl FnOnce() -> F + Send + 'static)
where
    F: std::future::Future<Output = ()>,
{
    std::thread::Builder::new()
        .name("promotion-lifecycle".into())
        .stack_size(16 << 20)
        .spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("current-thread runtime")
                .block_on(flow())
        })
        .expect("lifecycle thread")
        .join()
        .expect("lifecycle flow panicked");
}

mod backend;
use backend::*;

mod evaluation_fixtures;
use evaluation_fixtures::*;

mod context_fixtures;
use context_fixtures::*;

mod key_fixtures;
use key_fixtures::*;

mod policy_fixtures;
use policy_fixtures::*;

mod intent_fixtures;
use intent_fixtures::*;

mod job_fixtures;
use job_fixtures::*;

mod repair_fixtures;
use repair_fixtures::*;

mod deployment_fixtures;
use deployment_fixtures::*;

mod attestation_fixtures;
use attestation_fixtures::*;

mod cas_fixtures;
use cas_fixtures::*;

mod consumption_fixtures;
use consumption_fixtures::*;

mod derivation_fixtures;
use derivation_fixtures::*;

mod derivation_variant;
use derivation_variant::*;

mod materialization_fixtures;
use materialization_fixtures::*;

mod stored_attestation;
use stored_attestation::*;

mod contract_checks;

mod gate_checks;

mod artifact_lifecycle;

mod raw_document_lifecycle;

mod derivation_lifecycle;

mod consumption_lifecycle;

mod legacy_materialization;

mod raw_materialization;

mod materialization_lifecycle;

mod repair_lifecycle;

mod governed_lifecycle;

mod durable_lifecycle;
