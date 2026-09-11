//! Grounding work.

use super::*;

pub(super) fn validate_grounding_work(
    space: &SpaceType,
    vetoes: &[VetoRule],
    chunks: &[Chunk],
    max_work: u64,
    max_chunk_work: u64,
) -> Result<(), CogniGraphError> {
    let byte_units = |bytes: u64| {
        bytes
            .saturating_add(1023)
            .checked_div(1024)
            .unwrap_or(0)
            .max(1)
    };
    let mut setup_work = (space.entities.len() as u64)
        .checked_add(space.relation_rules.len() as u64)
        .and_then(|work| work.checked_add(vetoes.len() as u64))
        .ok_or_else(|| validation("M22 grounding setup-work estimate overflowed"))?;
    let mut surface_profiles = HashMap::with_capacity(space.entities.len());
    for entity in &space.entities {
        let mut max_bytes = 0_u64;
        let mut count = 0_u64;
        for surface in std::iter::once(&entity.name).chain(&entity.aliases) {
            let bytes = surface.to_lowercase().len() as u64;
            max_bytes = max_bytes.max(bytes);
            count = count
                .checked_add(1)
                .ok_or_else(|| validation("M22 surface-count estimate overflowed"))?;
            setup_work = setup_work
                .checked_add(byte_units(bytes))
                .ok_or_else(|| validation("M22 grounding setup-work estimate overflowed"))?;
        }
        surface_profiles.insert(entity.name.as_str(), (count, max_bytes));
    }
    let surface_profile = |name: &str| {
        surface_profiles
            .get(name)
            .copied()
            .unwrap_or((0_u64, 0_u64))
    };
    let mut vetoes_by_triple = HashMap::<(&str, &str, &str), Vec<&VetoRule>>::new();
    for veto in vetoes {
        for phrase in &veto.when_any {
            setup_work = setup_work
                .checked_add(byte_units(phrase.to_lowercase().len() as u64))
                .ok_or_else(|| validation("M22 grounding setup-work estimate overflowed"))?;
        }
        vetoes_by_triple
            .entry((
                veto.source.as_str(),
                veto.relation.as_str(),
                veto.target.as_str(),
            ))
            .or_default()
            .push(veto);
    }
    let mut total_work = 0_u64;
    for chunk in chunks {
        let text_units = byte_units(chunk.text.to_lowercase().len() as u64);
        let mut chunk_work = text_units
            .checked_add(setup_work)
            .ok_or_else(|| validation("M22 grounding-work estimate overflowed"))?;
        if chunk_work > max_chunk_work {
            return Err(CogniGraphError::CapacityExceeded(format!(
                "M22 chunk `{}` grounding setup work estimate {chunk_work} exceeds the pinned {max_chunk_work} unit limit",
                chunk.id
            )));
        }
        for rule in &space.relation_rules {
            for veto in vetoes_by_triple
                .get(&(
                    rule.source.as_str(),
                    rule.relation.as_str(),
                    rule.target.as_str(),
                ))
                .into_iter()
                .flatten()
            {
                for phrase in &veto.when_any {
                    let scan_work = text_units
                        .checked_add(byte_units(phrase.to_lowercase().len() as u64))
                        .ok_or_else(|| validation("M22 grounding-work estimate overflowed"))?;
                    chunk_work = chunk_work
                        .checked_add(scan_work)
                        .ok_or_else(|| validation("M22 grounding-work estimate overflowed"))?;
                }
            }
            for trigger in &rule.when_any {
                let source_placeholders = trigger.matches("{source}").count() as u64;
                let target_placeholders = trigger.matches("{target}").count() as u64;
                let (source_count, source_bytes) = if source_placeholders == 0 {
                    (1_u64, 0_u64)
                } else {
                    surface_profile(&rule.source)
                };
                let (target_count, target_bytes) = if target_placeholders == 0 {
                    (1_u64, 0_u64)
                } else {
                    surface_profile(&rule.target)
                };
                let expansions = source_count
                    .checked_mul(target_count)
                    .ok_or_else(|| validation("M22 trigger-expansion work estimate overflowed"))?;
                let expanded_bytes = (trigger.to_lowercase().len() as u64)
                    .checked_add(source_placeholders.checked_mul(source_bytes).ok_or_else(
                        || validation("M22 trigger-expansion size estimate overflowed"),
                    )?)
                    .and_then(|bytes| {
                        target_placeholders
                            .checked_mul(target_bytes)
                            .and_then(|target| bytes.checked_add(target))
                    })
                    .ok_or_else(|| validation("M22 trigger-expansion size estimate overflowed"))?;
                let mut attempt_work = text_units
                    .checked_add(byte_units(expanded_bytes))
                    .ok_or_else(|| validation("M22 grounding-work estimate overflowed"))?;
                if !rule.require_in_sentence.is_empty() {
                    let required_surfaces = rule
                        .require_in_sentence
                        .iter()
                        .try_fold(0_u64, |count, endpoint| {
                            let endpoint_count = match endpoint {
                                EndpointRef::Source => surface_profile(&rule.source).0,
                                EndpointRef::Target => surface_profile(&rule.target).0,
                            };
                            count
                                .checked_add(endpoint_count)
                                .ok_or_else(|| validation("M22 grounding-work estimate overflowed"))
                        })?
                        .max(1);
                    let gated_scan = text_units
                        .checked_mul(text_units)
                        .and_then(|work| work.checked_mul(required_surfaces))
                        .ok_or_else(|| validation("M22 gated-work estimate overflowed"))?;
                    attempt_work = attempt_work
                        .checked_add(gated_scan)
                        .ok_or_else(|| validation("M22 grounding-work estimate overflowed"))?;
                }
                chunk_work = chunk_work
                    .checked_add(expansions.checked_mul(attempt_work).ok_or_else(|| {
                        validation("M22 trigger-expansion work estimate overflowed")
                    })?)
                    .ok_or_else(|| validation("M22 grounding-work estimate overflowed"))?;
            }
            if chunk_work > max_chunk_work {
                return Err(CogniGraphError::CapacityExceeded(format!(
                    "M22 chunk `{}` grounding work estimate {chunk_work} exceeds the pinned {max_chunk_work} unit limit",
                    chunk.id
                )));
            }
        }
        total_work = total_work
            .checked_add(chunk_work)
            .ok_or_else(|| validation("M22 grounding-work estimate overflowed"))?;
        if total_work > max_work {
            return Err(CogniGraphError::CapacityExceeded(format!(
                "M22 grounding work estimate {total_work} exceeds the pinned {max_work} unit limit"
            )));
        }
    }
    Ok(())
}
