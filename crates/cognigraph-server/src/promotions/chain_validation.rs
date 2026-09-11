//! Chain validation.

use super::*;

impl PromotionManager {
    pub(super) fn validate_decision_set(
        &self,
        tenant: &str,
        incarnation: &str,
        evidence: &BTreeMap<String, PromotionEvidence>,
        decisions: &BTreeMap<String, PromotionDecision>,
    ) -> Result<(), CogniGraphError> {
        for record in evidence.values() {
            self.validate_evidence(record, tenant, incarnation)?;
        }
        validate_evidence_generation_boundaries(evidence.values())?;

        let mut chains = BTreeMap::<(String, String), Vec<&PromotionDecision>>::new();
        for decision in decisions.values() {
            self.validate_decision(decision, tenant, incarnation)?;
            let source = evidence
                .get(&decision.evidence_id)
                .ok_or_else(|| conflict("promotion decision references missing evidence"))?;
            let expected_decision_schema = match evidence_generation(source)? {
                AuthorityGeneration::M23 => M23_PROMOTION_DECISION_SCHEMA_VERSION,
                AuthorityGeneration::M22 => M22_PROMOTION_DECISION_SCHEMA_VERSION,
                AuthorityGeneration::M21 => M21_PROMOTION_DECISION_SCHEMA_VERSION,
                AuthorityGeneration::M20 => M20_PROMOTION_DECISION_SCHEMA_VERSION,
                AuthorityGeneration::M19 => M19_PROMOTION_DECISION_SCHEMA_VERSION,
                AuthorityGeneration::M18
                    if decision.governance.is_some()
                        && decision.action == PromotionAction::Rollback =>
                {
                    M19_PROMOTION_DECISION_SCHEMA_VERSION
                }
                AuthorityGeneration::M18 => PROMOTION_DECISION_SCHEMA_VERSION,
            };
            if decision.schema_version != expected_decision_schema {
                return Err(conflict(
                    "promotion decision schema does not match its evidence authority generation",
                ));
            }
            let gate_assessment_digest = canonical_digest(&source.gates)?;
            if decision.target != source.target
                || decision.evidence_digest != source.evidence_digest
                || decision.policy_digest != source.policy_digest
                || decision.gate_assessment_digest != gate_assessment_digest
            {
                return Err(conflict(
                    "promotion decision does not match its immutable evidence",
                ));
            }
            match (&source.governance, &decision.governance) {
                (Some(binding), Some(authorization)) => {
                    let signed = &authorization.statement.payload;
                    if signed.policy_revision_id.as_deref()
                        != Some(binding.policy_revision_id.as_str())
                        || signed.policy_revision_digest.as_deref()
                            != Some(binding.policy_revision_digest.as_str())
                        || signed.approval_id.as_deref() != Some(binding.approval_id.as_str())
                        || signed.approval_digest.as_deref()
                            != Some(binding.approval_digest.as_str())
                        || signed.artifact_authority_digest.as_deref()
                            != source
                                .artifact_attestations
                                .as_ref()
                                .map(|authority| authority.authority_digest.as_str())
                        || signed.consumption_authority_digest.as_deref()
                            != source
                                .artifact_consumption
                                .as_ref()
                                .map(|authority| authority.authority_digest.as_str())
                        || signed.derivation_authority_digest.as_deref()
                            != source
                                .artifact_consumption
                                .as_ref()
                                .and_then(|authority| authority.derivation.as_ref())
                                .map(|authority| authority.authority_digest.as_str())
                        || signed.preparation_authority_digest.as_deref()
                            != source
                                .artifact_consumption
                                .as_ref()
                                .and_then(|authority| authority.derivation.as_ref())
                                .and_then(|authority| authority.preparation.as_ref())
                                .map(|authority| authority.authority_digest.as_str())
                    {
                        return Err(conflict(
                            "signed promotion decision does not bind its evidence policy approval",
                        ));
                    }
                    if source
                        .artifact_attestations
                        .as_ref()
                        .is_some_and(|authority| {
                            authority
                                .candidate
                                .attestor_principals()
                                .contains(signed.promoter_principal_id.as_str())
                                || authority
                                    .baseline
                                    .attestor_principals()
                                    .contains(signed.promoter_principal_id.as_str())
                        })
                    {
                        return Err(conflict(
                            "stored promotion violates artifact-attestor/promoter separation",
                        ));
                    }
                    let requested_action = match decision.action {
                        PromotionAction::Promote | PromotionAction::Blocked => "promote",
                        PromotionAction::Reject => "reject",
                        PromotionAction::Rollback => "rollback",
                    };
                    let expected = crate::governance::PromotionIntentPayload {
                        requested_action: requested_action.into(),
                        target: source.target.clone(),
                        evidence_id: source.id.clone(),
                        evidence_digest: source.evidence_digest.clone(),
                        policy_revision_id: Some(binding.policy_revision_id.clone()),
                        policy_revision_digest: Some(binding.policy_revision_digest.clone()),
                        approval_id: Some(binding.approval_id.clone()),
                        approval_digest: Some(binding.approval_digest.clone()),
                        artifact_authority_digest: source
                            .artifact_attestations
                            .as_ref()
                            .map(|authority| authority.authority_digest.clone()),
                        consumption_authority_digest: source
                            .artifact_consumption
                            .as_ref()
                            .map(|authority| authority.authority_digest.clone()),
                        derivation_authority_digest: source
                            .artifact_consumption
                            .as_ref()
                            .and_then(|authority| authority.derivation.as_ref())
                            .map(|authority| authority.authority_digest.clone()),
                        preparation_authority_digest: source
                            .artifact_consumption
                            .as_ref()
                            .and_then(|authority| authority.derivation.as_ref())
                            .and_then(|authority| authority.preparation.as_ref())
                            .map(|authority| authority.authority_digest.clone()),
                        gate_assessment_digest: gate_assessment_digest.clone(),
                        expected_head_decision_id: decision.expected_head_decision_id.clone(),
                        rollback_target_evidence_id: match decision.action {
                            PromotionAction::Promote | PromotionAction::Blocked => {
                                source.rollback_target_evidence_id.clone()
                            }
                            PromotionAction::Rollback => Some(source.id.clone()),
                            PromotionAction::Reject => None,
                        },
                        reason: decision.reason.clone(),
                        idempotency_key_hash: decision.idempotency_key_hash.clone(),
                        promoter_registration_id: signed.promoter_registration_id.clone(),
                        promoter_principal_id: signed.promoter_principal_id.clone(),
                        signed_at_ms: signed.signed_at_ms,
                    };
                    self.validate_signed_promotion_intent(
                        tenant,
                        incarnation,
                        &decision.actor,
                        &expected,
                        authorization,
                        Some(binding),
                        decision.created_at_ms,
                    )?;
                }
                (None, Some(authorization)) if decision.action == PromotionAction::Rollback => {
                    let signed = &authorization.statement.payload;
                    if signed.policy_revision_id.is_some()
                        || signed.policy_revision_digest.is_some()
                        || signed.approval_id.is_some()
                        || signed.approval_digest.is_some()
                        || signed.artifact_authority_digest.is_some()
                        || signed.consumption_authority_digest.is_some()
                        || signed.derivation_authority_digest.is_some()
                        || signed.preparation_authority_digest.is_some()
                    {
                        return Err(conflict(
                            "signed legacy rollback unexpectedly names later governance authority",
                        ));
                    }
                }
                (None, None) => {}
                _ => {
                    return Err(conflict(
                        "promotion evidence and decision governance generations disagree",
                    ));
                }
            }
            match decision.action {
                PromotionAction::Promote if !source.gates.overall_passed => {
                    return Err(conflict("promotion decision bypasses failed gates"));
                }
                PromotionAction::Blocked if source.gates.overall_passed => {
                    return Err(conflict("blocked decision references passing evidence"));
                }
                PromotionAction::Rollback if !source.gates.overall_passed => {
                    return Err(conflict("rollback target evidence did not pass its gates"));
                }
                PromotionAction::Reject if decision.expected_head_decision_id.is_some() => {
                    return Err(conflict("rejection unexpectedly carries a head CAS value"));
                }
                _ => {}
            }
            if matches!(
                decision.action,
                PromotionAction::Promote | PromotionAction::Blocked | PromotionAction::Rollback
            ) && decision.expected_head_decision_id != decision.predecessor_decision_id
            {
                return Err(conflict(
                    "promotion decision expected head and predecessor disagree",
                ));
            }
            if let Some(selection) = &decision.resulting_selection {
                if selection.evidence_id != source.id
                    || selection.evidence_digest != source.evidence_digest
                    || selection.candidate_digest != source.candidate_digest
                    || selection.policy_digest != source.policy_digest
                {
                    return Err(conflict(
                        "promotion decision selection does not match its evidence",
                    ));
                }
                chains
                    .entry((
                        decision.target.space_type.clone(),
                        decision.target.channel.clone(),
                    ))
                    .or_default()
                    .push(decision);
            }
        }

        let selections_by_decision = decisions
            .values()
            .filter_map(|decision| {
                decision
                    .resulting_selection
                    .as_ref()
                    .map(|selection| (decision.id.as_str(), (&decision.target, selection)))
            })
            .collect::<BTreeMap<_, _>>();
        for source in evidence.values() {
            let Some(expected_head_id) = source.expected_head_decision_id.as_deref() else {
                continue;
            };
            let (predecessor_target, predecessor_selection) = selections_by_decision
                .get(expected_head_id)
                .copied()
                .ok_or_else(|| {
                    conflict("promotion evidence expected head is missing or non-actionable")
                })?;
            let predecessor_evidence = evidence
                .get(&predecessor_selection.evidence_id)
                .ok_or_else(|| conflict("promotion evidence rollback target is missing"))?;
            ensure_fresh_target_boundary(
                evidence_generation(source)?,
                evidence_generation(predecessor_evidence)?,
            )?;
            if *predecessor_target != source.target
                || source.rollback_target_evidence_id.as_deref()
                    != Some(predecessor_selection.evidence_id.as_str())
                || !same_construction_identity(
                    &source.runs[2].source.context,
                    &predecessor_evidence.runs[0].source.context,
                )
            {
                return Err(conflict(
                    "promotion evidence is not bound to its expected predecessor",
                ));
            }
        }
        for decision in decisions.values() {
            let source = evidence
                .get(&decision.evidence_id)
                .expect("decision evidence binding was validated above");
            match decision.action {
                PromotionAction::Promote | PromotionAction::Blocked => {
                    let predecessor = decision
                        .predecessor_decision_id
                        .as_deref()
                        .map(|id| {
                            selections_by_decision.get(id).copied().ok_or_else(|| {
                                conflict("promotion decision predecessor is missing")
                            })
                        })
                        .transpose()?;
                    if predecessor
                        .as_ref()
                        .is_some_and(|(target, _)| **target != decision.target)
                    {
                        return Err(conflict(
                            "promotion decision predecessor belongs to another target",
                        ));
                    }
                    let prior_evidence_id =
                        predecessor.map(|(_, selection)| selection.evidence_id.as_str());
                    let baseline_matches_predecessor = predecessor
                        .map(|(_, selection)| {
                            let selected =
                                evidence.get(&selection.evidence_id).ok_or_else(|| {
                                    conflict("promotion predecessor evidence is missing")
                                })?;
                            Ok::<bool, CogniGraphError>(same_construction_identity(
                                &source.runs[2].source.context,
                                &selected.runs[0].source.context,
                            ))
                        })
                        .transpose()?
                        .unwrap_or(true);
                    if source.expected_head_decision_id.as_deref()
                        != decision.predecessor_decision_id.as_deref()
                        || source.rollback_target_evidence_id.as_deref() != prior_evidence_id
                        || !baseline_matches_predecessor
                    {
                        return Err(conflict(
                            "promotion evidence was not registered against its decision predecessor",
                        ));
                    }
                }
                PromotionAction::Rollback => {
                    let predecessor_id = decision
                        .predecessor_decision_id
                        .as_deref()
                        .ok_or_else(|| conflict("rollback decision has no predecessor"))?;
                    let (predecessor_target, predecessor_selection) = selections_by_decision
                        .get(predecessor_id)
                        .copied()
                        .ok_or_else(|| conflict("rollback decision predecessor is missing"))?;
                    if *predecessor_target != decision.target
                        || predecessor_selection.prior_evidence_id.as_deref()
                            != Some(decision.evidence_id.as_str())
                    {
                        return Err(conflict(
                            "rollback decision does not select its predecessor's explicit prior evidence",
                        ));
                    }
                }
                PromotionAction::Reject => {}
            }
        }

        for chain in chains.values_mut() {
            if chain.len() > MAX_RECONCILE_DECISIONS {
                return Err(validation(format!(
                    "promotion target reconciliation exceeds {MAX_RECONCILE_DECISIONS} decisions"
                )));
            }
            chain.sort_by_key(|decision| {
                (
                    decision
                        .resulting_selection
                        .as_ref()
                        .map(|selection| selection.generation)
                        .unwrap_or_default(),
                    decision.id.as_str(),
                )
            });
            validate_authority_generation_order(
                chain.iter().map(|decision| decision.governance.is_some()),
            )?;
            let mut expected_generation = 1u64;
            let mut predecessor_decision_id: Option<&str> = None;
            let mut prior_evidence_id: Option<&str> = None;
            for decision in chain {
                let selection = decision
                    .resulting_selection
                    .as_ref()
                    .expect("actionable decision has a selection");
                if selection.generation != expected_generation
                    || decision.predecessor_decision_id.as_deref() != predecessor_decision_id
                    || selection.prior_evidence_id.as_deref() != prior_evidence_id
                {
                    return Err(conflict(
                        "promotion decision chain is forked or non-contiguous",
                    ));
                }
                predecessor_decision_id = Some(&decision.id);
                prior_evidence_id = Some(&selection.evidence_id);
                expected_generation = expected_generation
                    .checked_add(1)
                    .ok_or_else(|| conflict("promotion generation is exhausted"))?;
            }
        }
        Ok(())
    }
}
