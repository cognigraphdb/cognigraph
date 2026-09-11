//! Generation boundaries.

use super::*;

pub(super) fn ensure_actionable_capacity(decisions_scanned: usize) -> Result<(), CogniGraphError> {
    if decisions_scanned >= MAX_RECONCILE_DECISIONS {
        return Err(conflict(format!(
            "promotion target has reached its {MAX_RECONCILE_DECISIONS}-decision lifecycle limit; use a new target channel"
        )));
    }
    Ok(())
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AuthorityGeneration {
    M18,
    M19,
    M20,
    M21,
    M22,
    M23,
}
pub(super) fn context_generation(
    context: &PromotionContext,
) -> Result<AuthorityGeneration, CogniGraphError> {
    match context.schema_version {
        PROMOTION_CONTEXT_SCHEMA_VERSION => Ok(AuthorityGeneration::M18),
        M19_PROMOTION_CONTEXT_SCHEMA_VERSION => Ok(AuthorityGeneration::M19),
        M20_PROMOTION_CONTEXT_SCHEMA_VERSION => Ok(AuthorityGeneration::M20),
        M21_PROMOTION_CONTEXT_SCHEMA_VERSION => Ok(AuthorityGeneration::M21),
        M22_PROMOTION_CONTEXT_SCHEMA_VERSION => Ok(AuthorityGeneration::M22),
        M23_PROMOTION_CONTEXT_SCHEMA_VERSION => Ok(AuthorityGeneration::M23),
        version => Err(conflict(format!(
            "unsupported promotion context authority generation {version}"
        ))),
    }
}
pub(super) fn evidence_generation(
    evidence: &PromotionEvidence,
) -> Result<AuthorityGeneration, CogniGraphError> {
    match evidence.schema_version {
        PROMOTION_EVIDENCE_SCHEMA_VERSION => Ok(AuthorityGeneration::M18),
        M19_PROMOTION_EVIDENCE_SCHEMA_VERSION => Ok(AuthorityGeneration::M19),
        M20_PROMOTION_EVIDENCE_SCHEMA_VERSION => Ok(AuthorityGeneration::M20),
        M21_PROMOTION_EVIDENCE_SCHEMA_VERSION => Ok(AuthorityGeneration::M21),
        M22_PROMOTION_EVIDENCE_SCHEMA_VERSION => Ok(AuthorityGeneration::M22),
        M23_PROMOTION_EVIDENCE_SCHEMA_VERSION => Ok(AuthorityGeneration::M23),
        version => Err(conflict(format!(
            "unsupported promotion evidence authority generation {version}"
        ))),
    }
}
pub(super) fn ensure_fresh_target_boundary(
    candidate: AuthorityGeneration,
    predecessor: AuthorityGeneration,
) -> Result<(), CogniGraphError> {
    if candidate != predecessor {
        return Err(conflict(
            "promotion authority generations require separate target channels",
        ));
    }
    Ok(())
}
pub(super) fn ensure_target_evidence_generation(
    candidate: AuthorityGeneration,
    existing_generations: impl IntoIterator<Item = AuthorityGeneration>,
) -> Result<(), CogniGraphError> {
    if existing_generations
        .into_iter()
        .any(|generation| generation != candidate)
    {
        return Err(conflict(
            "M18 through M23 promotion evidence cannot share a target channel; use a fresh target",
        ));
    }
    Ok(())
}
pub(super) fn validate_evidence_generation_boundaries<'a>(
    evidence: impl IntoIterator<Item = &'a PromotionEvidence>,
) -> Result<(), CogniGraphError> {
    let mut generations = BTreeMap::<(String, String), AuthorityGeneration>::new();
    for record in evidence {
        let key = (
            record.target.space_type.clone(),
            record.target.channel.clone(),
        );
        let generation = evidence_generation(record)?;
        if let Some(existing) = generations.insert(key, generation)
            && existing != generation
        {
            return Err(conflict(
                "M18 through M23 promotion evidence share a target channel",
            ));
        }
    }
    Ok(())
}
pub(super) fn validate_authority_generation_order(
    governed_decisions: impl IntoIterator<Item = bool>,
) -> Result<(), CogniGraphError> {
    let mut signed_authority_seen = false;
    for governed in governed_decisions {
        if governed {
            signed_authority_seen = true;
        } else if signed_authority_seen {
            return Err(conflict(
                "legacy unsigned promotion decision follows signed governance authority",
            ));
        }
    }
    Ok(())
}
