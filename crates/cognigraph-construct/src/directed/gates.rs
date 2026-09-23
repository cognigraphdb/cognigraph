//! The pure evidence gate for directed proposals and its quote and endpoint matching.

use super::*;

/// Find `needle` in `haystack`, case-insensitive (ASCII) and tolerant of
/// whitespace differences: any whitespace RUN on either side matches any
/// whitespace run on the other. Returns byte offsets into the ORIGINAL
/// haystack, so evidence spans point at the real text even when the model
/// collapsed a line break into a space.
pub(crate) fn find_loose(haystack: &str, needle: &str) -> Option<(usize, usize)> {
    let needle = needle.trim();
    if needle.is_empty() {
        return None;
    }
    let hay = haystack.as_bytes();
    let need = needle.as_bytes();
    let mut start = 0usize;
    while start < hay.len() {
        // Candidate must begin on a non-space matching needle's first byte.
        if hay[start].is_ascii_whitespace()
            || !need
                .first()
                .is_some_and(|&b| b.eq_ignore_ascii_case(&hay[start]))
        {
            start += 1;
            continue;
        }
        let (mut i, mut j) = (start, 0usize);
        loop {
            if j >= need.len() {
                return Some((start, i));
            }
            if i >= hay.len() {
                break;
            }
            let (hb, nb) = (hay[i], need[j]);
            if nb.is_ascii_whitespace() {
                if !hb.is_ascii_whitespace() {
                    break;
                }
                while i < hay.len() && hay[i].is_ascii_whitespace() {
                    i += 1;
                }
                while j < need.len() && need[j].is_ascii_whitespace() {
                    j += 1;
                }
                continue;
            }
            if !hb.eq_ignore_ascii_case(&nb) {
                break;
            }
            i += 1;
            j += 1;
        }
        start += 1;
    }
    None
}

/// Endpoint mentions use the quote matcher's ASCII case/whitespace tolerance,
/// but neither adjacent character may be a Unicode word character (UTS #18:
/// Alphabetic, Mark, Decimal_Number, Connector_Punctuation or Join_Control).
/// Punctuation such as apostrophes and hyphens separates tokens. This prevents
/// partial words, not partial multi-word names or incorrect entity resolution;
/// unsegmented scripts conservatively require a separator around the name.
/// Keep this separate from quote lookup so evidence byte spans do not change.
pub(super) fn find_endpoint(haystack: &str, needle: &str) -> Option<(usize, usize)> {
    let mut offset = 0;
    while let Some((start, end)) = find_loose(&haystack[offset..], needle) {
        let (start, end) = (offset + start, offset + end);
        let touches_word = haystack[..start]
            .chars()
            .next_back()
            .is_some_and(regex_syntax::is_word_character)
            || haystack[end..]
                .chars()
                .next()
                .is_some_and(regex_syntax::is_word_character);
        if !touches_word {
            return Some((start, end));
        }
        // An invalid first substring must not hide a later complete mention.
        offset = start + haystack[start..].chars().next()?.len_utf8();
    }
    None
}

/// Run every proposal through the gates. Pure — the LLM and the backend sit
/// on either side of this function, which is what makes the restraint
/// behaviour testable in isolation.
/// Canonically equivalent quotes/endpoints are compared in NFC; returned spans
/// index NFC chunk text, the same representation used by the ingestion writer.
pub fn gate_directed_proposals(
    chunks: &[Chunk],
    taxonomy: &[DirectedRelation],
    proposals: &[DirectedProposal],
    extracted_by: &str,
) -> (Vec<(String, GroundedFact)>, Vec<EntityDef>, Vec<Refusal>) {
    let chunks = canonical_chunks(chunks);
    let by_id: BTreeMap<&str, &Chunk> = chunks.iter().map(|c| (c.id.as_str(), c)).collect();
    let by_relation: BTreeMap<&str, &DirectedRelation> =
        taxonomy.iter().map(|r| (r.relation.as_str(), r)).collect();

    let mut grounded: Vec<(String, GroundedFact)> = Vec::new();
    let mut entities: BTreeMap<String, EntityDef> = BTreeMap::new();
    let mut skips: Vec<Refusal> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for proposal in proposals {
        let p = DirectedProposal {
            source: canonical_text(&proposal.source).into_owned(),
            target: canonical_text(&proposal.target).into_owned(),
            evidence: canonical_text(&proposal.evidence).into_owned(),
            ..proposal.clone()
        };
        let label = format!("{} --{}--> {}", p.source, p.relation, p.target);
        let refusal = |gate: DirectedGate, reason: String| Refusal {
            gate,
            source: p.source.clone(),
            relation: p.relation.clone(),
            target: p.target.clone(),
            chunk_id: p.chunk_id.clone(),
            evidence: p.evidence.clone(),
            reason,
        };
        let Some(rule) = by_relation.get(p.relation.as_str()) else {
            skips.push(refusal(
                DirectedGate::RelationNotInTaxonomy,
                format!("`{label}`: relation not in the taxonomy — dropped"),
            ));
            continue;
        };
        let Some(chunk) = by_id.get(p.chunk_id.as_str()) else {
            skips.push(refusal(
                DirectedGate::ChunkNotInRequest,
                format!(
                    "`{label}`: cites chunk `{}` which is not in this request — dropped",
                    p.chunk_id
                ),
            ));
            continue;
        };
        if p.source.trim().is_empty() || p.target.trim().is_empty() {
            skips.push(refusal(
                DirectedGate::EmptyEndpoint,
                format!("`{label}`: empty endpoint — dropped"),
            ));
            continue;
        }
        // An endpoint must sanitize to a usable identity: "[***]" (a
        // redaction) passes every textual gate but has no key to live
        // under, and letting it through would fail the whole atomic write.
        if entity_key(&p.source).is_empty() || entity_key(&p.target).is_empty() {
            skips.push(refusal(
                DirectedGate::UnusableEndpointIdentity,
                format!("`{label}`: an endpoint has no usable identity (symbols only) — dropped"),
            ));
            continue;
        }
        let Some((ev_start, ev_end)) = find_loose(&chunk.text, &p.evidence) else {
            skips.push(refusal(
                DirectedGate::EvidenceNotVerbatim,
                format!(
                    "`{label}`: evidence is not a verbatim quote of chunk `{}` — dropped",
                    p.chunk_id
                ),
            ));
            continue;
        };
        // The gate window: the sentence the quote starts in, extended to the
        // end of the quote when it crosses a boundary.
        let (s_start, s_end) = sentence_bounds(&chunk.text, ev_start);
        let window = &chunk.text[s_start..s_end.max(ev_end)];
        if find_endpoint(window, &p.source).is_none() {
            skips.push(refusal(
                DirectedGate::SourceNotInSentence,
                format!("`{label}`: source does not occur in the evidence sentence — dropped"),
            ));
            continue;
        }
        if find_endpoint(window, &p.target).is_none() {
            skips.push(refusal(
                DirectedGate::TargetNotInSentence,
                format!("`{label}`: target does not occur in the evidence sentence — dropped"),
            ));
            continue;
        }
        let affirmed = rule.require_in_sentence.iter().any(|phrase| {
            !phrase.trim().is_empty() && affirms_phrase(window, &canonical_text(phrase))
        });
        if !affirmed {
            skips.push(refusal(
                DirectedGate::VocabularyNotAffirmed,
                format!(
                    "`{label}`: no affirmed occurrence of the relation's vocabulary in the \
                     evidence sentence — dropped"
                ),
            ));
            continue;
        }
        if !seen.insert((
            entity_key(&p.source),
            rule.relation.clone(),
            entity_key(&p.target),
            chunk.id.clone(),
            ev_start,
        )) {
            continue;
        }
        for (name, kind) in [(&p.source, &p.source_type), (&p.target, &p.target_type)] {
            let key = entity_key(name);
            entities.entry(key).or_insert_with(|| EntityDef {
                name: name.trim().to_string(),
                entity_type: if kind.trim().is_empty() {
                    "entity".into()
                } else {
                    kind.trim().to_string()
                },
                aliases: Vec::new(),
            });
        }
        grounded.push((
            chunk.id.clone(),
            GroundedFact {
                fact: Fact {
                    source: p.source.trim().to_string(),
                    relation: rule.relation.clone(),
                    target: p.target.trim().to_string(),
                },
                trigger: chunk.text[ev_start..ev_end].to_string(),
                chunk_id: chunk.id.clone(),
                licensed_by_neuron: None,
                reviewed_by: Some(extracted_by.to_string()),
                trigger_span: (ev_start, ev_end),
            },
        ));
    }
    (grounded, entities.into_values().collect(), skips)
}
