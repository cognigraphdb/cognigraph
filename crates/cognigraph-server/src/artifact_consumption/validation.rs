//! Validation.

use super::*;

pub(super) fn validate_digest(label: &str, value: &str) -> Result<(), CogniGraphError> {
    let valid = value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    });
    if !valid {
        return Err(validation(format!(
            "{label} must be sha256 followed by 64 lowercase hexadecimal characters"
        )));
    }
    Ok(())
}
pub(super) fn validate_text(label: &str, value: &str) -> Result<(), CogniGraphError> {
    if value.trim().is_empty()
        || value.len() > MAX_TEXT_BYTES
        || value.chars().any(char::is_control)
        || !is_nfc(value)
    {
        return Err(validation(format!(
            "{label} must be non-empty, NFC, control-free, and at most {MAX_TEXT_BYTES} bytes"
        )));
    }
    Ok(())
}
pub(super) fn validate_optional_text(
    label: &str,
    value: &str,
    max_bytes: usize,
) -> Result<(), CogniGraphError> {
    if value.len() > max_bytes || value.chars().any(char::is_control) || !is_nfc(value) {
        return Err(validation(format!(
            "{label} must be NFC, control-free, and at most {max_bytes} bytes"
        )));
    }
    Ok(())
}
pub(super) fn validate_corpus_text(
    label: &str,
    value: &str,
    max_bytes: u64,
) -> Result<(), CogniGraphError> {
    let disallowed_control = value
        .chars()
        .any(|character| character.is_control() && !matches!(character, '\n' | '\t'));
    if value.trim().is_empty()
        || value.len() as u64 > max_bytes
        || disallowed_control
        || !is_nfc(value)
    {
        return Err(validation(format!(
            "{label} must be non-empty NFC text with only LF/TAB controls and at most {max_bytes} bytes"
        )));
    }
    Ok(())
}
pub(super) fn validate_sorted_unique_text(
    label: &str,
    values: &[String],
    require_nonempty: bool,
) -> Result<(), CogniGraphError> {
    if require_nonempty && values.is_empty() {
        return Err(validation(format!("{label} must not be empty")));
    }
    let mut previous: Option<&str> = None;
    for value in values {
        validate_text(label, value)?;
        if previous.is_some_and(|previous| previous >= value.as_str()) {
            return Err(validation(format!("{label} must be sorted and unique")));
        }
        previous = Some(value);
    }
    Ok(())
}
pub(super) fn validate_sorted_unique_triggers(
    label: &str,
    triggers: &[String],
) -> Result<(), CogniGraphError> {
    validate_sorted_unique_text(label, triggers, true)?;
    for trigger in triggers {
        let remainder = trigger.replace("{source}", "").replace("{target}", "");
        if remainder.contains(['{', '}']) {
            return Err(validation(format!(
                "{label} contains an unsupported template placeholder"
            )));
        }
    }
    Ok(())
}
pub(super) fn validate_sorted_unique_endpoints(
    endpoints: &[EndpointRef],
) -> Result<(), CogniGraphError> {
    let mut previous = None;
    for endpoint in endpoints {
        let rank = match endpoint {
            EndpointRef::Source => 0_u8,
            EndpointRef::Target => 1_u8,
        };
        if previous.is_some_and(|previous| previous >= rank) {
            return Err(validation(
                "candidate.rule.require_in_sentence must be sorted and unique",
            ));
        }
        previous = Some(rank);
    }
    Ok(())
}
pub(super) fn construction_neuron_item_count(neuron: &ConstructionNeuron) -> usize {
    match neuron {
        ConstructionNeuron::Alias {
            evidence, aliases, ..
        } => evidence.len() + aliases.len(),
        ConstructionNeuron::RelationHint {
            evidence, triggers, ..
        }
        | ConstructionNeuron::RelationBlocker {
            evidence, triggers, ..
        } => evidence.len() + triggers.len(),
    }
}
pub(super) fn validate_construction_neuron(
    neuron: &ConstructionNeuron,
) -> Result<(), CogniGraphError> {
    let (reviewed_by, evidence) = match neuron {
        ConstructionNeuron::Alias {
            reviewed_by,
            evidence,
            entity,
            aliases,
            ..
        } => {
            validate_text("candidate.neuron.entity", entity)?;
            validate_sorted_unique_text("candidate.neuron.aliases", aliases, true)?;
            (reviewed_by, evidence)
        }
        ConstructionNeuron::RelationHint {
            reviewed_by,
            evidence,
            source,
            relation,
            target,
            triggers,
            ..
        }
        | ConstructionNeuron::RelationBlocker {
            reviewed_by,
            evidence,
            source,
            relation,
            target,
            triggers,
            ..
        } => {
            for (label, value) in [
                ("candidate.neuron.source", source),
                ("candidate.neuron.relation", relation),
                ("candidate.neuron.target", target),
            ] {
                validate_text(label, value)?;
            }
            validate_sorted_unique_triggers("candidate.neuron.triggers", triggers)?;
            (reviewed_by, evidence)
        }
    };
    if let Some(reviewed_by) = reviewed_by {
        validate_text("candidate.neuron.reviewed_by", reviewed_by)?;
    }
    validate_sorted_unique_text("candidate.neuron.evidence", evidence, true)
}
/// Apply the unchanged M22 corpus-dependent work budget to an invoked M25
/// materialization. The signed revision validates configuration shape; this
/// second check binds that configuration to the actual submitted chunks
/// before the caller enters construction writes.
pub(crate) fn validate_semantic_repair_grounding_work(
    space: &SpaceType,
    vetoes: &[VetoRule],
    chunks: &[Chunk],
) -> Result<(), CogniGraphError> {
    validate_grounding_work(
        space,
        vetoes,
        chunks,
        MAX_M22_GROUNDING_WORK,
        MAX_M22_CHUNK_GROUNDING_WORK,
    )
}
