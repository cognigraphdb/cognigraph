//! Capacity.

use super::*;

#[derive(Debug, Clone, Copy)]
pub(super) struct GenerationCapacityEntry<'a> {
    pub(super) space_type: &'a str,
    pub(super) idempotency_key_hash: &'a str,
    pub(super) canonical_record_bytes: u64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum GenerationCapacityViolation {
    TenantCount,
    SpaceCount,
    AggregateBytes,
    ByteAccountingOverflow,
    DuplicateIdempotencyHash,
}
pub(super) fn validate_generation_capacity_entries<'a>(
    entries: impl IntoIterator<Item = GenerationCapacityEntry<'a>>,
) -> Result<(), GenerationCapacityViolation> {
    let mut tenant_count = 0usize;
    let mut per_space = BTreeMap::<&str, usize>::new();
    let mut idempotency_hashes = BTreeSet::new();
    let mut bytes = 0u64;
    for entry in entries {
        tenant_count = tenant_count
            .checked_add(1)
            .ok_or(GenerationCapacityViolation::TenantCount)?;
        if tenant_count > MAX_M26_GENERATIONS_PER_TENANT {
            return Err(GenerationCapacityViolation::TenantCount);
        }
        let count = per_space.entry(entry.space_type).or_default();
        *count = count
            .checked_add(1)
            .ok_or(GenerationCapacityViolation::SpaceCount)?;
        if *count > MAX_M26_GENERATIONS_PER_SPACE {
            return Err(GenerationCapacityViolation::SpaceCount);
        }
        if !idempotency_hashes.insert(entry.idempotency_key_hash) {
            return Err(GenerationCapacityViolation::DuplicateIdempotencyHash);
        }
        bytes = bytes
            .checked_add(entry.canonical_record_bytes)
            .ok_or(GenerationCapacityViolation::ByteAccountingOverflow)?;
        if bytes > MAX_M26_GENERATION_BYTES_PER_TENANT as u64 {
            return Err(GenerationCapacityViolation::AggregateBytes);
        }
    }
    Ok(())
}
pub(super) fn validate_generation_capacity_set(
    generations: &BTreeMap<String, SemanticRepairGenerationRecord>,
) -> Result<(), CogniGraphError> {
    let result = validate_generation_capacity_entries(generations.values().map(|generation| {
        GenerationCapacityEntry {
            space_type: &generation.target.space_type,
            idempotency_key_hash: &generation.idempotency_key_hash,
            canonical_record_bytes: generation.canonical_record_bytes,
        }
    }));
    match result {
        Ok(()) => Ok(()),
        Err(GenerationCapacityViolation::TenantCount) => {
            Err(conflict("M26 tenant generation quota is exceeded"))
        }
        Err(GenerationCapacityViolation::SpaceCount) => {
            Err(conflict("M26 per-space generation quota is exceeded"))
        }
        Err(GenerationCapacityViolation::AggregateBytes) => {
            Err(conflict("M26 tenant generation byte quota is exceeded"))
        }
        Err(GenerationCapacityViolation::ByteAccountingOverflow) => {
            Err(conflict("M26 aggregate generation bytes overflowed"))
        }
        Err(GenerationCapacityViolation::DuplicateIdempotencyHash) => Err(conflict(
            "M26 duplicate generation idempotency-key hash is stored",
        )),
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DeploymentChainCapacityViolation {
    SpaceCount,
    DecisionCount,
}
pub(super) fn validate_deployment_chain_capacities<'a>(
    chains: impl IntoIterator<Item = (&'a str, usize)>,
) -> Result<(), DeploymentChainCapacityViolation> {
    let mut space_count = 0usize;
    for (_, decision_count) in chains {
        space_count = space_count
            .checked_add(1)
            .ok_or(DeploymentChainCapacityViolation::SpaceCount)?;
        if space_count > MAX_M26_DEPLOYMENT_SPACES_PER_TENANT {
            return Err(DeploymentChainCapacityViolation::SpaceCount);
        }
        if decision_count > MAX_M26_DEPLOYMENT_DECISIONS_PER_SPACE {
            return Err(DeploymentChainCapacityViolation::DecisionCount);
        }
    }
    Ok(())
}
pub(super) fn validate_deployment_chain_set(
    decisions: impl IntoIterator<Item = SemanticRepairDeploymentDecision>,
) -> Result<BTreeMap<String, Vec<SemanticRepairDeploymentDecision>>, CogniGraphError> {
    let mut chains = BTreeMap::<String, Vec<SemanticRepairDeploymentDecision>>::new();
    for decision in decisions {
        chains
            .entry(decision.space_type.clone())
            .or_default()
            .push(decision);
    }
    validate_deployment_chain_capacities(
        chains
            .iter()
            .map(|(space_type, chain)| (space_type.as_str(), chain.len())),
    )
    .map_err(|violation| match violation {
        DeploymentChainCapacityViolation::SpaceCount => {
            conflict("M26 deployment space safety bound is exceeded")
        }
        DeploymentChainCapacityViolation::DecisionCount => {
            conflict("M26 deployment decision quota is exceeded")
        }
    })?;
    for chain in chains.values_mut() {
        validate_deployment_chain(chain)?;
    }
    Ok(chains)
}
