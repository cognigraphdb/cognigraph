//! Trigger expansion over entity surfaces, authored placeholders and the sentence gate.

use super::*;

pub(super) struct EntitySurfaces<'a> {
    pub(super) raw: Vec<&'a str>,
    pub(super) casefolded: Vec<String>,
}

pub(super) fn indexed_entity_surfaces(config: &SpaceType) -> HashMap<&str, EntitySurfaces<'_>> {
    let mut indexed = HashMap::new();
    for entity in &config.entities {
        let surfaces = indexed
            .entry(entity.name.as_str())
            .or_insert_with(|| EntitySurfaces {
                raw: Vec::new(),
                casefolded: Vec::new(),
            });
        for surface in
            std::iter::once(entity.name.as_str()).chain(entity.aliases.iter().map(String::as_str))
        {
            surfaces.raw.push(surface);
            surfaces.casefolded.push(surface.to_lowercase());
        }
    }
    indexed
}

/// Expand a trigger's `{source}`/`{target}` placeholders against the
/// endpoints' surfaces (as authored, one expansion per combination), in
/// deterministic order. Plain triggers pass through unchanged. The
/// expansion is what actually matches — and what gets recorded as the
/// grounded trigger, so provenance stays verbatim.
pub(super) fn find_expanded_trigger(
    phrase: &str,
    rule: &RelationRule,
    surfaces: &HashMap<&str, EntitySurfaces<'_>>,
    text_cf: &str,
    negated_at: &[bool],
    accept: &impl Fn(usize) -> bool,
) -> Option<(String, (usize, usize))> {
    let needs_source = phrase.contains("{source}");
    let needs_target = phrase.contains("{target}");
    debug_assert!(needs_source || needs_target);
    let source_options: Vec<Option<&str>> = if needs_source {
        surfaces
            .get(rule.source.as_str())
            .into_iter()
            .flat_map(|surfaces| surfaces.raw.iter().copied())
            .map(Some)
            .collect()
    } else {
        vec![None]
    };
    let target_options: Vec<Option<&str>> = if needs_target {
        surfaces
            .get(rule.target.as_str())
            .into_iter()
            .flat_map(|surfaces| surfaces.raw.iter().copied())
            .map(Some)
            .collect()
    } else {
        vec![None]
    };
    for source in &source_options {
        for target in &target_options {
            let expanded = substitute_authored_placeholders(phrase, *source, *target);
            let expanded_cf = expanded.to_lowercase();
            if let Some(offset) =
                affirming_offset_where_indexed(text_cf, &expanded_cf, negated_at, accept)
            {
                return Some((expanded, (offset, offset + expanded_cf.len())));
            }
        }
    }
    None
}

/// Substitute placeholders found in the authored trigger exactly once.
///
/// Surface text is always literal: a `{target}` token inside a source alias is
/// not interpreted during the target substitution pass. Besides preventing
/// surprising matches, this keeps expansion work proportional to the authored
/// template and selected surfaces.
pub(super) fn substitute_authored_placeholders(
    phrase: &str,
    source: Option<&str>,
    target: Option<&str>,
) -> String {
    let mut expanded = String::with_capacity(phrase.len());
    let mut cursor = 0;
    while cursor < phrase.len() {
        let remaining = &phrase[cursor..];
        if remaining.starts_with("{source}") {
            expanded.push_str(source.unwrap_or("{source}"));
            cursor += "{source}".len();
        } else if remaining.starts_with("{target}") {
            expanded.push_str(target.unwrap_or("{target}"));
            cursor += "{target}".len();
        } else {
            let character = remaining
                .chars()
                .next()
                .expect("cursor is before the end of the trigger");
            expanded.push(character);
            cursor += character.len_utf8();
        }
    }
    expanded
}

pub(super) fn sentence_gate_accepts(
    text_cf: &str,
    required: &[&[String]],
    sentence_ranges: &[(usize, usize)],
    cache: &RefCell<HashMap<usize, bool>>,
    cf_start: usize,
) -> bool {
    if required.is_empty() {
        return true;
    }
    let sentence_index = sentence_ranges.partition_point(|(_, end)| *end <= cf_start);
    let Some(&(start, end)) = sentence_ranges.get(sentence_index) else {
        return false;
    };
    if cf_start < start {
        return false;
    }
    if let Some(accepted) = cache.borrow().get(&sentence_index).copied() {
        return accepted;
    }
    let accepted = required.iter().all(|surfaces| {
        surfaces
            .iter()
            .any(|surface| text_cf[start..end].contains(surface))
    });
    cache.borrow_mut().insert(sentence_index, accepted);
    accepted
}
