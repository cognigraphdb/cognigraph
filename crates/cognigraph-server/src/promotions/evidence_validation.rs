//! Evidence validation.

use super::*;

impl PromotionManager {
    pub(super) async fn validate_stored_evidence_provenance(
        &self,
        tenant: &str,
        incarnation: &str,
        evidence: &BTreeMap<String, PromotionEvidence>,
    ) -> Result<(), CogniGraphError> {
        for record in evidence.values() {
            for run in &record.runs {
                let authority = self
                    .authoritative_evaluation_source(tenant, incarnation, &run.source.job_id)
                    .await?;
                if authority != run.source {
                    return Err(conflict(format!(
                        "promotion evidence source job `{}` does not match authoritative stored job data",
                        run.source.job_id
                    )));
                }
            }
        }
        Ok(())
    }

    pub(super) fn validate_evidence(
        &self,
        record: &PromotionEvidence,
        tenant: &str,
        incarnation: &str,
    ) -> Result<(), CogniGraphError> {
        if !matches!(
            record.schema_version,
            PROMOTION_EVIDENCE_SCHEMA_VERSION
                | M19_PROMOTION_EVIDENCE_SCHEMA_VERSION
                | M20_PROMOTION_EVIDENCE_SCHEMA_VERSION
                | M21_PROMOTION_EVIDENCE_SCHEMA_VERSION
                | M22_PROMOTION_EVIDENCE_SCHEMA_VERSION
                | M23_PROMOTION_EVIDENCE_SCHEMA_VERSION
        ) || record.digest_algorithm != DIGEST_ALGORITHM
            || record.tenant != tenant
            || record.tenant_incarnation != incarnation
            || record.key != scoped_key(tenant, incarnation, "e", &record.id)
        {
            return Err(conflict("malformed or foreign promotion evidence record"));
        }
        validate_record_id("evidence id", &record.id)?;
        validate_digest("idempotency_key_hash", &record.idempotency_key_hash)?;
        if record.id
            != new_record_id(
                tenant,
                incarnation,
                "evidence",
                &record.idempotency_key_hash,
            )
        {
            return Err(conflict(
                "promotion evidence id does not match its idempotency identity",
            ));
        }
        if record.created_at_ms == 0 {
            return Err(conflict("promotion evidence timestamp is zero"));
        }
        if record.expected_head_decision_id.is_some()
            != record.rollback_target_evidence_id.is_some()
        {
            return Err(conflict(
                "promotion evidence head CAS and rollback target disagree",
            ));
        }
        if let Some(id) = &record.expected_head_decision_id {
            validate_record_id("expected_head_decision_id", id)?;
        }
        if let Some(id) = &record.rollback_target_evidence_id {
            validate_record_id("rollback_target_evidence_id", id)?;
        }
        validate_digest("evidence_digest", &record.evidence_digest)?;
        validate_digest("request_digest", &record.request_digest)?;
        validate_digest("policy_digest", &record.policy_digest)?;
        match (
            record.schema_version,
            &record.governance,
            &record.artifact_attestations,
            &record.artifact_consumption,
        ) {
            (PROMOTION_EVIDENCE_SCHEMA_VERSION, None, None, None) => {
                record.created_by.require_role("admin")?
            }
            (M19_PROMOTION_EVIDENCE_SCHEMA_VERSION, Some(binding), None, None) => {
                binding.validate()?;
                record.created_by.require_role("promoter")?;
            }
            (M20_PROMOTION_EVIDENCE_SCHEMA_VERSION, Some(binding), Some(artifacts), None) => {
                binding.validate()?;
                artifacts.validate()?;
                record.created_by.require_role("promoter")?;
            }
            (
                M21_PROMOTION_EVIDENCE_SCHEMA_VERSION,
                Some(binding),
                Some(artifacts),
                Some(consumption),
            ) => {
                binding.validate()?;
                artifacts.validate()?;
                consumption.validate()?;
                if consumption.derivation.is_some() {
                    return Err(conflict(
                        "M21 promotion evidence cannot carry M22 derivation authority",
                    ));
                }
                record.created_by.require_role("promoter")?;
            }
            (
                M22_PROMOTION_EVIDENCE_SCHEMA_VERSION,
                Some(binding),
                Some(artifacts),
                Some(consumption),
            ) => {
                binding.validate()?;
                artifacts.validate()?;
                consumption.validate()?;
                if consumption.derivation.is_none() {
                    return Err(conflict(
                        "M22 promotion evidence requires derivation authority",
                    ));
                }
                if consumption
                    .derivation
                    .as_ref()
                    .is_some_and(|authority| authority.preparation.is_some())
                {
                    return Err(conflict(
                        "M22 promotion evidence cannot carry M23 preparation authority",
                    ));
                }
                record.created_by.require_role("promoter")?;
            }
            (
                M23_PROMOTION_EVIDENCE_SCHEMA_VERSION,
                Some(binding),
                Some(artifacts),
                Some(consumption),
            ) => {
                binding.validate()?;
                artifacts.validate()?;
                consumption.validate()?;
                if !consumption
                    .derivation
                    .as_ref()
                    .is_some_and(|authority| authority.preparation.is_some())
                {
                    return Err(conflict(
                        "M23 promotion evidence requires derivation and preparation authority",
                    ));
                }
                record.created_by.require_role("promoter")?;
            }
            _ => {
                return Err(conflict(
                    "promotion evidence schema and governance/artifact bindings disagree",
                ));
            }
        }
        if let (Some(binding), Some(artifacts)) =
            (&record.governance, &record.artifact_attestations)
            && artifact_authority_conflicts_with_policy(artifacts, binding)
        {
            return Err(conflict(
                "stored candidate or baseline artifact attestor crosses policy-author or approver duties",
            ));
        }
        record.target.validate()?;
        if record.runs.len() != 4
            || record.runs[0].role != EvidenceRunRole::CandidateOriginal
            || record.runs[1].role != EvidenceRunRole::CandidateReplay
            || record.runs[2].role != EvidenceRunRole::BaselineOriginal
            || record.runs[3].role != EvidenceRunRole::BaselineReplay
        {
            return Err(conflict("promotion evidence has a malformed run ordering"));
        }
        if record.runs.iter().any(|run| {
            run.source
                .artifact_consumption
                .as_ref()
                .is_some_and(|receipt| {
                    receipt.tenant != tenant || receipt.tenant_incarnation != incarnation
                })
        }) {
            return Err(conflict(
                "promotion evidence contains a foreign artifact consumption receipt",
            ));
        }
        let assessed = assess_evidence_runs(
            &record.runs[0].source,
            &record.runs[1].source,
            &record.runs[2].source,
            &record.runs[3].source,
        )?;
        let reconstructed_request = RegisterEvidenceRequest {
            candidate_original_job_id: record.runs[0].source.job_id.clone(),
            candidate_replay_job_id: record.runs[1].source.job_id.clone(),
            baseline_original_job_id: record.runs[2].source.job_id.clone(),
            baseline_replay_job_id: record.runs[3].source.job_id.clone(),
            expected_head_decision_id: record.expected_head_decision_id.clone(),
        };
        reconstructed_request.validate()?;
        let expected_request_digest = canonical_digest(&reconstructed_request)?;
        let expected_artifact_authority = match (
            &record.runs[0].source.context.artifact_attestations,
            &record.runs[2].source.context.artifact_attestations,
        ) {
            (Some(candidate), Some(baseline)) => {
                let mut authority = EvidenceArtifactAuthority {
                    candidate: candidate.clone(),
                    baseline: baseline.clone(),
                    authority_digest: String::new(),
                };
                authority.authority_digest = record_digest(&authority, "authority_digest")?;
                Some(authority)
            }
            (None, None) => None,
            _ => return Err(conflict("promotion evidence artifact generations differ")),
        };
        let expected_consumption_authority = match (
            &record.runs[0].source.artifact_consumption,
            &record.runs[1].source.artifact_consumption,
            &record.runs[2].source.artifact_consumption,
            &record.runs[3].source.artifact_consumption,
        ) {
            (
                Some(candidate_original),
                Some(candidate_replay),
                Some(baseline_original),
                Some(baseline_replay),
            ) => Some(Box::new(
                crate::artifact_consumption::EvidenceConsumptionAuthority::from_receipts(
                    candidate_original,
                    candidate_replay,
                    baseline_original,
                    baseline_replay,
                )?,
            )),
            (None, None, None, None) => None,
            _ => {
                return Err(conflict(
                    "promotion evidence consumption generations differ",
                ));
            }
        };
        if assessed != record.gates
            || record.target != record.runs[0].source.context.target
            || record.policy != record.runs[0].source.context.policy
            || record.governance != record.runs[0].source.context.governance
            || record.artifact_attestations != expected_artifact_authority
            || record.artifact_consumption != expected_consumption_authority
            || canonical_digest(&record.policy)? != record.policy_digest
            || record.candidate_digest != record.runs[0].source.context.candidate.candidate_digest
            || record.baseline_candidate_digest
                != record.runs[2].source.context.candidate.candidate_digest
            || record.request_digest != expected_request_digest
            || record_digest(record, "evidence_digest")? != record.evidence_digest
        {
            return Err(conflict(
                "promotion evidence digest or derived fields mismatch",
            ));
        }
        Ok(())
    }
}
