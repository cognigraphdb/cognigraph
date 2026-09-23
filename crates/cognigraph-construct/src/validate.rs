//! Neuron validation against the ontology — the "ontology-bounded" and
//! "evidence-bound" properties, enforced with the same rejections as the
//! research implementation.

use std::collections::{HashMap, HashSet};

use thiserror::Error;

use crate::types::{Neuron, NeuronKind, NeuronSet, NeuronStatus, SpaceType};

#[derive(Debug, Error, Clone, PartialEq)]
pub enum NeuronError {
    #[error("neuron `{0}`: id must be kebab-case")]
    BadId(String),
    #[error("neuron `{0}`: space_type `{1}` does not match `{2}`")]
    SpaceTypeMismatch(String, String, String),
    #[error("neuron `{0}`: unknown entity `{1}`")]
    UnknownEntity(String, String),
    #[error("neuron `{0}`: unknown relation `{1}`")]
    UnknownRelation(String, String),
    #[error("neuron `{0}`: evidence must not be empty")]
    EmptyEvidence(String),
    #[error("neuron `{0}`: aliases must not be empty")]
    EmptyAliases(String),
    #[error("neuron `{0}`: triggers must not be empty")]
    EmptyTriggers(String),
    #[error("neuron `{0}`: confidence {1} out of range 0..=1")]
    BadConfidence(String, f64),
    #[error("neuron `{0}`: boost {1} must be finite and non-zero")]
    BadBoost(String, f64),
    #[error(
        "neurons `{0}` and `{1}`: accepted hint and blocker target the same triple — retire one"
    )]
    HintBlockerConflict(String, String),
}

fn is_kebab(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !id.starts_with('-')
        && !id.ends_with('-')
}

pub fn validate_neurons(set: &NeuronSet, space: &SpaceType) -> Result<(), NeuronError> {
    let entity_names: HashSet<&str> = space.entities.iter().map(|e| e.name.as_str()).collect();
    let relations: HashSet<&str> = space
        .relation_rules
        .iter()
        .map(|r| r.relation.as_str())
        .collect();
    if set.space_type != space.id {
        return Err(NeuronError::SpaceTypeMismatch(
            set.neurons
                .first()
                .map(|n| n.id.clone())
                .unwrap_or_default(),
            set.space_type.clone(),
            space.id.clone(),
        ));
    }
    for neuron in &set.neurons {
        validate_neuron(neuron, &entity_names, &relations)?;
    }
    // Decision 4 (decision_relation_blocker.md): an ACCEPTED hint and an
    // ACCEPTED blocker on the same triple is either sophisticated intent or
    // a review mistake — at our n it is the latter. The engine's semantics
    // would not be ambiguous (the veto wins), but the pair must be resolved
    // by a human, not shipped.
    let mut blockers = HashMap::new();
    for blocker in set.neurons.iter().filter(|neuron| {
        neuron.kind == NeuronKind::RelationBlocker && neuron.status == NeuronStatus::Accepted
    }) {
        blockers
            .entry((
                blocker.source.as_str(),
                blocker.relation.as_str(),
                blocker.target.as_str(),
            ))
            .or_insert(blocker.id.as_str());
    }
    for hint in set.neurons.iter().filter(|neuron| {
        neuron.kind == NeuronKind::RelationHint && neuron.status == NeuronStatus::Accepted
    }) {
        if let Some(blocker_id) = blockers.get(&(
            hint.source.as_str(),
            hint.relation.as_str(),
            hint.target.as_str(),
        )) {
            return Err(NeuronError::HintBlockerConflict(
                hint.id.clone(),
                (*blocker_id).to_string(),
            ));
        }
    }
    Ok(())
}

fn validate_neuron(
    neuron: &Neuron,
    entity_names: &HashSet<&str>,
    relations: &HashSet<&str>,
) -> Result<(), NeuronError> {
    if !is_kebab(&neuron.id) {
        return Err(NeuronError::BadId(neuron.id.clone()));
    }
    if !(0.0..=1.0).contains(&neuron.confidence) {
        return Err(NeuronError::BadConfidence(
            neuron.id.clone(),
            neuron.confidence,
        ));
    }
    if neuron.evidence.iter().all(|e| e.trim().is_empty()) {
        return Err(NeuronError::EmptyEvidence(neuron.id.clone()));
    }
    match neuron.kind {
        NeuronKind::Alias => {
            if !entity_names.contains(&neuron.entity.as_str()) {
                return Err(NeuronError::UnknownEntity(
                    neuron.id.clone(),
                    neuron.entity.clone(),
                ));
            }
            if neuron.aliases.is_empty() {
                return Err(NeuronError::EmptyAliases(neuron.id.clone()));
            }
        }
        NeuronKind::RelationHint => {
            for endpoint in [&neuron.source, &neuron.target] {
                if !entity_names.contains(&endpoint.as_str()) {
                    return Err(NeuronError::UnknownEntity(
                        neuron.id.clone(),
                        endpoint.clone(),
                    ));
                }
            }
            if !relations.contains(&neuron.relation.as_str()) {
                return Err(NeuronError::UnknownRelation(
                    neuron.id.clone(),
                    neuron.relation.clone(),
                ));
            }
            if neuron.triggers.is_empty() {
                return Err(NeuronError::EmptyTriggers(neuron.id.clone()));
            }
        }
        NeuronKind::RelationBlocker => {
            // A blocker vetoes grounding of a configured triple, so it is
            // bounded exactly like a relation hint: known endpoints, known
            // relation, and at least one veto phrase.
            for endpoint in [&neuron.source, &neuron.target] {
                if !entity_names.contains(&endpoint.as_str()) {
                    return Err(NeuronError::UnknownEntity(
                        neuron.id.clone(),
                        endpoint.clone(),
                    ));
                }
            }
            if !relations.contains(&neuron.relation.as_str()) {
                return Err(NeuronError::UnknownRelation(
                    neuron.id.clone(),
                    neuron.relation.clone(),
                ));
            }
            if neuron.triggers.iter().all(|t| t.trim().is_empty()) {
                return Err(NeuronError::EmptyTriggers(neuron.id.clone()));
            }
        }
        NeuronKind::RelationRankHint => {
            // The relation must exist to be reweighted: either a configured
            // rule's relation or one of the built-in low-signal relations
            // that ingestion constructs (e.g. MENTIONS).
            let known = relations.contains(&neuron.relation.as_str())
                || crate::rank::is_low_signal(&neuron.relation);
            if !known {
                return Err(NeuronError::UnknownRelation(
                    neuron.id.clone(),
                    neuron.relation.clone(),
                ));
            }
            if !neuron.boost.is_finite() || neuron.boost == 0.0 {
                return Err(NeuronError::BadBoost(neuron.id.clone(), neuron.boost));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "validate_tests.rs"]
mod tests;
