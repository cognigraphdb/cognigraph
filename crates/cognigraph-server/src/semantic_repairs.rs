//! M25 immutable, signed Semantic Repair authority.
//!
//! A repair revision binds the exact canonical M22 `candidate.json` value to
//! the promotion head it was authored against. A separate policy approver may
//! then approve or reject that immutable revision. Resolution is deliberately
//! fail closed: the current promotion selection is usable only when exactly
//! one independently approved revision is compatible with its decision
//! predecessor (or with the earlier promotion selected by a rollback).

use cognigraph_auth::Role;
use cognigraph_core::{CogniGraphError, CollectionType};
use cognigraph_governance::{GovernanceStatement, KeyPurpose, SignatureEnvelope};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use unicode_normalization::is_nfc;

use crate::artifact_consumption::ConstructionCandidateArtifact;
use crate::governance::{
    GovernanceActor, GovernanceKeyRecord, GovernanceMutation, validate_historical_key_use,
};
use crate::promotions::{
    DIGEST_ALGORITHM, EvidenceRunRole, PromotionAction, PromotionDecision, PromotionHead,
    PromotionManager, PromotionPage, PromotionTarget, canonical_digest, canonical_json_bytes,
    digest_bytes, now_millis, record_digest, scoped_key, validate_idempotency_key, validate_reason,
};

pub const SEMANTIC_REPAIR_REVISIONS_COLLECTION: &str = "_cognigraph_semantic_repair_revisions";
pub const SEMANTIC_REPAIR_REVIEWS_COLLECTION: &str = "_cognigraph_semantic_repair_reviews";

pub const SEMANTIC_REPAIR_REVISION_DOMAIN: &str = "cognigraph.semantic-repair-revision.v1";
pub const SEMANTIC_REPAIR_REVIEW_DOMAIN: &str = "cognigraph.semantic-repair-review.v1";

pub const SEMANTIC_REPAIR_RECORD_SCHEMA_VERSION: u32 = 1;
pub(crate) const MAX_SEMANTIC_REPAIR_REVISIONS_PER_TENANT: usize = 10_000;
pub(crate) const MAX_SEMANTIC_REPAIR_REVIEWS_PER_TENANT: usize = 10_000;
pub(crate) const MAX_SEMANTIC_REPAIR_CANDIDATE_BYTES_PER_TENANT: usize = 64 * 1024 * 1024;
pub(crate) const MAX_SEMANTIC_REPAIR_REVISION_PAGE_SIZE: usize = 8;
const MAX_IDENTIFIER_BYTES: usize = 256;
const MAX_PAGE_SIZE: usize = 200;
const MAX_CLOCK_SKEW_MS: u64 = 5 * 60 * 1_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticRepairRevisionPayload {
    pub semantic_repair_revision_id: String,
    pub target: PromotionTarget,
    #[serde(deserialize_with = "deserialize_required_option")]
    pub base_promotion_head_decision_id: Option<String>,
    pub candidate: ConstructionCandidateArtifact,
    pub candidate_digest: String,
    pub author_registration_id: String,
    pub author_principal_id: String,
    pub signed_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateSemanticRepairRevisionRequest {
    pub statement: GovernanceStatement<SemanticRepairRevisionPayload>,
    pub author_signature: SignatureEnvelope,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticRepairRevisionRecord {
    #[serde(rename = "_key")]
    pub key: String,
    pub schema_version: u32,
    pub digest_algorithm: String,
    pub tenant: String,
    pub tenant_incarnation: String,
    pub semantic_repair_revision_id: String,
    pub target: PromotionTarget,
    #[serde(deserialize_with = "deserialize_required_option")]
    pub base_promotion_head_decision_id: Option<String>,
    pub candidate: ConstructionCandidateArtifact,
    pub candidate_digest: String,
    pub author_registration_id: String,
    pub author_registration_digest: String,
    pub author_principal_id: String,
    pub signed_at_ms: u64,
    pub author_signature: SignatureEnvelope,
    pub created_by: GovernanceActor,
    pub created_at_ms: u64,
    pub idempotency_key_hash: String,
    pub request_digest: String,
    pub semantic_repair_revision_digest: String,
}

impl SemanticRepairRevisionRecord {
    pub fn public_value(&self) -> Result<Value, CogniGraphError> {
        public_value(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticRepairReviewDecision {
    Approve,
    Reject,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticRepairReviewPayload {
    pub semantic_repair_review_id: String,
    pub semantic_repair_revision_id: String,
    pub semantic_repair_revision_digest: String,
    pub target: PromotionTarget,
    #[serde(deserialize_with = "deserialize_required_option")]
    pub base_promotion_head_decision_id: Option<String>,
    pub candidate_digest: String,
    pub author_principal_id: String,
    pub approver_registration_id: String,
    pub approver_principal_id: String,
    pub decision: SemanticRepairReviewDecision,
    pub reason: String,
    pub signed_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewSemanticRepairRevisionRequest {
    pub statement: GovernanceStatement<SemanticRepairReviewPayload>,
    pub approver_signature: SignatureEnvelope,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticRepairReviewRecord {
    #[serde(rename = "_key")]
    pub key: String,
    pub schema_version: u32,
    pub digest_algorithm: String,
    pub tenant: String,
    pub tenant_incarnation: String,
    pub semantic_repair_review_id: String,
    pub semantic_repair_revision_id: String,
    pub semantic_repair_revision_digest: String,
    pub target: PromotionTarget,
    #[serde(deserialize_with = "deserialize_required_option")]
    pub base_promotion_head_decision_id: Option<String>,
    pub candidate_digest: String,
    pub author_principal_id: String,
    pub approver_registration_id: String,
    pub approver_registration_digest: String,
    pub approver_principal_id: String,
    pub decision: SemanticRepairReviewDecision,
    pub reason: String,
    pub signed_at_ms: u64,
    pub approver_signature: SignatureEnvelope,
    pub reviewed_by: GovernanceActor,
    pub reviewed_at_ms: u64,
    pub idempotency_key_hash: String,
    pub request_digest: String,
    pub semantic_repair_review_digest: String,
}

impl SemanticRepairReviewRecord {
    pub fn public_value(&self) -> Result<Value, CogniGraphError> {
        public_value(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResolvedSemanticRepairAuthority {
    pub revision: SemanticRepairRevisionRecord,
    pub review: SemanticRepairReviewRecord,
}

struct CompatibleSemanticRepairBase {
    base_decision_id: Option<String>,
    promoter_principal_id: String,
}

impl ResolvedSemanticRepairAuthority {
    pub fn public_value(&self) -> Result<Value, CogniGraphError> {
        Ok(json!({
            "revision": self.revision.public_value()?,
            "review": self.review.public_value()?,
        }))
    }
}

impl PromotionManager {
    pub(crate) async fn ensure_semantic_repair_repository(
        &self,
        tenant: &str,
    ) -> Result<(), CogniGraphError> {
        for collection in [
            SEMANTIC_REPAIR_REVISIONS_COLLECTION,
            SEMANTIC_REPAIR_REVIEWS_COLLECTION,
        ] {
            if let Err(error) = self
                .backend
                .ensure_collection(collection, CollectionType::Document)
                .await
            {
                self.record_error(
                    tenant,
                    format!("semantic repair repository initialization failed: {error}"),
                );
                return Err(error);
            }
        }
        Ok(())
    }

    pub async fn create_semantic_repair_revision(
        &self,
        tenant: &str,
        incarnation: &str,
        actor: GovernanceActor,
        idempotency_key: &str,
        request: CreateSemanticRepairRevisionRequest,
    ) -> Result<GovernanceMutation<SemanticRepairRevisionRecord>, CogniGraphError> {
        actor.require_role(Role::PolicyAuthor)?;
        validate_idempotency_key(idempotency_key)?;
        self.ensure_repository(tenant).await?;
        self.ensure_governance_repository(tenant).await?;
        self.ensure_semantic_repair_repository(tenant).await?;
        let _guard = self.transition_lock.lock().await;
        self.ensure_tenant_active(tenant)?;
        self.ensure_mutations_healthy(tenant)?;

        let now = now_millis();
        validate_statement_scope(
            &request.statement,
            SEMANTIC_REPAIR_REVISION_DOMAIN,
            tenant,
            incarnation,
        )?;
        let payload = &request.statement.payload;
        validate_revision_payload(payload, tenant, incarnation, now)?;
        let key_hash = digest_bytes(idempotency_key.as_bytes());
        let request_digest = canonical_digest(&request)?;
        let key = scoped_key(
            tenant,
            incarnation,
            "srr",
            &payload.semantic_repair_revision_id,
        );
        if let Some(existing) = self
            .get_authority_raw::<SemanticRepairRevisionRecord>(
                tenant,
                SEMANTIC_REPAIR_REVISIONS_COLLECTION,
                &key,
            )
            .await?
        {
            self.validate_stored_semantic_repair_revision(&existing, tenant, incarnation)
                .await?;
            if existing.idempotency_key_hash == key_hash
                && existing.request_digest == request_digest
                && existing.created_by == actor
            {
                return Ok(GovernanceMutation {
                    record: existing,
                    replayed: true,
                });
            }
            return Err(conflict(
                "semantic repair revision identity already has different authority",
            ));
        }

        let author = self
            .get_active_key_locked(
                tenant,
                incarnation,
                &payload.author_registration_id,
                KeyPurpose::PolicyAuthor,
                now,
            )
            .await?;
        require_key_actor(&author, &actor, &payload.author_principal_id)?;
        author
            .verification_key
            .verify(
                &request.statement,
                &request.author_signature,
                SEMANTIC_REPAIR_REVISION_DOMAIN,
                tenant,
                incarnation,
                KeyPurpose::PolicyAuthor,
            )
            .map_err(signature_error)?;
        self.ensure_semantic_repair_idempotency_available_locked(tenant, incarnation, &key_hash)
            .await?;

        self.reconcile_locked(tenant, incarnation, &payload.target, false)
            .await?;
        let observed_base = self
            .current_raw(tenant, incarnation, &payload.target)
            .await?
            .map(|head| head.applied_decision_id);
        if observed_base != payload.base_promotion_head_decision_id {
            return Err(conflict(
                "semantic repair revision base does not match the current promotion head",
            ));
        }

        let (revision_count, existing_candidate_bytes) = self
            .semantic_repair_revision_capacity_locked(tenant, incarnation)
            .await?;
        if revision_count >= MAX_SEMANTIC_REPAIR_REVISIONS_PER_TENANT {
            return Err(conflict(format!(
                "tenant semantic repair revision limit of {MAX_SEMANTIC_REPAIR_REVISIONS_PER_TENANT} is exhausted"
            )));
        }
        ensure_semantic_repair_candidate_capacity([
            Ok(existing_candidate_bytes),
            canonical_json_bytes(&payload.candidate).map(|bytes| bytes.len()),
        ])?;

        let mut record = SemanticRepairRevisionRecord {
            key,
            schema_version: SEMANTIC_REPAIR_RECORD_SCHEMA_VERSION,
            digest_algorithm: DIGEST_ALGORITHM.into(),
            tenant: tenant.into(),
            tenant_incarnation: incarnation.into(),
            semantic_repair_revision_id: payload.semantic_repair_revision_id.clone(),
            target: payload.target.clone(),
            base_promotion_head_decision_id: payload.base_promotion_head_decision_id.clone(),
            candidate: payload.candidate.clone(),
            candidate_digest: payload.candidate_digest.clone(),
            author_registration_id: author.registration_id.clone(),
            author_registration_digest: author.registration_digest.clone(),
            author_principal_id: author.principal_id.clone(),
            signed_at_ms: payload.signed_at_ms,
            author_signature: request.author_signature,
            created_by: actor,
            created_at_ms: now,
            idempotency_key_hash: key_hash,
            request_digest,
            semantic_repair_revision_digest: String::new(),
        };
        record.semantic_repair_revision_digest =
            record_digest(&record, "semantic_repair_revision_digest")?;
        self.validate_semantic_repair_revision_record(&record, tenant, incarnation)
            .await?;
        self.insert_immutable(
            tenant,
            SEMANTIC_REPAIR_REVISIONS_COLLECTION,
            &record.key,
            &record,
        )
        .await?;
        Ok(GovernanceMutation {
            record,
            replayed: false,
        })
    }

    pub async fn review_semantic_repair_revision(
        &self,
        tenant: &str,
        incarnation: &str,
        actor: GovernanceActor,
        idempotency_key: &str,
        request: ReviewSemanticRepairRevisionRequest,
    ) -> Result<GovernanceMutation<SemanticRepairReviewRecord>, CogniGraphError> {
        actor.require_role(Role::PolicyApprover)?;
        validate_idempotency_key(idempotency_key)?;
        self.ensure_governance_repository(tenant).await?;
        self.ensure_semantic_repair_repository(tenant).await?;
        let _guard = self.transition_lock.lock().await;
        self.ensure_tenant_active(tenant)?;
        self.ensure_mutations_healthy(tenant)?;

        let now = now_millis();
        validate_statement_scope(
            &request.statement,
            SEMANTIC_REPAIR_REVIEW_DOMAIN,
            tenant,
            incarnation,
        )?;
        let payload = &request.statement.payload;
        validate_review_payload(payload, tenant, incarnation, now)?;
        let revision = self
            .get_semantic_repair_revision_locked(
                tenant,
                incarnation,
                &payload.semantic_repair_revision_id,
            )
            .await?;
        if payload.semantic_repair_revision_digest != revision.semantic_repair_revision_digest
            || payload.target != revision.target
            || payload.base_promotion_head_decision_id != revision.base_promotion_head_decision_id
            || payload.candidate_digest != revision.candidate_digest
            || payload.author_principal_id != revision.author_principal_id
        {
            return Err(validation(
                "semantic repair review does not bind the exact revision",
            ));
        }

        let key_hash = digest_bytes(idempotency_key.as_bytes());
        let request_digest = canonical_digest(&request)?;
        let key = scoped_key(
            tenant,
            incarnation,
            "srrv",
            &payload.semantic_repair_review_id,
        );
        if let Some(existing) = self
            .get_authority_raw::<SemanticRepairReviewRecord>(
                tenant,
                SEMANTIC_REPAIR_REVIEWS_COLLECTION,
                &key,
            )
            .await?
        {
            self.validate_stored_semantic_repair_review(&existing, tenant, incarnation)
                .await?;
            if existing.idempotency_key_hash == key_hash
                && existing.request_digest == request_digest
                && existing.reviewed_by == actor
            {
                return Ok(GovernanceMutation {
                    record: existing,
                    replayed: true,
                });
            }
            return Err(conflict(
                "semantic repair revision already has a different review authority",
            ));
        }

        let author = self
            .get_active_key_locked(
                tenant,
                incarnation,
                &revision.author_registration_id,
                KeyPurpose::PolicyAuthor,
                now,
            )
            .await?;
        if author.registration_digest != revision.author_registration_digest
            || author.principal_id != revision.author_principal_id
        {
            return Err(conflict(
                "semantic repair review references different author authority",
            ));
        }
        let approver = self
            .get_active_key_locked(
                tenant,
                incarnation,
                &payload.approver_registration_id,
                KeyPurpose::PolicyApprover,
                now,
            )
            .await?;
        require_key_actor(&approver, &actor, &payload.approver_principal_id)?;
        if approver.principal_id == revision.author_principal_id {
            return Err(CogniGraphError::Forbidden(
                "semantic repair author and reviewer principals must be distinct".into(),
            ));
        }
        approver
            .verification_key
            .verify(
                &request.statement,
                &request.approver_signature,
                SEMANTIC_REPAIR_REVIEW_DOMAIN,
                tenant,
                incarnation,
                KeyPurpose::PolicyApprover,
            )
            .map_err(signature_error)?;
        self.ensure_semantic_repair_idempotency_available_locked(tenant, incarnation, &key_hash)
            .await?;

        let review_count = self
            .semantic_repair_record_count_locked(
                SEMANTIC_REPAIR_REVIEWS_COLLECTION,
                tenant,
                incarnation,
                "srrv",
                MAX_SEMANTIC_REPAIR_REVIEWS_PER_TENANT,
            )
            .await?;
        if review_count >= MAX_SEMANTIC_REPAIR_REVIEWS_PER_TENANT {
            return Err(conflict(format!(
                "tenant semantic repair review limit of {MAX_SEMANTIC_REPAIR_REVIEWS_PER_TENANT} is exhausted"
            )));
        }

        let mut record = SemanticRepairReviewRecord {
            key,
            schema_version: SEMANTIC_REPAIR_RECORD_SCHEMA_VERSION,
            digest_algorithm: DIGEST_ALGORITHM.into(),
            tenant: tenant.into(),
            tenant_incarnation: incarnation.into(),
            semantic_repair_review_id: payload.semantic_repair_review_id.clone(),
            semantic_repair_revision_id: revision.semantic_repair_revision_id.clone(),
            semantic_repair_revision_digest: revision.semantic_repair_revision_digest.clone(),
            target: revision.target.clone(),
            base_promotion_head_decision_id: revision.base_promotion_head_decision_id.clone(),
            candidate_digest: revision.candidate_digest.clone(),
            author_principal_id: revision.author_principal_id.clone(),
            approver_registration_id: approver.registration_id.clone(),
            approver_registration_digest: approver.registration_digest.clone(),
            approver_principal_id: approver.principal_id.clone(),
            decision: payload.decision,
            reason: payload.reason.clone(),
            signed_at_ms: payload.signed_at_ms,
            approver_signature: request.approver_signature,
            reviewed_by: actor,
            reviewed_at_ms: now,
            idempotency_key_hash: key_hash,
            request_digest,
            semantic_repair_review_digest: String::new(),
        };
        record.semantic_repair_review_digest =
            record_digest(&record, "semantic_repair_review_digest")?;
        self.validate_semantic_repair_review_record(&record, tenant, incarnation)
            .await?;
        self.insert_immutable(
            tenant,
            SEMANTIC_REPAIR_REVIEWS_COLLECTION,
            &record.key,
            &record,
        )
        .await?;
        Ok(GovernanceMutation {
            record,
            replayed: false,
        })
    }

    pub async fn list_semantic_repair_revisions(
        &self,
        tenant: &str,
        incarnation: &str,
        limit: usize,
        cursor: Option<&str>,
    ) -> Result<PromotionPage<SemanticRepairRevisionRecord>, CogniGraphError> {
        validate_page_size(limit, MAX_SEMANTIC_REPAIR_REVISION_PAGE_SIZE, "revision")?;
        self.ensure_semantic_repair_repository(tenant).await?;
        let page = self
            .list_governance_records(
                SEMANTIC_REPAIR_REVISIONS_COLLECTION,
                tenant,
                incarnation,
                "srr",
                limit,
                cursor,
            )
            .await?;
        for record in &page.records {
            self.validate_stored_semantic_repair_revision(record, tenant, incarnation)
                .await?;
        }
        Ok(page)
    }

    pub async fn get_semantic_repair_revision(
        &self,
        tenant: &str,
        incarnation: &str,
        id: &str,
    ) -> Result<SemanticRepairRevisionRecord, CogniGraphError> {
        self.ensure_semantic_repair_repository(tenant).await?;
        self.get_semantic_repair_revision_locked(tenant, incarnation, id)
            .await
    }

    pub async fn list_semantic_repair_reviews(
        &self,
        tenant: &str,
        incarnation: &str,
        limit: usize,
        cursor: Option<&str>,
    ) -> Result<PromotionPage<SemanticRepairReviewRecord>, CogniGraphError> {
        validate_page_size(limit, MAX_PAGE_SIZE, "review")?;
        self.ensure_semantic_repair_repository(tenant).await?;
        let page = self
            .list_governance_records(
                SEMANTIC_REPAIR_REVIEWS_COLLECTION,
                tenant,
                incarnation,
                "srrv",
                limit,
                cursor,
            )
            .await?;
        for record in &page.records {
            self.validate_stored_semantic_repair_review(record, tenant, incarnation)
                .await?;
        }
        Ok(page)
    }

    pub async fn get_semantic_repair_review(
        &self,
        tenant: &str,
        incarnation: &str,
        id: &str,
    ) -> Result<SemanticRepairReviewRecord, CogniGraphError> {
        self.ensure_semantic_repair_repository(tenant).await?;
        self.get_semantic_repair_review_locked(tenant, incarnation, id)
            .await
    }

    /// Resolve authority for a caller-supplied promotion head. The head must
    /// still be current when the shared transition lock is acquired.
    #[allow(dead_code)] // Explicit-head API is retained for non-HTTP in-crate consumers.
    pub async fn resolve_semantic_repair_authority(
        &self,
        tenant: &str,
        incarnation: &str,
        head: &PromotionHead,
        candidate_digest: &str,
    ) -> Result<ResolvedSemanticRepairAuthority, CogniGraphError> {
        validate_digest("candidate_digest", candidate_digest)?;
        self.ensure_repository(tenant).await?;
        self.ensure_governance_repository(tenant).await?;
        self.ensure_semantic_repair_repository(tenant).await?;
        let _guard = self.transition_lock.lock().await;
        self.ensure_tenant_active(tenant)?;
        self.ensure_mutations_healthy(tenant)?;
        self.reconcile_locked(tenant, incarnation, &head.target, false)
            .await?;
        let current = self
            .current_raw(tenant, incarnation, &head.target)
            .await?
            .ok_or_else(|| conflict("semantic repair resolution requires a promotion head"))?;
        if current != *head {
            return Err(conflict(
                "promotion head changed before semantic repair authority resolution",
            ));
        }
        self.resolve_semantic_repair_authority_locked(tenant, incarnation, head, candidate_digest)
            .await
    }

    /// Resolve the current target head and its exact selected candidate.
    pub async fn resolve_current_semantic_repair_authority(
        &self,
        tenant: &str,
        incarnation: &str,
        target: &PromotionTarget,
    ) -> Result<ResolvedSemanticRepairAuthority, CogniGraphError> {
        target.validate()?;
        self.ensure_repository(tenant).await?;
        self.ensure_governance_repository(tenant).await?;
        self.ensure_semantic_repair_repository(tenant).await?;
        let _guard = self.transition_lock.lock().await;
        self.ensure_tenant_active(tenant)?;
        self.resolve_current_semantic_repair_authority_locked(tenant, incarnation, target)
            .await
    }

    /// Resolve while a governed-ingest caller holds `transition_lock`, so the
    /// authority cannot change between selection and atomic graph mutation.
    pub(crate) async fn resolve_current_semantic_repair_authority_locked(
        &self,
        tenant: &str,
        incarnation: &str,
        target: &PromotionTarget,
    ) -> Result<ResolvedSemanticRepairAuthority, CogniGraphError> {
        target.validate()?;
        self.ensure_tenant_active(tenant)?;
        self.ensure_mutations_healthy(tenant)?;
        self.reconcile_locked(tenant, incarnation, target, false)
            .await?;
        let head = self
            .current_raw(tenant, incarnation, target)
            .await?
            .ok_or_else(|| conflict("semantic repair resolution requires a promotion head"))?;
        let candidate_digest = head.selection.candidate_digest.clone();
        self.resolve_semantic_repair_authority_locked(tenant, incarnation, &head, &candidate_digest)
            .await
    }

    pub(crate) async fn resolve_semantic_repair_authority_locked(
        &self,
        tenant: &str,
        incarnation: &str,
        head: &PromotionHead,
        candidate_digest: &str,
    ) -> Result<ResolvedSemanticRepairAuthority, CogniGraphError> {
        if head.tenant != tenant
            || head.tenant_incarnation != incarnation
            || head.selection.candidate_digest != candidate_digest
        {
            return Err(conflict(
                "semantic repair resolver received a foreign head or candidate digest",
            ));
        }
        let applied = self
            .get_decision(tenant, incarnation, &head.applied_decision_id)
            .await?;
        if applied.target != head.target
            || applied.id != head.applied_decision_id
            || applied.resulting_selection.as_ref() != Some(&head.selection)
        {
            return Err(conflict(
                "promotion head does not match its authoritative applied decision",
            ));
        }
        let applied_promoter_principal_id = promotion_principal_id(&applied)?.to_string();
        let selected_evidence = self
            .get_evidence(tenant, incarnation, &applied.evidence_id)
            .await?;
        let selected_candidate = selected_evidence
            .runs
            .iter()
            .find(|run| run.role == EvidenceRunRole::CandidateOriginal)
            .ok_or_else(|| conflict("promotion evidence has no candidate-original identity"))?
            .source
            .context
            .candidate
            .clone();
        if selected_evidence.target != head.target
            || selected_evidence.candidate_digest != candidate_digest
            || selected_candidate.candidate_digest != candidate_digest
        {
            return Err(conflict(
                "promotion head candidate does not match its authoritative evidence identity",
            ));
        }

        let compatible_bases = match applied.action {
            PromotionAction::Promote => vec![CompatibleSemanticRepairBase {
                base_decision_id: applied.predecessor_decision_id.clone(),
                promoter_principal_id: applied_promoter_principal_id.clone(),
            }],
            PromotionAction::Rollback => {
                self.rollback_compatible_bases(tenant, incarnation, head)
                    .await?
            }
            PromotionAction::Reject | PromotionAction::Blocked => Vec::new(),
        };
        if compatible_bases.is_empty() {
            return Err(conflict(
                "promotion selection has no compatible semantic repair base head",
            ));
        }

        let mut approved = Vec::new();
        for compatible in compatible_bases {
            let revision_id = semantic_repair_revision_id(
                tenant,
                incarnation,
                &head.target,
                compatible.base_decision_id.as_deref(),
                candidate_digest,
            )?;
            let revision_key = scoped_key(tenant, incarnation, "srr", &revision_id);
            let Some(revision) = self
                .get_authority_raw::<SemanticRepairRevisionRecord>(
                    tenant,
                    SEMANTIC_REPAIR_REVISIONS_COLLECTION,
                    &revision_key,
                )
                .await?
            else {
                continue;
            };
            self.validate_stored_semantic_repair_revision(&revision, tenant, incarnation)
                .await?;
            if revision.target != head.target
                || revision.candidate_digest != candidate_digest
                || revision.candidate.kind != selected_candidate.kind
                || revision.candidate.id != selected_candidate.id
                || revision.candidate.revision != selected_candidate.revision
                || revision.base_promotion_head_decision_id != compatible.base_decision_id
            {
                return Err(conflict(
                    "semantic repair natural identity resolved to incompatible authority",
                ));
            }
            let review_id = semantic_repair_review_id(
                tenant,
                incarnation,
                &revision.semantic_repair_revision_id,
            );
            let review_key = scoped_key(tenant, incarnation, "srrv", &review_id);
            let Some(review) = self
                .get_authority_raw::<SemanticRepairReviewRecord>(
                    tenant,
                    SEMANTIC_REPAIR_REVIEWS_COLLECTION,
                    &review_key,
                )
                .await?
            else {
                continue;
            };
            self.validate_stored_semantic_repair_review(&review, tenant, incarnation)
                .await?;
            if review.decision != SemanticRepairReviewDecision::Approve {
                continue;
            }
            validate_resolution_separation(
                Some(&applied_promoter_principal_id),
                &revision.author_principal_id,
                &review.approver_principal_id,
            )?;
            validate_resolution_separation(
                Some(&compatible.promoter_principal_id),
                &revision.author_principal_id,
                &review.approver_principal_id,
            )?;
            approved.push(ResolvedSemanticRepairAuthority { revision, review });
        }
        if approved.len() != 1 {
            return Err(conflict(format!(
                "semantic repair resolution requires exactly one approved compatible revision, found {}",
                approved.len()
            )));
        }
        Ok(approved.remove(0))
    }

    async fn rollback_compatible_bases(
        &self,
        tenant: &str,
        incarnation: &str,
        head: &PromotionHead,
    ) -> Result<Vec<CompatibleSemanticRepairBase>, CogniGraphError> {
        let mut cursor = None;
        let mut bases = Vec::new();
        loop {
            let page = self
                .list_decisions(tenant, incarnation, MAX_PAGE_SIZE, cursor.as_deref())
                .await?;
            for decision in page.records {
                if decision.action == PromotionAction::Promote
                    && decision.target == head.target
                    && decision
                        .resulting_selection
                        .as_ref()
                        .is_some_and(|selection| {
                            compatible_historical_selection(selection, &head.selection)
                        })
                {
                    let promoter_principal_id = promotion_principal_id(&decision)?.to_string();
                    bases.push(CompatibleSemanticRepairBase {
                        base_decision_id: decision.predecessor_decision_id,
                        promoter_principal_id,
                    });
                }
            }
            let Some(next) = page.next_cursor else {
                break;
            };
            cursor = Some(next);
        }
        bases.sort_by(|left, right| {
            (&left.base_decision_id, &left.promoter_principal_id)
                .cmp(&(&right.base_decision_id, &right.promoter_principal_id))
        });
        bases.dedup_by(|left, right| {
            left.base_decision_id == right.base_decision_id
                && left.promoter_principal_id == right.promoter_principal_id
        });
        Ok(bases)
    }

    pub(crate) async fn get_semantic_repair_revision_locked(
        &self,
        tenant: &str,
        incarnation: &str,
        id: &str,
    ) -> Result<SemanticRepairRevisionRecord, CogniGraphError> {
        validate_record_id("semantic_repair_revision_id", id)?;
        let key = scoped_key(tenant, incarnation, "srr", id);
        let record = self
            .get_authority_raw::<SemanticRepairRevisionRecord>(
                tenant,
                SEMANTIC_REPAIR_REVISIONS_COLLECTION,
                &key,
            )
            .await?
            .ok_or_else(|| not_found("semantic repair revision", id))?;
        self.validate_stored_semantic_repair_revision(&record, tenant, incarnation)
            .await?;
        Ok(record)
    }

    pub(crate) async fn get_semantic_repair_review_locked(
        &self,
        tenant: &str,
        incarnation: &str,
        id: &str,
    ) -> Result<SemanticRepairReviewRecord, CogniGraphError> {
        validate_record_id("semantic_repair_review_id", id)?;
        let key = scoped_key(tenant, incarnation, "srrv", id);
        let record = self
            .get_authority_raw::<SemanticRepairReviewRecord>(
                tenant,
                SEMANTIC_REPAIR_REVIEWS_COLLECTION,
                &key,
            )
            .await?
            .ok_or_else(|| not_found("semantic repair review", id))?;
        self.validate_stored_semantic_repair_review(&record, tenant, incarnation)
            .await?;
        Ok(record)
    }

    async fn validate_semantic_repair_revision_record(
        &self,
        record: &SemanticRepairRevisionRecord,
        tenant: &str,
        incarnation: &str,
    ) -> Result<(), CogniGraphError> {
        let author = self
            .get_key_locked(tenant, incarnation, &record.author_registration_id)
            .await?;
        self.validate_semantic_repair_revision_record_against(record, &author, tenant, incarnation)
    }

    async fn validate_stored_semantic_repair_revision(
        &self,
        record: &SemanticRepairRevisionRecord,
        tenant: &str,
        incarnation: &str,
    ) -> Result<(), CogniGraphError> {
        self.validate_semantic_repair_revision_record(record, tenant, incarnation)
            .await
            .inspect_err(|error| self.record_error(tenant, error.to_string()))
    }

    pub(crate) fn validate_semantic_repair_revision_record_against(
        &self,
        record: &SemanticRepairRevisionRecord,
        author: &GovernanceKeyRecord,
        tenant: &str,
        incarnation: &str,
    ) -> Result<(), CogniGraphError> {
        if record.schema_version != SEMANTIC_REPAIR_RECORD_SCHEMA_VERSION
            || record.digest_algorithm != DIGEST_ALGORITHM
            || record.tenant != tenant
            || record.tenant_incarnation != incarnation
            || record.key
                != scoped_key(
                    tenant,
                    incarnation,
                    "srr",
                    &record.semantic_repair_revision_id,
                )
            || record.semantic_repair_revision_digest
                != record_digest(record, "semantic_repair_revision_digest")?
        {
            return Err(conflict(
                "malformed or foreign signed semantic repair revision",
            ));
        }
        validate_revision_payload(
            &SemanticRepairRevisionPayload {
                semantic_repair_revision_id: record.semantic_repair_revision_id.clone(),
                target: record.target.clone(),
                base_promotion_head_decision_id: record.base_promotion_head_decision_id.clone(),
                candidate: record.candidate.clone(),
                candidate_digest: record.candidate_digest.clone(),
                author_registration_id: record.author_registration_id.clone(),
                author_principal_id: record.author_principal_id.clone(),
                signed_at_ms: record.signed_at_ms,
            },
            tenant,
            incarnation,
            record.created_at_ms,
        )?;
        record.created_by.require_role(Role::PolicyAuthor)?;
        validate_historical_key_use(
            author,
            KeyPurpose::PolicyAuthor,
            record.signed_at_ms,
            record.created_at_ms,
            None,
        )?;
        if author.registration_digest != record.author_registration_digest
            || author.principal_id != record.author_principal_id
            || author.subject_user_key != record.created_by.user_key
            || author.verification_key.purpose != KeyPurpose::PolicyAuthor
        {
            return Err(conflict(
                "semantic repair revision author authority mismatch",
            ));
        }
        let statement = GovernanceStatement::new(
            SEMANTIC_REPAIR_REVISION_DOMAIN,
            tenant,
            incarnation,
            SemanticRepairRevisionPayload {
                semantic_repair_revision_id: record.semantic_repair_revision_id.clone(),
                target: record.target.clone(),
                base_promotion_head_decision_id: record.base_promotion_head_decision_id.clone(),
                candidate: record.candidate.clone(),
                candidate_digest: record.candidate_digest.clone(),
                author_registration_id: record.author_registration_id.clone(),
                author_principal_id: record.author_principal_id.clone(),
                signed_at_ms: record.signed_at_ms,
            },
        );
        author
            .verification_key
            .verify(
                &statement,
                &record.author_signature,
                SEMANTIC_REPAIR_REVISION_DOMAIN,
                tenant,
                incarnation,
                KeyPurpose::PolicyAuthor,
            )
            .map_err(stored_signature_error)?;
        let expected_request_digest = canonical_digest(&CreateSemanticRepairRevisionRequest {
            statement,
            author_signature: record.author_signature.clone(),
        })?;
        if !is_digest(&record.idempotency_key_hash)
            || record.request_digest != expected_request_digest
        {
            return Err(conflict(
                "signed semantic repair revision request authority mismatch",
            ));
        }
        Ok(())
    }

    async fn validate_semantic_repair_review_record(
        &self,
        record: &SemanticRepairReviewRecord,
        tenant: &str,
        incarnation: &str,
    ) -> Result<(), CogniGraphError> {
        let revision = self
            .get_semantic_repair_revision_locked(
                tenant,
                incarnation,
                &record.semantic_repair_revision_id,
            )
            .await?;
        let author = self
            .get_active_key_locked(
                tenant,
                incarnation,
                &revision.author_registration_id,
                KeyPurpose::PolicyAuthor,
                record.reviewed_at_ms,
            )
            .await?;
        if author.registration_digest != revision.author_registration_digest
            || author.principal_id != revision.author_principal_id
        {
            return Err(conflict(
                "semantic repair review references different author authority",
            ));
        }
        let approver = self
            .get_key_locked(tenant, incarnation, &record.approver_registration_id)
            .await?;
        self.validate_semantic_repair_review_record_against(
            record,
            &revision,
            &approver,
            tenant,
            incarnation,
        )
    }

    async fn validate_stored_semantic_repair_review(
        &self,
        record: &SemanticRepairReviewRecord,
        tenant: &str,
        incarnation: &str,
    ) -> Result<(), CogniGraphError> {
        self.validate_semantic_repair_review_record(record, tenant, incarnation)
            .await
            .inspect_err(|error| self.record_error(tenant, error.to_string()))
    }

    pub(crate) fn validate_semantic_repair_review_record_against(
        &self,
        record: &SemanticRepairReviewRecord,
        revision: &SemanticRepairRevisionRecord,
        approver: &GovernanceKeyRecord,
        tenant: &str,
        incarnation: &str,
    ) -> Result<(), CogniGraphError> {
        if record.schema_version != SEMANTIC_REPAIR_RECORD_SCHEMA_VERSION
            || record.digest_algorithm != DIGEST_ALGORITHM
            || record.tenant != tenant
            || record.tenant_incarnation != incarnation
            || record.key
                != scoped_key(
                    tenant,
                    incarnation,
                    "srrv",
                    &record.semantic_repair_review_id,
                )
            || record.semantic_repair_review_digest
                != record_digest(record, "semantic_repair_review_digest")?
        {
            return Err(conflict(
                "malformed or foreign signed semantic repair review",
            ));
        }
        let payload = SemanticRepairReviewPayload {
            semantic_repair_review_id: record.semantic_repair_review_id.clone(),
            semantic_repair_revision_id: record.semantic_repair_revision_id.clone(),
            semantic_repair_revision_digest: record.semantic_repair_revision_digest.clone(),
            target: record.target.clone(),
            base_promotion_head_decision_id: record.base_promotion_head_decision_id.clone(),
            candidate_digest: record.candidate_digest.clone(),
            author_principal_id: record.author_principal_id.clone(),
            approver_registration_id: record.approver_registration_id.clone(),
            approver_principal_id: record.approver_principal_id.clone(),
            decision: record.decision,
            reason: record.reason.clone(),
            signed_at_ms: record.signed_at_ms,
        };
        validate_review_payload(&payload, tenant, incarnation, record.reviewed_at_ms)?;
        record.reviewed_by.require_role(Role::PolicyApprover)?;
        validate_historical_key_use(
            approver,
            KeyPurpose::PolicyApprover,
            record.signed_at_ms,
            record.reviewed_at_ms,
            None,
        )?;
        if revision.semantic_repair_revision_digest != record.semantic_repair_revision_digest
            || revision.target != record.target
            || revision.base_promotion_head_decision_id != record.base_promotion_head_decision_id
            || revision.candidate_digest != record.candidate_digest
            || revision.author_principal_id != record.author_principal_id
            || approver.registration_digest != record.approver_registration_digest
            || approver.principal_id != record.approver_principal_id
            || approver.subject_user_key != record.reviewed_by.user_key
            || approver.verification_key.purpose != KeyPurpose::PolicyApprover
            || approver.principal_id == revision.author_principal_id
            || record.signed_at_ms < revision.created_at_ms
            || record.reviewed_at_ms < revision.created_at_ms
        {
            return Err(conflict("semantic repair review authority mismatch"));
        }
        let statement =
            GovernanceStatement::new(SEMANTIC_REPAIR_REVIEW_DOMAIN, tenant, incarnation, payload);
        approver
            .verification_key
            .verify(
                &statement,
                &record.approver_signature,
                SEMANTIC_REPAIR_REVIEW_DOMAIN,
                tenant,
                incarnation,
                KeyPurpose::PolicyApprover,
            )
            .map_err(stored_signature_error)?;
        let expected_request_digest = canonical_digest(&ReviewSemanticRepairRevisionRequest {
            statement,
            approver_signature: record.approver_signature.clone(),
        })?;
        if !is_digest(&record.idempotency_key_hash)
            || record.request_digest != expected_request_digest
        {
            return Err(conflict(
                "signed semantic repair review request authority mismatch",
            ));
        }
        Ok(())
    }

    async fn ensure_semantic_repair_idempotency_available_locked(
        &self,
        tenant: &str,
        incarnation: &str,
        idempotency_key_hash: &str,
    ) -> Result<(), CogniGraphError> {
        self.ensure_governance_idempotency_available_locked(
            tenant,
            incarnation,
            idempotency_key_hash,
        )
        .await?;
        for (collection, kind) in [
            (SEMANTIC_REPAIR_REVISIONS_COLLECTION, "srr"),
            (SEMANTIC_REPAIR_REVIEWS_COLLECTION, "srrv"),
        ] {
            if self
                .semantic_repair_idempotency_hash_exists_locked(
                    collection,
                    kind,
                    tenant,
                    incarnation,
                    idempotency_key_hash,
                )
                .await?
            {
                return Err(conflict(
                    "Idempotency-Key was already used for different signed authority",
                ));
            }
        }
        Ok(())
    }

    async fn semantic_repair_idempotency_hash_exists_locked(
        &self,
        collection: &str,
        kind: &str,
        tenant: &str,
        incarnation: &str,
        idempotency_key_hash: &str,
    ) -> Result<bool, CogniGraphError> {
        let prefix = scoped_key(tenant, incarnation, kind, "");
        let fields = [
            "_key".to_string(),
            "tenant".to_string(),
            "tenant_incarnation".to_string(),
            "idempotency_key_hash".to_string(),
        ];
        let mut after = prefix.clone();
        loop {
            let rows = self
                .backend
                .list_documents_after_key(collection, Some(&after), &fields, 256)
                .await?;
            if rows.is_empty() {
                return Ok(false);
            }
            let mut advanced = false;
            for record in rows {
                let key = record
                    .get("_key")
                    .and_then(Value::as_str)
                    .ok_or_else(|| conflict("semantic repair record is missing `_key`"))?;
                if !key.starts_with(&prefix) {
                    return Ok(false);
                }
                after = key.to_string();
                advanced = true;
                if record.get("tenant").and_then(Value::as_str) != Some(tenant)
                    || record.get("tenant_incarnation").and_then(Value::as_str) != Some(incarnation)
                {
                    return Err(conflict("semantic repair record scope is malformed"));
                }
                let hash = record
                    .get("idempotency_key_hash")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        conflict("semantic repair record has no idempotency-key hash")
                    })?;
                if hash == idempotency_key_hash {
                    return Ok(true);
                }
            }
            if !advanced {
                return Ok(false);
            }
        }
    }

    /// Count revisions and canonical candidate bytes without re-fetching full
    /// records after reconciliation has already validated global authority.
    /// Eight projected candidates per keyset page preserve the 64 MiB bound
    /// even when every valid candidate is at the 8 MiB M22 ceiling.
    async fn semantic_repair_revision_capacity_locked(
        &self,
        tenant: &str,
        incarnation: &str,
    ) -> Result<(usize, usize), CogniGraphError> {
        let prefix = scoped_key(tenant, incarnation, "srr", "");
        let fields = vec!["_key".to_string(), "candidate".to_string()];
        let mut after = prefix.clone();
        let mut count = 0_usize;
        let mut candidate_bytes = 0_usize;
        loop {
            let rows = self
                .backend
                .list_documents_after_key(
                    SEMANTIC_REPAIR_REVISIONS_COLLECTION,
                    Some(&after),
                    &fields,
                    MAX_SEMANTIC_REPAIR_REVISION_PAGE_SIZE,
                )
                .await?;
            if rows.is_empty() {
                return Ok((count, candidate_bytes));
            }
            for record in rows {
                let key = record
                    .get("_key")
                    .and_then(Value::as_str)
                    .ok_or_else(|| conflict("semantic repair revision is missing `_key`"))?;
                if !key.starts_with(&prefix) {
                    return Ok((count, candidate_bytes));
                }
                after = key.to_string();
                count = count
                    .checked_add(1)
                    .ok_or_else(|| conflict("semantic repair revision count overflowed"))?;
                if count > MAX_SEMANTIC_REPAIR_REVISIONS_PER_TENANT {
                    return Err(conflict("semantic repair revision capacity exceeded"));
                }
                let candidate = record
                    .get("candidate")
                    .ok_or_else(|| conflict("semantic repair revision is missing its candidate"))?;
                candidate_bytes = candidate_bytes
                    .checked_add(canonical_json_bytes(candidate)?.len())
                    .ok_or_else(|| {
                        conflict("semantic repair candidate storage accounting overflowed")
                    })?;
                if candidate_bytes > MAX_SEMANTIC_REPAIR_CANDIDATE_BYTES_PER_TENANT {
                    return Err(conflict(
                        "semantic repair candidate storage capacity exceeded",
                    ));
                }
            }
        }
    }

    async fn semantic_repair_record_count_locked(
        &self,
        collection: &str,
        tenant: &str,
        incarnation: &str,
        kind: &str,
        maximum: usize,
    ) -> Result<usize, CogniGraphError> {
        let prefix = scoped_key(tenant, incarnation, kind, "");
        let fields = vec!["_key".to_string()];
        let mut after = prefix.clone();
        let mut count = 0_usize;
        loop {
            let rows = self
                .backend
                .list_documents_after_key(collection, Some(&after), &fields, 256)
                .await?;
            if rows.is_empty() {
                return Ok(count);
            }
            for record in rows {
                let key = record
                    .get("_key")
                    .and_then(Value::as_str)
                    .ok_or_else(|| conflict("semantic repair record is missing `_key`"))?;
                if !key.starts_with(&prefix) {
                    return Ok(count);
                }
                after = key.to_string();
                count = count
                    .checked_add(1)
                    .ok_or_else(|| conflict("semantic repair record count overflowed"))?;
                if count > maximum {
                    return Err(conflict("semantic repair record capacity exceeded"));
                }
            }
        }
    }
}

pub fn semantic_repair_revision_id(
    tenant: &str,
    incarnation: &str,
    target: &PromotionTarget,
    base_promotion_head_decision_id: Option<&str>,
    candidate_digest: &str,
) -> Result<String, CogniGraphError> {
    target.validate()?;
    if let Some(id) = base_promotion_head_decision_id {
        validate_record_id("base_promotion_head_decision_id", id)?;
    }
    validate_digest("candidate_digest", candidate_digest)?;
    let identity = json!({
        "tenant": tenant,
        "tenant_incarnation": incarnation,
        "target": target,
        "base_promotion_head_decision_id": base_promotion_head_decision_id,
        "candidate_digest": candidate_digest,
    });
    Ok(canonical_digest(&identity)?
        .trim_start_matches("sha256:")
        .to_string())
}

pub fn semantic_repair_review_id(
    tenant: &str,
    incarnation: &str,
    semantic_repair_revision_id: &str,
) -> String {
    digest_bytes(
        format!("{tenant}\0{incarnation}\0semantic-repair-review\0{semantic_repair_revision_id}")
            .as_bytes(),
    )
    .trim_start_matches("sha256:")
    .to_string()
}

fn validate_revision_payload(
    payload: &SemanticRepairRevisionPayload,
    tenant: &str,
    incarnation: &str,
    now: u64,
) -> Result<(), CogniGraphError> {
    payload.target.validate()?;
    validate_record_id(
        "semantic_repair_revision_id",
        &payload.semantic_repair_revision_id,
    )?;
    if let Some(id) = &payload.base_promotion_head_decision_id {
        validate_record_id("base_promotion_head_decision_id", id)?;
    }
    validate_identifier("author_registration_id", &payload.author_registration_id)?;
    validate_identifier("author_principal_id", &payload.author_principal_id)?;
    validate_signed_at(payload.signed_at_ms, now)?;
    payload
        .candidate
        .validate_semantic_repair_candidate(&payload.target)?;
    let candidate_digest = canonical_digest(&payload.candidate)?;
    if payload.candidate_digest != candidate_digest
        || payload.semantic_repair_revision_id
            != semantic_repair_revision_id(
                tenant,
                incarnation,
                &payload.target,
                payload.base_promotion_head_decision_id.as_deref(),
                &payload.candidate_digest,
            )?
    {
        return Err(validation(
            "semantic repair revision identity or canonical candidate digest mismatch",
        ));
    }
    Ok(())
}

fn validate_review_payload(
    payload: &SemanticRepairReviewPayload,
    tenant: &str,
    incarnation: &str,
    now: u64,
) -> Result<(), CogniGraphError> {
    payload.target.validate()?;
    validate_record_id(
        "semantic_repair_review_id",
        &payload.semantic_repair_review_id,
    )?;
    validate_record_id(
        "semantic_repair_revision_id",
        &payload.semantic_repair_revision_id,
    )?;
    validate_digest(
        "semantic_repair_revision_digest",
        &payload.semantic_repair_revision_digest,
    )?;
    if let Some(id) = &payload.base_promotion_head_decision_id {
        validate_record_id("base_promotion_head_decision_id", id)?;
    }
    validate_digest("candidate_digest", &payload.candidate_digest)?;
    validate_identifier("author_principal_id", &payload.author_principal_id)?;
    validate_identifier(
        "approver_registration_id",
        &payload.approver_registration_id,
    )?;
    validate_identifier("approver_principal_id", &payload.approver_principal_id)?;
    validate_reason(&payload.reason)?;
    validate_signed_at(payload.signed_at_ms, now)?;
    if payload.author_principal_id == payload.approver_principal_id {
        return Err(CogniGraphError::Forbidden(
            "semantic repair author and reviewer principals must be distinct".into(),
        ));
    }
    if payload.semantic_repair_review_id
        != semantic_repair_review_id(tenant, incarnation, &payload.semantic_repair_revision_id)
    {
        return Err(validation(
            "semantic repair review identity does not match its revision",
        ));
    }
    Ok(())
}

fn validate_statement_scope<T>(
    statement: &GovernanceStatement<T>,
    domain: &str,
    tenant: &str,
    incarnation: &str,
) -> Result<(), CogniGraphError> {
    if statement.schema_version != cognigraph_governance::GOVERNANCE_SCHEMA_VERSION
        || statement.domain != domain
        || statement.tenant != tenant
        || statement.tenant_incarnation != incarnation
    {
        return Err(validation(
            "signed semantic repair statement domain or tenant scope mismatch",
        ));
    }
    Ok(())
}

fn validate_signed_at(signed_at_ms: u64, now: u64) -> Result<(), CogniGraphError> {
    if signed_at_ms == 0 || signed_at_ms > now.saturating_add(MAX_CLOCK_SKEW_MS) {
        return Err(validation(
            "semantic repair signed_at_ms is zero or more than five minutes in the future",
        ));
    }
    Ok(())
}

fn validate_identifier(label: &str, value: &str) -> Result<(), CogniGraphError> {
    if value.trim().is_empty()
        || value.len() > MAX_IDENTIFIER_BYTES
        || value.chars().any(char::is_control)
        || !is_nfc(value)
    {
        return Err(validation(format!(
            "{label} must be non-empty, NFC-normalized, control-free, and at most {MAX_IDENTIFIER_BYTES} bytes"
        )));
    }
    Ok(())
}

fn validate_record_id(label: &str, value: &str) -> Result<(), CogniGraphError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(validation(format!(
            "{label} must be 64 lowercase hexadecimal characters"
        )));
    }
    Ok(())
}

fn validate_digest(label: &str, value: &str) -> Result<(), CogniGraphError> {
    if !is_digest(value) {
        return Err(validation(format!(
            "{label} must be sha256:<64 lowercase hex characters>"
        )));
    }
    Ok(())
}

fn is_digest(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

fn validate_page_size(
    limit: usize,
    maximum: usize,
    record_kind: &str,
) -> Result<(), CogniGraphError> {
    if !(1..=maximum).contains(&limit) {
        return Err(validation(format!(
            "semantic repair {record_kind} list limit must be between 1 and {maximum}"
        )));
    }
    Ok(())
}

fn validate_resolution_separation(
    promoter_principal_id: Option<&str>,
    author_principal_id: &str,
    approver_principal_id: &str,
) -> Result<(), CogniGraphError> {
    let promoter_principal_id = promoter_principal_id.ok_or_else(|| {
        conflict("M25 semantic repair resolution requires a signed promotion decision")
    })?;
    if promoter_principal_id == author_principal_id
        || promoter_principal_id == approver_principal_id
        || author_principal_id == approver_principal_id
    {
        return Err(CogniGraphError::Forbidden(
            "semantic repair author, reviewer, and promoter principals must be distinct".into(),
        ));
    }
    Ok(())
}

fn promotion_principal_id(decision: &PromotionDecision) -> Result<&str, CogniGraphError> {
    decision
        .governance
        .as_ref()
        .map(|signed| signed.statement.payload.promoter_principal_id.as_str())
        .ok_or_else(|| {
            conflict("M25 semantic repair resolution requires signed promotion decisions")
        })
}

fn same_selected_authority(
    left: &crate::promotions::PromotionSelection,
    right: &crate::promotions::PromotionSelection,
) -> bool {
    left.evidence_id == right.evidence_id
        && left.evidence_digest == right.evidence_digest
        && left.candidate_digest == right.candidate_digest
        && left.policy_digest == right.policy_digest
}

fn compatible_historical_selection(
    promoted: &crate::promotions::PromotionSelection,
    historical_head: &crate::promotions::PromotionSelection,
) -> bool {
    promoted.generation <= historical_head.generation
        && same_selected_authority(promoted, historical_head)
}

pub(crate) fn ensure_semantic_repair_candidate_capacity<I>(
    candidate_sizes: I,
) -> Result<(), CogniGraphError>
where
    I: IntoIterator<Item = Result<usize, CogniGraphError>>,
{
    let mut total = 0_usize;
    for size in candidate_sizes {
        total = total
            .checked_add(size?)
            .ok_or_else(|| conflict("semantic repair candidate storage accounting overflowed"))?;
        if total > MAX_SEMANTIC_REPAIR_CANDIDATE_BYTES_PER_TENANT {
            return Err(conflict(format!(
                "tenant semantic repair candidate storage limit of {MAX_SEMANTIC_REPAIR_CANDIDATE_BYTES_PER_TENANT} canonical bytes is exhausted"
            )));
        }
    }
    Ok(())
}

fn deserialize_required_option<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

fn require_key_actor(
    key: &GovernanceKeyRecord,
    actor: &GovernanceActor,
    principal_id: &str,
) -> Result<(), CogniGraphError> {
    if key.subject_user_key != actor.user_key
        || key.principal_id != principal_id
        || key.principal_id.trim().is_empty()
    {
        return Err(CogniGraphError::Forbidden(
            "authenticated user does not own the semantic repair signing principal".into(),
        ));
    }
    Ok(())
}

fn public_value<T: Serialize>(record: &T) -> Result<Value, CogniGraphError> {
    let mut value = serde_json::to_value(record)?;
    if let Some(fields) = value.as_object_mut() {
        fields.remove("_key");
        fields.remove("idempotency_key_hash");
    }
    Ok(value)
}

pub(crate) fn deserialize_semantic_repair_stored<T: DeserializeOwned>(
    mut value: Value,
) -> Result<T, CogniGraphError> {
    if let Some(fields) = value.as_object_mut() {
        fields.remove("_id");
        fields.remove("_rev");
        fields.remove("created_at");
        fields.remove("updated_at");
    }
    serde_json::from_value(value).map_err(CogniGraphError::from)
}

fn signature_error(error: cognigraph_governance::GovernanceError) -> CogniGraphError {
    CogniGraphError::Forbidden(format!("semantic repair signature rejected: {error}"))
}

fn stored_signature_error(error: cognigraph_governance::GovernanceError) -> CogniGraphError {
    conflict(format!(
        "stored semantic repair signature is invalid: {error}"
    ))
}

fn validation(message: impl Into<String>) -> CogniGraphError {
    CogniGraphError::ValidationError(message.into())
}

fn conflict(message: impl Into<String>) -> CogniGraphError {
    CogniGraphError::DocumentConflict(message.into())
}

fn not_found(collection: &str, id: &str) -> CogniGraphError {
    CogniGraphError::DocumentNotFound {
        collection: collection.into(),
        key: id.into(),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::artifact_consumption::{
        ConstructionEntity, ConstructionNeuron, ConstructionRelationRule, ConstructionSpaceType,
    };
    use crate::governance::{
        KEY_REGISTRATION_DOMAIN, KEY_REVOCATION_DOMAIN, KeyRegistrationPayload,
        KeyRevocationPayload, RegisterGovernanceKeyRequest, RevokeGovernanceKeyRequest,
        key_registration_id, key_revocation_id,
    };
    use crate::promotions::digest_bytes;
    use crate::state::AppState;
    use cognigraph_auth::User;
    use cognigraph_core::GraphBackend;
    use cognigraph_governance::{KeyPurpose, SigningKeyMaterial};
    use cognigraph_native::NativeBackend;

    const TENANT: &str = "default";
    const INCARNATION: &str = "default";

    struct TestPrincipal {
        signing_key: SigningKeyMaterial,
        actor: GovernanceActor,
        record: GovernanceKeyRecord,
    }

    fn user(role: Role, key: &str) -> User {
        User {
            key: key.into(),
            username: format!("{key}@example.test"),
            role,
            tenant: TENANT.into(),
        }
    }

    async fn register_principal(
        state: &AppState,
        root: &SigningKeyMaterial,
        purpose: KeyPurpose,
        role: Role,
        principal_id: &str,
        user_key: &str,
    ) -> TestPrincipal {
        let signing_key = SigningKeyMaterial::generate(purpose).unwrap();
        let verification_key = signing_key.verification_key().unwrap();
        let now = now_millis();
        let statement = GovernanceStatement::new(
            KEY_REGISTRATION_DOMAIN,
            TENANT,
            INCARNATION,
            KeyRegistrationPayload {
                registration_id: key_registration_id(TENANT, INCARNATION, &verification_key.key_id),
                principal_id: principal_id.into(),
                subject_user_key: user_key.into(),
                verification_key,
                not_before_ms: now.saturating_sub(1),
                not_after_ms: None,
                signed_at_ms: now,
            },
        );
        let actor = GovernanceActor::from_user(user(role, user_key));
        let record = state
            .promotions
            .register_governance_key(
                TENANT,
                INCARNATION,
                GovernanceActor::from_user(user(Role::Admin, "governance-admin")),
                user(role, user_key),
                &format!("register-{principal_id}"),
                RegisterGovernanceKeyRequest {
                    root_signature: root.sign(&statement).unwrap(),
                    possession_signature: signing_key.sign(&statement).unwrap(),
                    statement,
                },
            )
            .await
            .unwrap()
            .record;
        TestPrincipal {
            signing_key,
            actor,
            record,
        }
    }

    fn candidate() -> ConstructionCandidateArtifact {
        ConstructionCandidateArtifact {
            schema_version: 1,
            kind: "semantic-neuron-bundle".into(),
            id: "repair-v2".into(),
            revision: "2".into(),
            base_space_type: ConstructionSpaceType {
                id: "pharma".into(),
                name: "Pharma".into(),
                version: 1,
                description: String::new(),
                entities: vec![
                    ConstructionEntity {
                        name: "Compound".into(),
                        entity_type: "compound".into(),
                        aliases: Vec::new(),
                    },
                    ConstructionEntity {
                        name: "Supplier".into(),
                        entity_type: "organization".into(),
                        aliases: Vec::new(),
                    },
                ],
                relation_rules: vec![ConstructionRelationRule {
                    source: "Supplier".into(),
                    relation: "SUPPLIES".into(),
                    target: "Compound".into(),
                    when_any: vec!["supplies".into()],
                    require_in_sentence: Vec::new(),
                }],
            },
            accepted_neurons: vec![ConstructionNeuron::RelationHint {
                id: "n-1".into(),
                reviewed_by: Some("reviewer".into()),
                evidence: vec!["Supplier provides Compound".into()],
                source: "Supplier".into(),
                relation: "SUPPLIES".into(),
                target: "Compound".into(),
                triggers: vec!["provides".into()],
            }],
        }
    }

    fn target() -> PromotionTarget {
        PromotionTarget {
            space_type: "pharma".into(),
            channel: "stable".into(),
        }
    }

    fn signed_revision_request(
        author: &TestPrincipal,
        candidate: ConstructionCandidateArtifact,
        base: Option<String>,
    ) -> CreateSemanticRepairRevisionRequest {
        let target = target();
        let candidate_digest = canonical_digest(&candidate).unwrap();
        let statement = GovernanceStatement::new(
            SEMANTIC_REPAIR_REVISION_DOMAIN,
            TENANT,
            INCARNATION,
            SemanticRepairRevisionPayload {
                semantic_repair_revision_id: semantic_repair_revision_id(
                    TENANT,
                    INCARNATION,
                    &target,
                    base.as_deref(),
                    &candidate_digest,
                )
                .unwrap(),
                target,
                base_promotion_head_decision_id: base,
                candidate,
                candidate_digest,
                author_registration_id: author.record.registration_id.clone(),
                author_principal_id: author.record.principal_id.clone(),
                signed_at_ms: now_millis(),
            },
        );
        CreateSemanticRepairRevisionRequest {
            author_signature: author.signing_key.sign(&statement).unwrap(),
            statement,
        }
    }

    fn signed_review_request(
        approver: &TestPrincipal,
        revision: &SemanticRepairRevisionRecord,
        decision: SemanticRepairReviewDecision,
    ) -> ReviewSemanticRepairRevisionRequest {
        let statement = GovernanceStatement::new(
            SEMANTIC_REPAIR_REVIEW_DOMAIN,
            TENANT,
            INCARNATION,
            SemanticRepairReviewPayload {
                semantic_repair_review_id: semantic_repair_review_id(
                    TENANT,
                    INCARNATION,
                    &revision.semantic_repair_revision_id,
                ),
                semantic_repair_revision_id: revision.semantic_repair_revision_id.clone(),
                semantic_repair_revision_digest: revision.semantic_repair_revision_digest.clone(),
                target: revision.target.clone(),
                base_promotion_head_decision_id: revision.base_promotion_head_decision_id.clone(),
                candidate_digest: revision.candidate_digest.clone(),
                author_principal_id: revision.author_principal_id.clone(),
                approver_registration_id: approver.record.registration_id.clone(),
                approver_principal_id: approver.record.principal_id.clone(),
                decision,
                reason: "independent semantic repair review".into(),
                signed_at_ms: now_millis().max(revision.created_at_ms),
            },
        );
        ReviewSemanticRepairRevisionRequest {
            approver_signature: approver.signing_key.sign(&statement).unwrap(),
            statement,
        }
    }

    #[test]
    fn revision_identity_binds_target_base_and_canonical_candidate() {
        let target = PromotionTarget {
            space_type: "pharma".into(),
            channel: "stable".into(),
        };
        let candidate = candidate();
        let digest = canonical_digest(&candidate).unwrap();
        let base = digest_bytes(b"base")
            .trim_start_matches("sha256:")
            .to_string();
        let id =
            semantic_repair_revision_id("tenant", "inc", &target, Some(&base), &digest).unwrap();
        assert_eq!(id.len(), 64);
        assert_ne!(
            id,
            semantic_repair_revision_id(
                "tenant",
                "inc",
                &PromotionTarget {
                    channel: "canary".into(),
                    ..target
                },
                Some(&base),
                &digest,
            )
            .unwrap()
        );
        assert_ne!(
            id,
            semantic_repair_revision_id(
                "tenant",
                "inc",
                &PromotionTarget {
                    space_type: "pharma".into(),
                    channel: "stable".into(),
                },
                None,
                &digest,
            )
            .unwrap()
        );
    }

    #[test]
    fn revision_payload_rejects_noncanonical_candidate_digest() {
        let target = PromotionTarget {
            space_type: "pharma".into(),
            channel: "stable".into(),
        };
        let candidate = candidate();
        let base = digest_bytes(b"base")
            .trim_start_matches("sha256:")
            .to_string();
        let wrong = digest_bytes(b"wrong");
        let payload = SemanticRepairRevisionPayload {
            semantic_repair_revision_id: semantic_repair_revision_id(
                "tenant",
                "inc",
                &target,
                Some(&base),
                &wrong,
            )
            .unwrap(),
            target,
            base_promotion_head_decision_id: Some(base),
            candidate,
            candidate_digest: wrong,
            author_registration_id: "author-key".into(),
            author_principal_id: "author".into(),
            signed_at_ms: 1,
        };
        assert!(validate_revision_payload(&payload, "tenant", "inc", 1).is_err());
    }

    #[test]
    fn review_identity_is_one_immutable_decision_per_revision() {
        let revision = digest_bytes(b"revision")
            .trim_start_matches("sha256:")
            .to_string();
        assert_eq!(
            semantic_repair_review_id("tenant", "inc", &revision),
            semantic_repair_review_id("tenant", "inc", &revision)
        );
        assert_ne!(
            semantic_repair_review_id("tenant", "inc", &revision),
            semantic_repair_review_id("tenant", "other", &revision)
        );
    }

    #[test]
    fn signed_base_field_requires_explicit_null_or_value() {
        let candidate = candidate();
        let target = target();
        let candidate_digest = canonical_digest(&candidate).unwrap();
        let mut value = json!({
            "semantic_repair_revision_id": semantic_repair_revision_id(
                TENANT,
                INCARNATION,
                &target,
                None,
                &candidate_digest,
            ).unwrap(),
            "target": target,
            "base_promotion_head_decision_id": null,
            "candidate": candidate,
            "candidate_digest": candidate_digest,
            "author_registration_id": "author-key",
            "author_principal_id": "author-principal",
            "signed_at_ms": 1,
        });
        let decoded: SemanticRepairRevisionPayload = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(decoded.base_promotion_head_decision_id, None);
        value
            .as_object_mut()
            .unwrap()
            .remove("base_promotion_head_decision_id");
        assert!(serde_json::from_value::<SemanticRepairRevisionPayload>(value).is_err());
    }

    #[test]
    fn signed_review_base_field_requires_explicit_null_or_value() {
        let revision_id = digest_bytes(b"revision-id")
            .trim_start_matches("sha256:")
            .to_string();
        let revision_digest = digest_bytes(b"revision-record");
        let candidate_digest = digest_bytes(b"candidate");
        let mut value = json!({
            "semantic_repair_review_id": semantic_repair_review_id(
                TENANT,
                INCARNATION,
                &revision_id,
            ),
            "semantic_repair_revision_id": revision_id,
            "semantic_repair_revision_digest": revision_digest,
            "target": target(),
            "base_promotion_head_decision_id": null,
            "candidate_digest": candidate_digest,
            "author_principal_id": "author-principal",
            "approver_registration_id": "approver-key",
            "approver_principal_id": "approver-principal",
            "decision": "approve",
            "reason": "independent review",
            "signed_at_ms": 1,
        });
        let decoded: SemanticRepairReviewPayload = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(decoded.base_promotion_head_decision_id, None);

        let mut omitted = value.clone();
        omitted
            .as_object_mut()
            .unwrap()
            .remove("base_promotion_head_decision_id");
        assert!(serde_json::from_value::<SemanticRepairReviewPayload>(omitted).is_err());

        value["base_promotion_head_decision_id"] = json!(
            digest_bytes(b"promotion-head")
                .trim_start_matches("sha256:")
                .to_string()
        );
        let populated: SemanticRepairReviewPayload = serde_json::from_value(value).unwrap();
        assert!(populated.base_promotion_head_decision_id.is_some());
    }

    #[test]
    fn nested_reviewed_by_rejects_explicit_null_without_changing_valid_shapes() {
        let mut omitted = serde_json::to_value(candidate()).unwrap();
        omitted["accepted_neurons"][0]
            .as_object_mut()
            .unwrap()
            .remove("reviewed_by");
        let decoded: ConstructionCandidateArtifact =
            serde_json::from_value(omitted.clone()).unwrap();
        assert!(matches!(
            &decoded.accepted_neurons[0],
            ConstructionNeuron::RelationHint {
                reviewed_by: None,
                ..
            }
        ));
        let mut explicit_null = omitted.clone();
        explicit_null["accepted_neurons"][0]["reviewed_by"] = Value::Null;
        assert!(serde_json::from_value::<ConstructionCandidateArtifact>(explicit_null).is_err());
        omitted["accepted_neurons"][0]["reviewed_by"] = json!("reviewer");
        assert!(serde_json::from_value::<ConstructionCandidateArtifact>(omitted).is_ok());
    }

    #[test]
    fn semantic_repair_candidate_rejects_wrong_kind_and_oversize_canonical_json() {
        let mut wrong_kind = candidate();
        wrong_kind.kind = "generic-config".into();
        assert!(
            wrong_kind
                .validate_semantic_repair_candidate(&target())
                .is_err()
        );

        let mut oversized = candidate();
        oversized.base_space_type.description = "x"
            .repeat((crate::artifact_consumption::MAX_M22_CANDIDATE_ARTIFACT_BYTES + 1) as usize);
        assert!(matches!(
            oversized.validate_semantic_repair_candidate(&target()),
            Err(CogniGraphError::CapacityExceeded(_))
        ));
    }

    #[test]
    fn semantic_repair_candidate_aggregate_capacity_is_checked_and_overflow_safe() {
        assert!(
            ensure_semantic_repair_candidate_capacity([
                Ok(MAX_SEMANTIC_REPAIR_CANDIDATE_BYTES_PER_TENANT / 2),
                Ok(MAX_SEMANTIC_REPAIR_CANDIDATE_BYTES_PER_TENANT / 2),
            ])
            .is_ok()
        );
        assert!(matches!(
            ensure_semantic_repair_candidate_capacity([
                Ok(MAX_SEMANTIC_REPAIR_CANDIDATE_BYTES_PER_TENANT),
                Ok(1),
            ]),
            Err(CogniGraphError::DocumentConflict(_))
        ));
        assert!(matches!(
            ensure_semantic_repair_candidate_capacity([Ok(usize::MAX), Ok(1)]),
            Err(CogniGraphError::DocumentConflict(_))
        ));
    }

    #[test]
    fn resolution_requires_signed_three_way_separation() {
        assert!(validate_resolution_separation(None, "author", "reviewer").is_err());
        assert!(validate_resolution_separation(Some("author"), "author", "reviewer").is_err());
        assert!(validate_resolution_separation(Some("reviewer"), "author", "reviewer").is_err());
        assert!(validate_resolution_separation(Some("promoter"), "author", "reviewer").is_ok());

        let original = crate::promotions::PromotionSelection {
            generation: 2,
            evidence_id: digest_bytes(b"evidence")
                .trim_start_matches("sha256:")
                .to_string(),
            evidence_digest: digest_bytes(b"evidence-record"),
            candidate_digest: digest_bytes(b"candidate"),
            policy_digest: digest_bytes(b"policy"),
            prior_evidence_id: Some(
                digest_bytes(b"prior-evidence")
                    .trim_start_matches("sha256:")
                    .to_string(),
            ),
        };
        let restored = crate::promotions::PromotionSelection {
            generation: 4,
            prior_evidence_id: Some(original.evidence_id.clone()),
            ..original.clone()
        };
        assert!(same_selected_authority(&original, &restored));
        assert!(compatible_historical_selection(&original, &restored));
        assert!(!compatible_historical_selection(&restored, &original));
    }

    #[tokio::test]
    async fn signed_revision_and_review_are_immutable_idempotent_and_historical() {
        let state = AppState::new(NativeBackend::new());
        let root = SigningKeyMaterial::generate(KeyPurpose::TrustRoot).unwrap();
        state
            .promotions
            .configure_governance_root(Some(&root.public_key))
            .unwrap();
        let author = register_principal(
            &state,
            &root,
            KeyPurpose::PolicyAuthor,
            Role::PolicyAuthor,
            "semantic-author",
            "semantic-author-user",
        )
        .await;
        let approver = register_principal(
            &state,
            &root,
            KeyPurpose::PolicyApprover,
            Role::PolicyApprover,
            "semantic-reviewer",
            "semantic-reviewer-user",
        )
        .await;
        let request = signed_revision_request(&author, candidate(), None);
        assert!(matches!(
            state
                .promotions
                .create_semantic_repair_revision(
                    TENANT,
                    INCARNATION,
                    approver.actor.clone(),
                    "wrong-role-revision",
                    request.clone(),
                )
                .await,
            Err(CogniGraphError::Forbidden(_))
        ));
        let mut foreign_scope = request.clone();
        foreign_scope.statement.tenant = "other-tenant".into();
        foreign_scope.author_signature = author.signing_key.sign(&foreign_scope.statement).unwrap();
        assert!(matches!(
            state
                .promotions
                .create_semantic_repair_revision(
                    TENANT,
                    INCARNATION,
                    author.actor.clone(),
                    "foreign-scope-revision",
                    foreign_scope,
                )
                .await,
            Err(CogniGraphError::ValidationError(_))
        ));
        let created = state
            .promotions
            .create_semantic_repair_revision(
                TENANT,
                INCARNATION,
                author.actor.clone(),
                "semantic-revision-1",
                request.clone(),
            )
            .await
            .unwrap();
        assert!(!created.replayed);
        assert_eq!(
            created.record.candidate_digest,
            canonical_digest(&candidate()).unwrap()
        );
        let replay = state
            .promotions
            .create_semantic_repair_revision(
                TENANT,
                INCARNATION,
                author.actor.clone(),
                "semantic-revision-1",
                request.clone(),
            )
            .await
            .unwrap();
        assert!(replay.replayed);
        assert_eq!(replay.record, created.record);
        assert!(matches!(
            state
                .promotions
                .create_semantic_repair_revision(
                    TENANT,
                    INCARNATION,
                    author.actor.clone(),
                    "different-idempotency-key",
                    request,
                )
                .await,
            Err(CogniGraphError::DocumentConflict(_))
        ));

        let mut same_principal_review = signed_review_request(
            &approver,
            &created.record,
            SemanticRepairReviewDecision::Approve,
        );
        same_principal_review
            .statement
            .payload
            .approver_principal_id = created.record.author_principal_id.clone();
        same_principal_review.approver_signature = approver
            .signing_key
            .sign(&same_principal_review.statement)
            .unwrap();
        assert!(matches!(
            state
                .promotions
                .review_semantic_repair_revision(
                    TENANT,
                    INCARNATION,
                    approver.actor.clone(),
                    "same-principal-review",
                    same_principal_review,
                )
                .await,
            Err(CogniGraphError::Forbidden(_))
        ));

        let review_request = signed_review_request(
            &approver,
            &created.record,
            SemanticRepairReviewDecision::Approve,
        );
        let reviewed = state
            .promotions
            .review_semantic_repair_revision(
                TENANT,
                INCARNATION,
                approver.actor.clone(),
                "semantic-review-1",
                review_request.clone(),
            )
            .await
            .unwrap();
        assert_eq!(
            reviewed.record.decision,
            SemanticRepairReviewDecision::Approve
        );
        assert!(
            state
                .promotions
                .review_semantic_repair_revision(
                    TENANT,
                    INCARNATION,
                    approver.actor.clone(),
                    "semantic-review-1",
                    review_request,
                )
                .await
                .unwrap()
                .replayed
        );
        let rejected_request = signed_review_request(
            &approver,
            &created.record,
            SemanticRepairReviewDecision::Reject,
        );
        assert!(matches!(
            state
                .promotions
                .review_semantic_repair_revision(
                    TENANT,
                    INCARNATION,
                    approver.actor.clone(),
                    "semantic-review-reject",
                    rejected_request,
                )
                .await,
            Err(CogniGraphError::DocumentConflict(_))
        ));

        let mut second_candidate = candidate();
        second_candidate.id = "repair-v3".into();
        second_candidate.revision = "3".into();
        let mut wrong_signature = signed_revision_request(&author, second_candidate.clone(), None);
        wrong_signature.author_signature = approver
            .signing_key
            .sign(&wrong_signature.statement)
            .unwrap();
        assert!(matches!(
            state
                .promotions
                .create_semantic_repair_revision(
                    TENANT,
                    INCARNATION,
                    author.actor.clone(),
                    "wrong-signature-revision",
                    wrong_signature,
                )
                .await,
            Err(CogniGraphError::Forbidden(_))
        ));
        let second = state
            .promotions
            .create_semantic_repair_revision(
                TENANT,
                INCARNATION,
                author.actor.clone(),
                "semantic-revision-2",
                signed_revision_request(&author, second_candidate, None),
            )
            .await
            .unwrap();

        let first_page = state
            .promotions
            .list_semantic_repair_revisions(TENANT, INCARNATION, 1, None)
            .await
            .unwrap();
        assert_eq!(first_page.records.len(), 1);
        let first_cursor = first_page.next_cursor.expect("second revision page");
        let second_page = state
            .promotions
            .list_semantic_repair_revisions(TENANT, INCARNATION, 1, Some(first_cursor.as_str()))
            .await
            .unwrap();
        assert_eq!(second_page.records.len(), 1);
        assert!(second_page.next_cursor.is_none());
        assert_ne!(
            first_page.records[0].semantic_repair_revision_id,
            second_page.records[0].semantic_repair_revision_id
        );
        assert!(matches!(
            state
                .promotions
                .list_semantic_repair_revisions(
                    TENANT,
                    INCARNATION,
                    MAX_SEMANTIC_REPAIR_REVISION_PAGE_SIZE + 1,
                    None,
                )
                .await,
            Err(CogniGraphError::ValidationError(_))
        ));

        let revoked_at = now_millis();
        let revocation_statement = GovernanceStatement::new(
            KEY_REVOCATION_DOMAIN,
            TENANT,
            INCARNATION,
            KeyRevocationPayload {
                revocation_id: key_revocation_id(
                    TENANT,
                    INCARNATION,
                    &author.record.registration_id,
                ),
                registration_id: author.record.registration_id.clone(),
                key_id: author.record.verification_key.key_id.clone(),
                public_key_digest: author.record.public_key_digest.clone(),
                reason: "rotate semantic repair author key".into(),
                signed_at_ms: revoked_at,
                effective_at_ms: revoked_at,
            },
        );
        state
            .promotions
            .revoke_governance_key(
                TENANT,
                INCARNATION,
                GovernanceActor::from_user(user(Role::Admin, "governance-admin")),
                "revoke-semantic-author",
                RevokeGovernanceKeyRequest {
                    root_signature: root.sign(&revocation_statement).unwrap(),
                    statement: revocation_statement,
                },
            )
            .await
            .unwrap();
        state
            .promotions
            .recover_governance_locked(TENANT, INCARNATION)
            .await
            .expect("pre-revocation revision and review remain valid history");

        let post_revocation_review = signed_review_request(
            &approver,
            &second.record,
            SemanticRepairReviewDecision::Approve,
        );
        assert!(matches!(
            state
                .promotions
                .review_semantic_repair_revision(
                    TENANT,
                    INCARNATION,
                    approver.actor.clone(),
                    "semantic-review-after-author-revocation",
                    post_revocation_review,
                )
                .await,
            Err(CogniGraphError::Forbidden(_))
        ));
        let mut third_candidate = candidate();
        third_candidate.id = "repair-v4".into();
        third_candidate.revision = "4".into();
        assert!(matches!(
            state
                .promotions
                .create_semantic_repair_revision(
                    TENANT,
                    INCARNATION,
                    author.actor.clone(),
                    "semantic-revision-after-revocation",
                    signed_revision_request(&author, third_candidate, None),
                )
                .await,
            Err(CogniGraphError::Forbidden(_))
        ));

        state.jobs.shutdown().await;
    }

    #[tokio::test]
    async fn snapshot_preflight_validates_semantic_repair_union_before_applying() {
        let root = SigningKeyMaterial::generate(KeyPurpose::TrustRoot).unwrap();
        let source_raw: Arc<dyn GraphBackend> = Arc::new(NativeBackend::new());
        let source = AppState::new_shared(source_raw.clone());
        source
            .promotions
            .configure_governance_root(Some(&root.public_key))
            .unwrap();
        let author = register_principal(
            &source,
            &root,
            KeyPurpose::PolicyAuthor,
            Role::PolicyAuthor,
            "snapshot-semantic-author",
            "snapshot-semantic-author-user",
        )
        .await;
        let approver = register_principal(
            &source,
            &root,
            KeyPurpose::PolicyApprover,
            Role::PolicyApprover,
            "snapshot-semantic-approver",
            "snapshot-semantic-approver-user",
        )
        .await;
        let revision = source
            .promotions
            .create_semantic_repair_revision(
                TENANT,
                INCARNATION,
                author.actor.clone(),
                "snapshot-semantic-revision",
                signed_revision_request(&author, candidate(), None),
            )
            .await
            .unwrap()
            .record;
        let review = source
            .promotions
            .review_semantic_repair_revision(
                TENANT,
                INCARNATION,
                approver.actor.clone(),
                "snapshot-semantic-review",
                signed_review_request(&approver, &revision, SemanticRepairReviewDecision::Approve),
            )
            .await
            .unwrap()
            .record;
        let valid_snapshot = source_raw.export_snapshot().await.unwrap();

        let restored_raw: Arc<dyn GraphBackend> = Arc::new(NativeBackend::new());
        let restored = AppState::new_shared(restored_raw.clone());
        restored
            .promotions
            .configure_governance_root(Some(&root.public_key))
            .unwrap();
        restored
            .promotions
            .import_snapshot(TENANT, INCARNATION, &valid_snapshot)
            .await
            .unwrap();
        assert_eq!(
            restored
                .promotions
                .get_semantic_repair_review(TENANT, INCARNATION, &review.semantic_repair_review_id,)
                .await
                .unwrap(),
            review
        );

        let marker = "m25_tampered_snapshot_must_not_apply";
        let mut tampered = valid_snapshot.clone();
        tampered["collections"][SEMANTIC_REPAIR_REVIEWS_COLLECTION]["documents"][&review.key]["decision"] =
            json!("reject");
        tampered["collections"].as_object_mut().unwrap().insert(
            marker.into(),
            json!({
                "type": "document",
                "documents": {"marker": {"_key": "marker", "value": 1}}
            }),
        );
        let tampered_raw: Arc<dyn GraphBackend> = Arc::new(NativeBackend::new());
        let tampered_target = AppState::new_shared(tampered_raw.clone());
        tampered_target
            .promotions
            .configure_governance_root(Some(&root.public_key))
            .unwrap();
        assert!(
            tampered_target
                .promotions
                .import_snapshot(TENANT, INCARNATION, &tampered)
                .await
                .is_err()
        );
        assert_eq!(
            tampered_raw.get_document(marker, "marker").await.unwrap(),
            None
        );

        let union_marker = "m25_divergent_union_must_not_apply";
        let mut divergent = valid_snapshot;
        divergent["collections"][SEMANTIC_REPAIR_REVIEWS_COLLECTION]["documents"][&review.key]["reason"] =
            json!("divergent immutable authority");
        divergent["collections"].as_object_mut().unwrap().insert(
            union_marker.into(),
            json!({
                "type": "document",
                "documents": {"marker": {"_key": "marker", "value": 1}}
            }),
        );
        assert!(
            restored
                .promotions
                .import_snapshot(TENANT, INCARNATION, &divergent)
                .await
                .is_err()
        );
        assert_eq!(
            restored_raw
                .get_document(union_marker, "marker")
                .await
                .unwrap(),
            None
        );

        // Key-only pages must still fetch and deserialize the complete stored
        // record; projecting known fields would hide direct-store additions
        // from `deny_unknown_fields` and turn malformed authority into a valid
        // list summary.
        let mut unknown_field_revision = serde_json::to_value(&revision).unwrap();
        unknown_field_revision["unexpected_direct_store_field"] = json!("tamper");
        source_raw
            .replace_document(
                SEMANTIC_REPAIR_REVISIONS_COLLECTION,
                &revision.key,
                unknown_field_revision,
            )
            .await
            .unwrap();
        assert!(
            source
                .promotions
                .list_semantic_repair_revisions(
                    TENANT,
                    INCARNATION,
                    MAX_SEMANTIC_REPAIR_REVISION_PAGE_SIZE,
                    None,
                )
                .await
                .is_err()
        );

        source.jobs.shutdown().await;
        restored.jobs.shutdown().await;
        tampered_target.jobs.shutdown().await;
    }
}
