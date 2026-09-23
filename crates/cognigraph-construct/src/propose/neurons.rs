//! Neuron proposal for one missed fact: candidate evidence, the LLM call, validation.

use super::*;

fn neuron_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "id": { "type": "string" },
            "type": { "type": "string", "enum": ["relation_hint"] },
            "confidence": { "type": "number" },
            "rationale": { "type": "string" },
            "source": { "type": "string" },
            "relation": { "type": "string" },
            "target": { "type": "string" },
            "triggers": { "type": "array", "items": { "type": "string" } },
            "evidence": { "type": "array", "items": { "type": "string" } }
        },
        "required": ["id", "type", "source", "relation", "target", "triggers", "evidence"],
        "additionalProperties": true
    })
}

/// Candidate evidence: chunks mentioning the fact's endpoints (by name or
/// alias). Chunks mentioning BOTH endpoints rank first — a chunk that names
/// only the popular endpoint rarely asserts the relation, and feeding the
/// model those makes it decline facts the document does support.
pub(super) fn candidate_chunks<'a>(
    fact: &Fact,
    space: &SpaceType,
    chunks: &'a [Chunk],
) -> Vec<&'a Chunk> {
    let surfaces_of = |name: &str| -> Vec<String> {
        space
            .entities
            .iter()
            .filter(|e| e.name == name)
            .flat_map(|e| std::iter::once(e.name.clone()).chain(e.aliases.iter().cloned()))
            .map(|s| s.to_lowercase())
            .collect()
    };
    let source_surfaces = surfaces_of(&fact.source);
    let target_surfaces = surfaces_of(&fact.target);
    let mut scored: Vec<(usize, &Chunk)> = chunks
        .iter()
        .filter_map(|chunk| {
            let text = chunk.text.to_lowercase();
            let hits_source = source_surfaces.iter().any(|s| text.contains(s));
            let hits_target = target_surfaces.iter().any(|s| text.contains(s));
            match (hits_source, hits_target) {
                (true, true) => Some((0, chunk)),
                (true, false) | (false, true) => Some((1, chunk)),
                (false, false) => None,
            }
        })
        .collect();
    scored.sort_by_key(|(score, _)| *score); // stable: keeps document order per tier
    scored.into_iter().map(|(_, c)| c).take(6).collect()
}

pub(super) const ATTEMPTS: usize = 2;

pub(super) async fn propose_one(
    provider: &dyn CompletionProvider,
    space: &SpaceType,
    fact: &Fact,
    index: usize,
    evidence: Vec<&Chunk>,
    corpus: &[Chunk],
    existing: &[Neuron],
) -> std::result::Result<Neuron, String> {
    if evidence.is_empty() {
        return Err("no candidate evidence chunks mention the endpoints".into());
    }
    let evidence_text: Vec<&str> = evidence.iter().map(|c| c.text.as_str()).collect();
    let user = format!(
        "Missing fact: {} --{}--> {}\n\nCandidate evidence chunks:\n{}\n\n\
         Propose exactly ONE relation_hint neuron that recovers this fact.\n\
         Rules:\n\
         - Triggers must be short phrases copied VERBATIM from sentences that \
         explicitly assert this specific relation between this source and target.\n\
         - Do NOT use sentences that only imply the fact by category membership \
         or co-occurrence; if no sentence explicitly supports the fact, return \
         an empty triggers array.\n\
         - id must be descriptive kebab-case (e.g. \"kurtz-leads-crowdstrike\").\n\
         - evidence must quote the licensing sentences.",
        fact.source,
        fact.relation,
        fact.target,
        evidence_text.join("\n---\n")
    );
    let system = "You propose Semantic Neurons: governed edge-construction rules. \
                  You may ONLY reference the given source, relation, and target. \
                  You never invent entities or relations. Precision beats recall: \
                  an unsupported proposal is worse than no proposal.";

    let mut last_error = String::new();
    for _ in 0..ATTEMPTS {
        let value = match provider
            .complete_json(system, &user, &neuron_schema())
            .await
        {
            Ok(value) => value,
            Err(e) => {
                last_error = format!("completion error: {e}");
                continue;
            }
        };
        // Tolerate a {"neurons": [ ... ]} wrapper shape.
        let value = value
            .get("neurons")
            .and_then(|n| n.as_array())
            .and_then(|a| a.first())
            .cloned()
            .unwrap_or(value);
        let mut neuron: Neuron = match serde_json::from_value(value) {
            Ok(neuron) => neuron,
            Err(e) => {
                last_error = format!("unparseable proposal: {e}");
                continue;
            }
        };
        // Normalize and force the governance invariants.
        neuron.status = NeuronStatus::Proposed;
        neuron.source = fact.source.clone();
        neuron.relation = fact.relation.clone();
        neuron.target = fact.target.clone();
        neuron.id = kebab(&neuron.id);
        if neuron.id.is_empty() || existing.iter().any(|n| n.id == neuron.id) {
            neuron.id = kebab(&format!(
                "{}-{}-{}-{index}",
                fact.source, fact.relation, fact.target
            ));
        }
        if neuron.triggers.is_empty() {
            // A decline is an answer, but a single sample is noisy — it
            // stands only if the model declines on every attempt.
            last_error = "model declined: no explicit supporting sentence".into();
            continue;
        }
        // Symbolic self-check: a trigger that never occurs verbatim in the
        // document can never ground. Paraphrased triggers are a retry case.
        let before = neuron.triggers.len();
        neuron.triggers.retain(|t| {
            let t = t.to_lowercase();
            corpus.iter().any(|c| c.text.to_lowercase().contains(&t))
        });
        if neuron.triggers.is_empty() {
            last_error = format!(
                "all {before} triggers were paraphrased (none occur verbatim in the document)"
            );
            continue;
        }
        // Per-neuron validation: one bad proposal must not sink the batch.
        // Accepted shape exercises the full check set.
        let mut check = neuron.clone();
        check.status = NeuronStatus::Accepted;
        let single = NeuronSet {
            space_type: space.id.clone(),
            neurons: vec![check],
        };
        if let Err(e) = validate_neurons(&single, space) {
            last_error = format!("invalid proposal: {e}");
            continue;
        }
        return Ok(neuron);
    }
    Err(last_error)
}

pub(super) fn kebab(raw: &str) -> String {
    raw.to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}
