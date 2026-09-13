//! Historical authority.

use super::*;

impl PromotionManager {
    pub(crate) async fn validate_historical_promotion_authority_locked(
        &self,
        tenant: &str,
        incarnation: &str,
        evidence: &BTreeMap<String, PromotionEvidence>,
        decisions: &BTreeMap<String, PromotionDecision>,
    ) -> Result<(), CogniGraphError> {
        self.validate_historical_promotion_authority_with_semantic_target_locked(
            tenant,
            incarnation,
            evidence,
            decisions,
            None,
        )
        .await
    }

    pub(crate) async fn validate_historical_promotion_authority_for_target_locked(
        &self,
        tenant: &str,
        incarnation: &str,
        evidence: &BTreeMap<String, PromotionEvidence>,
        decisions: &BTreeMap<String, PromotionDecision>,
        target: &PromotionTarget,
    ) -> Result<(), CogniGraphError> {
        self.validate_historical_promotion_authority_with_semantic_target_locked(
            tenant,
            incarnation,
            evidence,
            decisions,
            Some(target),
        )
        .await
    }

    pub(crate) async fn validate_historical_promotion_authority_without_semantic_repairs_locked(
        &self,
        tenant: &str,
        incarnation: &str,
        evidence: &BTreeMap<String, PromotionEvidence>,
        decisions: &BTreeMap<String, PromotionDecision>,
    ) -> Result<(), CogniGraphError> {
        let authority = self
            .load_governance_authority_locked(tenant, incarnation)
            .await?;
        self.validate_governance_authority(tenant, incarnation, &authority)?;
        self.validate_promotions_against_authority(evidence, decisions, &authority)
    }

    pub(super) async fn validate_historical_promotion_authority_with_semantic_target_locked(
        &self,
        tenant: &str,
        incarnation: &str,
        evidence: &BTreeMap<String, PromotionEvidence>,
        decisions: &BTreeMap<String, PromotionDecision>,
        target: Option<&PromotionTarget>,
    ) -> Result<(), CogniGraphError> {
        let authority = self
            .load_governance_authority_locked(tenant, incarnation)
            .await?;
        self.validate_governance_authority(tenant, incarnation, &authority)?;
        self.validate_semantic_repair_bases_against_decisions(&authority, decisions, target)?;
        self.validate_promotions_against_authority(evidence, decisions, &authority)
    }

    pub(super) fn validate_semantic_repair_bases_against_decisions(
        &self,
        authority: &GovernanceAuthority,
        decisions: &BTreeMap<String, PromotionDecision>,
        target: Option<&PromotionTarget>,
    ) -> Result<(), CogniGraphError> {
        for revision in authority.semantic_repair_revisions.values() {
            if target.is_some_and(|target| revision.target != *target) {
                continue;
            }
            let actionable = decisions
                .values()
                .filter(|decision| {
                    decision.target == revision.target && decision.resulting_selection.is_some()
                })
                .map(|decision| {
                    (
                        decision.id.as_str(),
                        decision.predecessor_decision_id.as_deref(),
                        decision.created_at_ms,
                    )
                })
                .collect::<Vec<_>>();
            let possible_heads = possible_promotion_heads_at(&actionable, revision.created_at_ms)?;
            if !possible_heads.contains(&revision.base_promotion_head_decision_id) {
                return Err(conflict(
                    "semantic repair revision base was not the current promotion head when accepted",
                ));
            }
        }
        Ok(())
    }

    pub(super) fn validate_promotions_against_authority(
        &self,
        evidence: &BTreeMap<String, PromotionEvidence>,
        decisions: &BTreeMap<String, PromotionDecision>,
        authority: &GovernanceAuthority,
    ) -> Result<(), CogniGraphError> {
        for record in evidence.values() {
            let Some(binding) = &record.governance else {
                continue;
            };
            self.validate_binding_against_authority(
                &record.target,
                &record.policy,
                binding,
                record.created_at_ms,
                authority,
            )?;
            for run in &record.runs {
                let Some(set) = &run.source.context.artifact_attestations else {
                    continue;
                };
                crate::artifact_attestations::validate_context_subject_bindings(
                    &run.source.context,
                    set,
                )?;
                for artifact_binding in set.bindings() {
                    let artifact = authority
                        .artifacts
                        .get(&artifact_binding.attestation_id)
                        .ok_or_else(|| {
                            conflict("promotion evidence references a missing artifact attestation")
                        })?;
                    if artifact.binding() != *artifact_binding
                        || artifact.accepted_at_ms > record.created_at_ms
                    {
                        return Err(conflict(
                            "promotion evidence artifact binding does not match historical authority",
                        ));
                    }
                    let attestor = authority
                        .keys
                        .get(&artifact.attestor_registration_id)
                        .ok_or_else(|| conflict("artifact attestor key is missing"))?;
                    validate_historical_key_use(
                        attestor,
                        KeyPurpose::ArtifactAttestor,
                        record.created_at_ms,
                        record.created_at_ms,
                        authority.revocations.get(&attestor.registration_id),
                    )?;
                }
            }
        }
        for record in decisions.values() {
            let Some(authorization) = &record.governance else {
                continue;
            };
            let source = evidence
                .get(&record.evidence_id)
                .ok_or_else(|| conflict("signed promotion references missing evidence"))?;
            if let Some(binding) = &source.governance {
                self.validate_binding_against_authority(
                    &source.target,
                    &source.policy,
                    binding,
                    record.created_at_ms,
                    authority,
                )?;
            }
            if let Some(artifact_authority) = &source.artifact_attestations {
                for artifact_binding in artifact_authority
                    .candidate
                    .bindings()
                    .into_iter()
                    .chain(artifact_authority.baseline.bindings())
                {
                    let artifact = authority
                        .artifacts
                        .get(&artifact_binding.attestation_id)
                        .ok_or_else(|| {
                            conflict("signed promotion references a missing artifact attestation")
                        })?;
                    if artifact.binding() != *artifact_binding
                        || artifact.accepted_at_ms > record.created_at_ms
                    {
                        return Err(conflict(
                            "signed promotion artifact binding does not match historical authority",
                        ));
                    }
                    let attestor = authority
                        .keys
                        .get(&artifact.attestor_registration_id)
                        .ok_or_else(|| conflict("artifact attestor key is missing"))?;
                    validate_historical_key_use(
                        attestor,
                        KeyPurpose::ArtifactAttestor,
                        record.created_at_ms,
                        record.created_at_ms,
                        authority.revocations.get(&attestor.registration_id),
                    )?;
                }
            }
            let signed = &authorization.statement.payload;
            let promoter = authority
                .keys
                .get(&signed.promoter_registration_id)
                .ok_or_else(|| conflict("signed promotion references a missing promoter key"))?;
            if promoter != &authorization.promoter_registration
                || promoter.principal_id != signed.promoter_principal_id
                || promoter.subject_user_key != record.actor.user_key
            {
                return Err(conflict(
                    "signed promotion embeds a promoter registration other than stored authority",
                ));
            }
            validate_historical_key_use(
                promoter,
                KeyPurpose::Promoter,
                signed.signed_at_ms,
                record.created_at_ms,
                authority.revocations.get(&promoter.registration_id),
            )?;
        }
        Ok(())
    }
}
