//! Authority validation.

use super::*;

impl PromotionManager {
    pub(super) fn validate_governance_authority(
        &self,
        tenant: &str,
        incarnation: &str,
        authority: &GovernanceAuthority,
    ) -> Result<(), CogniGraphError> {
        if authority.keys.len() > MAX_KEYS_PER_TENANT
            || authority.revocations.len() > MAX_KEYS_PER_TENANT
        {
            return Err(conflict("governance key capacity exceeded"));
        }
        if authority.policies.len() > MAX_POLICY_REVISIONS_PER_TENANT
            || authority.approvals.len() > MAX_POLICY_REVISIONS_PER_TENANT
        {
            return Err(conflict("policy authority capacity exceeded"));
        }
        if authority.artifacts.len() > MAX_ARTIFACT_ATTESTATIONS_PER_TENANT {
            return Err(conflict("artifact attestation capacity exceeded"));
        }
        if authority.semantic_repair_revisions.len() > MAX_SEMANTIC_REPAIR_REVISIONS_PER_TENANT
            || authority.semantic_repair_reviews.len() > MAX_SEMANTIC_REPAIR_REVIEWS_PER_TENANT
        {
            return Err(conflict("semantic repair authority capacity exceeded"));
        }
        let mut principal_purposes = BTreeMap::<String, KeyPurpose>::new();
        let mut key_ids = HashSet::new();
        let mut idempotency_hashes = HashSet::new();
        for key in authority.keys.values() {
            self.validate_key_record(key, tenant, incarnation)?;
            if !key_ids.insert(key.verification_key.key_id.clone()) {
                return Err(conflict("duplicate governance key id"));
            }
            if let Some(existing) =
                principal_purposes.insert(key.principal_id.clone(), key.verification_key.purpose)
                && existing != key.verification_key.purpose
            {
                return Err(conflict("governance principal crosses duties"));
            }
            if !idempotency_hashes.insert(key.idempotency_key_hash.clone()) {
                return Err(conflict("governance records reuse an idempotency-key hash"));
            }
        }
        for revocation in authority.revocations.values() {
            self.validate_revocation_record(revocation, tenant, incarnation)?;
            if !idempotency_hashes.insert(revocation.idempotency_key_hash.clone()) {
                return Err(conflict("governance records reuse an idempotency-key hash"));
            }
            let key = authority
                .keys
                .get(&revocation.registration_id)
                .ok_or_else(|| conflict("governance revocation references a missing key"))?;
            if key.verification_key.key_id != revocation.key_id
                || key.public_key_digest != revocation.public_key_digest
                || revocation.recorded_at_ms < key.registered_at_ms
            {
                return Err(conflict(
                    "governance revocation references another key or predates its registration",
                ));
            }
        }
        for policy in authority.policies.values() {
            let author = authority
                .keys
                .get(&policy.author_registration_id)
                .ok_or_else(|| conflict("signed policy references a missing author key"))?;
            self.validate_policy_record_against(policy, author, tenant, incarnation)?;
            validate_historical_key_use(
                author,
                KeyPurpose::PolicyAuthor,
                policy.signed_at_ms,
                policy.created_at_ms,
                authority.revocations.get(&author.registration_id),
            )?;
            if !idempotency_hashes.insert(policy.idempotency_key_hash.clone()) {
                return Err(conflict("governance records reuse an idempotency-key hash"));
            }
        }
        for approval in authority.approvals.values() {
            let policy = authority
                .policies
                .get(&approval.policy_revision_id)
                .ok_or_else(|| conflict("signed approval references a missing policy"))?;
            let author = authority
                .keys
                .get(&policy.author_registration_id)
                .ok_or_else(|| conflict("signed approval references a missing author key"))?;
            let approver = authority
                .keys
                .get(&approval.approver_registration_id)
                .ok_or_else(|| conflict("signed approval references a missing approver key"))?;
            self.validate_approval_record_against(approval, policy, approver, tenant, incarnation)?;
            validate_historical_key_use(
                author,
                KeyPurpose::PolicyAuthor,
                approval.approved_at_ms,
                approval.approved_at_ms,
                authority.revocations.get(&author.registration_id),
            )?;
            validate_historical_key_use(
                approver,
                KeyPurpose::PolicyApprover,
                approval.signed_at_ms,
                approval.approved_at_ms,
                authority.revocations.get(&approver.registration_id),
            )?;
            if !idempotency_hashes.insert(approval.idempotency_key_hash.clone()) {
                return Err(conflict("governance records reuse an idempotency-key hash"));
            }
        }
        ensure_semantic_repair_candidate_capacity(
            authority
                .semantic_repair_revisions
                .values()
                .map(|revision| canonical_json_bytes(&revision.candidate).map(|bytes| bytes.len())),
        )?;
        for revision in authority.semantic_repair_revisions.values() {
            let author = authority
                .keys
                .get(&revision.author_registration_id)
                .ok_or_else(|| {
                    conflict("semantic repair revision references a missing author key")
                })?;
            self.validate_semantic_repair_revision_record_against(
                revision,
                author,
                tenant,
                incarnation,
            )?;
            validate_historical_key_use(
                author,
                KeyPurpose::PolicyAuthor,
                revision.signed_at_ms,
                revision.created_at_ms,
                authority.revocations.get(&author.registration_id),
            )?;
            if !idempotency_hashes.insert(revision.idempotency_key_hash.clone()) {
                return Err(conflict("governance records reuse an idempotency-key hash"));
            }
        }
        for review in authority.semantic_repair_reviews.values() {
            let revision = authority
                .semantic_repair_revisions
                .get(&review.semantic_repair_revision_id)
                .ok_or_else(|| conflict("semantic repair review references a missing revision"))?;
            let approver = authority
                .keys
                .get(&review.approver_registration_id)
                .ok_or_else(|| {
                    conflict("semantic repair review references a missing approver key")
                })?;
            let author = authority
                .keys
                .get(&revision.author_registration_id)
                .ok_or_else(|| {
                    conflict("semantic repair review references a missing author key")
                })?;
            self.validate_semantic_repair_review_record_against(
                review,
                revision,
                approver,
                tenant,
                incarnation,
            )?;
            validate_historical_key_use(
                author,
                KeyPurpose::PolicyAuthor,
                review.reviewed_at_ms,
                review.reviewed_at_ms,
                authority.revocations.get(&author.registration_id),
            )?;
            validate_historical_key_use(
                approver,
                KeyPurpose::PolicyApprover,
                review.signed_at_ms,
                review.reviewed_at_ms,
                authority.revocations.get(&approver.registration_id),
            )?;
            if !idempotency_hashes.insert(review.idempotency_key_hash.clone()) {
                return Err(conflict("governance records reuse an idempotency-key hash"));
            }
        }
        let mut artifact_manifest_bytes = 0_usize;
        for artifact in authority.artifacts.values() {
            artifact_manifest_bytes = artifact_manifest_bytes
                .checked_add(artifact.manifest.canonical_size_bytes()?)
                .ok_or_else(|| conflict("artifact manifest storage accounting overflowed"))?;
            if artifact_manifest_bytes > MAX_ARTIFACT_MANIFEST_BYTES_PER_TENANT {
                return Err(conflict("artifact manifest storage capacity exceeded"));
            }
            let attestor = authority
                .keys
                .get(&artifact.attestor_registration_id)
                .ok_or_else(|| {
                    conflict("artifact attestation references a missing attestor key")
                })?;
            self.validate_artifact_record_against(artifact, attestor, tenant, incarnation)?;
            validate_historical_key_use(
                attestor,
                KeyPurpose::ArtifactAttestor,
                artifact.signed_at_ms,
                artifact.accepted_at_ms,
                authority.revocations.get(&attestor.registration_id),
            )?;
            if !idempotency_hashes.insert(artifact.idempotency_key_hash.clone()) {
                return Err(conflict("governance records reuse an idempotency-key hash"));
            }
        }
        Ok(())
    }
}
