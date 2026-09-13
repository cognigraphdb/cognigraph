//! B2 (roadmap-2026-h2.md): end-to-end answer quality. Construction
//! metrics prove the right edges exist; this module measures whether they
//! carry through retrieval into ANSWERS: build a ranked graph-facts trace
//! per eval question (the same ranking the graph-augmented route uses,
//! rank-hint boosts included), have a model answer strictly from that
//! trace as a fact list, and score the list against the question's
//! expected and forbidden facts — answer-level recall and restraint.

use std::collections::HashSet;

use anyhow::Result;
use cognigraph_core::GraphBackend;
use cognigraph_embeddings::completion::CompletionProvider;
use serde_json::{Value, json};

use crate::ingest::entity_key;
use crate::rank::{EdgeSelectOpts, GraphEdge, rank_boosts, select_graph_edges};
use crate::types::{EvalSpec, Fact, Neuron, SpaceType};

#[derive(Debug, Clone)]
pub struct QuestionScore {
    pub id: String,
    pub question: String,
    pub expected_found: usize,
    pub expected_total: usize,
    pub forbidden_asserted: usize,
    pub forbidden_total: usize,
    /// Facts the model asserted, as `A --REL--> B` strings.
    pub asserted: Vec<String>,
    /// Asserted facts NOT present in the trace the model was given —
    /// fabrications. They never count toward recall (no credit for lucky
    /// invention) but DO count toward restraint, which stays a behavioral
    /// measurement of the model, not a structural guarantee.
    pub fabricated: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct AnswerOutcome {
    pub questions: Vec<QuestionScore>,
}

impl AnswerOutcome {
    pub fn recall_ok(&self) -> bool {
        self.questions
            .iter()
            .all(|q| q.expected_found == q.expected_total)
    }

    pub fn restraint_ok(&self) -> bool {
        self.questions.iter().all(|q| q.forbidden_asserted == 0)
    }
}

/// Options for [`answer_eval_with`].
#[derive(Debug, Clone, Copy, Default)]
pub struct AnswerOpts {
    /// Second, completeness-critic pass per question: the model reviews
    /// the trace against its own first-pass selection and may ADD facts
    /// it missed (never remove). Targets the measured residue — answer
    /// recall misses concentrated on boundary/meta questions where the
    /// first pass is too selective. Additions are trace-checked exactly
    /// like first-pass assertions; restraint stays behavioral and is
    /// measured over the union.
    pub two_pass: bool,
    /// Augment each trace fact line with its LICENSING SENTENCE (looked
    /// up via the edge's evidence_chunk_id + trigger_start — stored
    /// provenance, no new machinery). Targets the trace-representation
    /// residue: fact lines compress away the connective prose that
    /// meta-questions need (triple-confirmed across vector control,
    /// mini, and gpt-5.4). Scoring is unchanged — assertions are still
    /// plain fact lines checked against the plain trace.
    pub evidence_sentences: bool,
}

/// Answer every eval question from the constructed graph and score the
/// answers. The trace is built exactly like the graph-augmented route:
/// facts edges ranked by `select_graph_edges`, seeded by the entities the
/// question mentions, reweighted by ACCEPTED rank-hint neurons.
pub async fn answer_eval(
    backend: &dyn GraphBackend,
    space_id: &str,
    space: &SpaceType,
    neurons: &[Neuron],
    spec: &EvalSpec,
    provider: &dyn CompletionProvider,
) -> Result<AnswerOutcome> {
    answer_eval_with(
        backend,
        space_id,
        space,
        neurons,
        spec,
        provider,
        AnswerOpts::default(),
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub async fn answer_eval_with(
    backend: &dyn GraphBackend,
    space_id: &str,
    space: &SpaceType,
    neurons: &[Neuron],
    spec: &EvalSpec,
    provider: &dyn CompletionProvider,
    opts: AnswerOpts,
) -> Result<AnswerOutcome> {
    use cognigraph_core::{FieldPredicate, PredicateOp};
    let predicate = [FieldPredicate {
        path: vec!["space_id".into()],
        op: PredicateOp::Eq,
        value: Value::String(space_id.to_string()),
    }];
    let edge_docs = backend
        .list_documents_filtered("facts", &predicate, None, None, None)
        .await?;
    let edges: Vec<GraphEdge> = edge_docs.iter().filter_map(GraphEdge::from_value).collect();
    // Evidence lookup for the augmented trace: (from, relation, to,
    // chunk) -> trigger_start, straight from stored provenance.
    let trigger_starts: std::collections::HashMap<(String, String, String, String), usize> =
        edge_docs
            .iter()
            .filter_map(|doc| {
                Some((
                    (
                        doc.get("_from")?.as_str()?.to_string(),
                        doc.get("relation_type")?.as_str()?.to_string(),
                        doc.get("_to")?.as_str()?.to_string(),
                        doc.get("evidence_chunk_id")?.as_str()?.to_string(),
                    ),
                    doc.get("trigger_start")?.as_u64()? as usize,
                ))
            })
            .collect();
    let mut chunk_texts: std::collections::HashMap<String, Option<String>> =
        std::collections::HashMap::new();
    let boosts = rank_boosts(neurons);

    // Edge endpoints are entity keys ("entities/<sanitized>"); the trace
    // shows canonical names so answers and eval notation line up.
    let display = |vertex: &str| -> String {
        let key = vertex.strip_prefix("entities/").unwrap_or(vertex);
        space
            .entities
            .iter()
            .find(|e| entity_key(&e.name) == key)
            .map(|e| e.name.clone())
            .unwrap_or_else(|| key.to_string())
    };

    // The answerer's trace is its ENTIRE world — unlike the compact
    // graph-augmented route trace, stinginess here directly caps answer
    // recall (measured: at the route default of 12, 13/68 expected facts
    // never reached the model across the blind kits; at 24+ all did —
    // answer_trace_diag reproduces this).
    let edge_opts = EdgeSelectOpts {
        limit: 48,
        ..EdgeSelectOpts::default()
    };

    let mut questions = Vec::new();
    for question in &spec.questions {
        let seeds: HashSet<String> =
            crate::grounding::mentions(&question.question, &space.entities)
                .into_iter()
                .map(|e| format!("entities/{}", entity_key(&e.name)))
                .collect();
        let trace = select_graph_edges(&edges, &seeds, &boosts, &edge_opts);
        let fact_lines: Vec<String> = trace
            .iter()
            .map(|edge| {
                format!(
                    "{} --{}--> {}",
                    display(&edge.source),
                    edge.relation,
                    display(&edge.target)
                )
            })
            .collect();
        let trace_facts: Vec<Fact> = fact_lines.iter().filter_map(|l| Fact::parse(l)).collect();

        // Evidence-augmented trace: each fact line carries its licensing
        // sentence (stored provenance) — the connective prose fact lines
        // compress away. Scoring stays on the PLAIN lines.
        let mut prompt_lines = fact_lines.clone();
        if opts.evidence_sentences {
            for (line, edge) in prompt_lines.iter_mut().zip(&trace) {
                let key = (
                    edge.source.clone(),
                    edge.relation.clone(),
                    edge.target.clone(),
                    edge.evidence_chunk_id.clone(),
                );
                let Some(&start) = trigger_starts.get(&key) else {
                    continue;
                };
                let chunk_id = crate::ingest::chunk_key(space_id, &edge.evidence_chunk_id);
                if !chunk_texts.contains_key(&chunk_id) {
                    let text = backend
                        .get_document("chunks", &chunk_id)
                        .await
                        .ok()
                        .flatten()
                        .and_then(|d| d.get("text").and_then(Value::as_str).map(str::to_string));
                    chunk_texts.insert(chunk_id.clone(), text);
                }
                if let Some(Some(text)) = chunk_texts.get(&chunk_id) {
                    let (s, e) = crate::grounding::sentence_bounds(text, start);
                    let sentence: String = text[s..e].trim().chars().take(220).collect();
                    line.push_str(&format!("  [evidence: {sentence}]"));
                }
            }
        }
        let evidence_note = if opts.evidence_sentences {
            " Lines may carry an [evidence: …] annotation for context; return ONLY the \
             `A --REL--> B` part of each fact, never the annotation."
        } else {
            ""
        };

        let user = format!(
            "Graph facts (the ONLY knowledge you may use):\n{}\n\n\
             Question: {}\n\n\
             Return JSON {{\"facts\": [\"A --REL--> B\", ...]}} listing EVERY \
             fact from the list above that is relevant to answering the \
             question, copied verbatim — do not stop at the most salient \
             ones; include each fact that belongs in a complete answer. \
             Never include a fact that is not in the list. Return an empty \
             list if none apply.{evidence_note}",
            prompt_lines.join("\n"),
            question.question
        );
        let system = "You answer questions STRICTLY from the provided graph \
                      facts. Never invent facts; never reword them.";
        let response = provider
            .complete_json(system, &user, &answer_schema())
            .await?;
        let mut asserted: Vec<String> = response
            .get("facts")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        if opts.two_pass {
            // Completeness critic: same trace, the first-pass selection
            // shown, additions only. It can never REMOVE an assertion —
            // restraint is measured over the union, not laundered by a
            // second opinion.
            let critic = format!(
                "Graph facts (the ONLY knowledge you may use):\n{}\n\n\
                 Question: {}\n\n\
                 A first pass selected these facts as the answer:\n{}\n\n\
                 Review the full graph-fact list once more for COMPLETENESS: \
                 which facts from it were MISSED but also belong in a \
                 complete answer — including boundary, negative, \
                 definitional, or context facts a careful answer would \
                 cite? Return JSON {{\"facts\": [...]}} with ONLY the \
                 additional facts, copied verbatim from the graph-fact \
                 list; return an empty list if the selection is already \
                 complete.",
                prompt_lines.join("\n"),
                question.question,
                if asserted.is_empty() {
                    "(none)".to_string()
                } else {
                    asserted.join("\n")
                }
            );
            let additions = provider
                .complete_json(system, &critic, &answer_schema())
                .await?;
            if let Some(items) = additions.get("facts").and_then(Value::as_array) {
                for item in items.iter().filter_map(|v| v.as_str()) {
                    if !asserted.iter().any(|have| have == item) {
                        asserted.push(item.to_string());
                    }
                }
            }
        }
        let asserted_facts: Vec<Fact> =
            asserted.iter().filter_map(|raw| Fact::parse(raw)).collect();
        // Fabrications: asserted lines absent from the trace. Recall is
        // scored against trace-backed assertions only; restraint against
        // the raw list (a model that invents a forbidden fact must be
        // measured doing so, not silently laundered).
        let fabricated: Vec<String> = asserted
            .iter()
            .filter(|raw| Fact::parse(raw).is_none_or(|fact| !trace_facts.contains(&fact)))
            .cloned()
            .collect();

        let expected: Vec<Fact> = question
            .expected_facts
            .iter()
            .filter_map(|raw| Fact::parse(raw))
            .collect();
        let forbidden: Vec<Fact> = question
            .forbidden_facts
            .iter()
            .filter_map(|raw| Fact::parse(raw))
            .collect();
        questions.push(QuestionScore {
            id: question.id.clone(),
            question: question.question.clone(),
            expected_found: expected
                .iter()
                .filter(|f| asserted_facts.contains(f) && trace_facts.contains(f))
                .count(),
            expected_total: expected.len(),
            forbidden_asserted: forbidden
                .iter()
                .filter(|f| asserted_facts.contains(f))
                .count(),
            forbidden_total: forbidden.len(),
            asserted,
            fabricated,
        });
    }
    Ok(AnswerOutcome { questions })
}

fn answer_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "facts": { "type": "array", "items": { "type": "string" } }
        },
        "required": ["facts"],
        "additionalProperties": false
    })
}
