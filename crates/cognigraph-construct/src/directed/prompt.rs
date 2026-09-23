//! Taxonomy validation, the directed system prompt and the response schema.

use super::*;

pub(super) fn validate_taxonomy(taxonomy: &[DirectedRelation]) -> Result<()> {
    if taxonomy.is_empty() {
        return Err(CogniGraphError::ValidationError(
            "directed construction needs a non-empty taxonomy".into(),
        ));
    }
    let mut seen = std::collections::HashSet::new();
    for rule in taxonomy {
        if rule.relation.trim().is_empty() {
            return Err(CogniGraphError::ValidationError(
                "taxonomy relation names must be non-empty".into(),
            ));
        }
        if !seen.insert(rule.relation.trim().to_string()) {
            return Err(CogniGraphError::ValidationError(format!(
                "duplicate taxonomy relation `{}`",
                rule.relation
            )));
        }
        if rule
            .require_in_sentence
            .iter()
            .all(|phrase| phrase.trim().is_empty())
        {
            return Err(CogniGraphError::ValidationError(format!(
                "taxonomy relation `{}` has no restraint vocabulary — every directed \
                 relation must name at least one require_in_sentence phrase",
                rule.relation
            )));
        }
    }
    Ok(())
}

pub(super) const DIRECTED_SYSTEM: &str = "You extract facts from document excerpts, under restraint.\n\
Rules:\n\
- Assert ONLY relations from the taxonomy the user provides; never invent one.\n\
- `source` and `target` must be short entity names copied VERBATIM from the evidence sentence.\n\
- `evidence` must be an EXACT quote (one sentence, or a contiguous span) copied from one excerpt, \
  and `chunk_id` must be that excerpt's id.\n\
- Only assert what the quoted sentence itself states. If the document does not clearly state a \
  relation, do not report it. An empty facts list is a good answer for an excerpt with nothing \
  to say.\n\
Return JSON only.";

pub(super) fn proposal_schema(taxonomy: &[DirectedRelation], chunks: &[Chunk]) -> Value {
    // IDs are opaque: bind the exact values used by the local gates, with no
    // trimming, normalization or repair. Sets keep enum order deterministic.
    let relations: BTreeSet<&str> = taxonomy.iter().map(|r| r.relation.as_str()).collect();
    let chunk_ids: BTreeSet<&str> = chunks.iter().map(|c| c.id.as_str()).collect();
    json!({
        "type": "object",
        "properties": {
            "facts": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "source": {"type": "string"},
                        "source_type": {"type": "string"},
                        "target": {"type": "string"},
                        "target_type": {"type": "string"},
                        "relation": {"type": "string", "enum": relations},
                        "evidence": {"type": "string"},
                        "chunk_id": {"type": "string", "enum": chunk_ids}
                    },
                    "required": ["source", "source_type", "target", "target_type",
                                  "relation", "evidence", "chunk_id"],
                    "additionalProperties": false
                }
            }
        },
        "required": ["facts"],
        "additionalProperties": false
    })
}
