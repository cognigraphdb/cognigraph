use std::collections::{BTreeMap, BTreeSet};

use anyhow::Result;
use serde_json::json;

use super::model::*;
use super::review::{fact_key, validate_annotations};
use super::vocabulary::ClinicalVocabulary;
use crate::{Chunk, Neuron, NeuronKind, NeuronStatus, SpaceType, ground_chunk};

pub const JUDGE_POLICY_THRESHOLD: f64 = 0.90;

/// The original hand-picked generic condition vocabulary of the shipped
/// clinical construction mechanism. Kept verbatim so the frozen baseline lane
/// measures the system as it actually shipped — calibration showed this
/// vocabulary cannot express a concept-preserving gold, and the baseline lane
/// exists precisely to report that end to end.
pub const FROZEN_GENERIC_VOCABULARY: &[&str] = &[
    "psoriasis",
    "eczema",
    "atopic dermatitis",
    "dermatitis",
    "hypertension",
    "depression",
    "anxiety",
    "diabetes",
    "glaucoma",
    "pain",
    "infection",
    "inflammation",
    "asthma",
    "epilepsy",
    "seizures",
    "arthritis",
    "migraine",
    "insomnia",
    "nausea",
    "hypothyroidism",
    "pregnancy",
    "renal impairment",
    "hepatic impairment",
    "heart failure",
    "constipation",
    "rosacea",
    "acne",
    "schizophrenia",
    "hypersensitivity",
    "bradycardia",
];

/// Which condition vocabulary the grounder is given. This choice is the single
/// most consequential decision in the measurement, so it is explicit and
/// reported alongside every score.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VocabularyLane {
    /// The shipped 30-term generic vocabulary. **The headline, end-to-end
    /// result**: concept discovery and trigger matching are both on trial.
    FrozenGeneric,
    /// A corpus-derived vocabulary, frozen before scoring and built without the
    /// reference documents. The improved-system result, still end-to-end.
    CorpusDerived,
    /// The gold's own concepts, handed to the grounder. **A DIAGNOSTIC ONLY.**
    /// It isolates trigger/relation quality by removing concept discovery from
    /// the problem, and must never be reported as end-to-end performance — it
    /// tells the grounder the answers.
    OracleDiagnostic,
}

impl VocabularyLane {
    pub fn label(&self) -> &'static str {
        match self {
            Self::FrozenGeneric => "frozen-generic (end-to-end; the shipped mechanism)",
            Self::CorpusDerived => "corpus-derived (end-to-end; improved system)",
            Self::OracleDiagnostic => {
                "oracle-vocabulary (DIAGNOSTIC ONLY — gold concepts supplied; \
                 conditional grounding, NOT end-to-end performance)"
            }
        }
    }

    pub fn is_end_to_end(&self) -> bool {
        !matches!(self, Self::OracleDiagnostic)
    }
}

/// Conditions offered to the grounder for one packet, per lane.
fn lane_conditions(
    lane: VocabularyLane,
    record: &AnnotationRecord,
    corpus: Option<&ClinicalVocabulary>,
) -> BTreeSet<String> {
    match lane {
        VocabularyLane::FrozenGeneric => FROZEN_GENERIC_VOCABULARY
            .iter()
            .map(|term| term.to_string())
            .collect(),
        VocabularyLane::CorpusDerived => corpus
            .map(|vocabulary| {
                vocabulary
                    .treats
                    .iter()
                    .chain(vocabulary.contraindicated_in.iter())
                    .cloned()
                    .collect()
            })
            .unwrap_or_default(),
        // The leak this lane exists to make explicit: the grounder is handed
        // the adjudicated concepts. Diagnostic value only.
        VocabularyLane::OracleDiagnostic => {
            record.facts.iter().map(|f| f.condition.clone()).collect()
        }
    }
}

pub fn score_grounding(
    packets: &[ReviewPacket],
    records: &[AnnotationRecord],
    lane: VocabularyLane,
    corpus: Option<&ClinicalVocabulary>,
) -> Result<ClinicalScore> {
    validate_annotations(packets, records)?;
    if lane == VocabularyLane::CorpusDerived && corpus.is_none() {
        anyhow::bail!("the corpus-derived lane requires a frozen vocabulary");
    }
    let record_map: BTreeMap<_, _> = records
        .iter()
        .map(|record| (record.set_id.as_str(), record))
        .collect();
    let mut score = ClinicalScore {
        lane: lane.label().to_string(),
        end_to_end: lane.is_end_to_end(),
        documents: packets.len(),
        true_positive: 0,
        false_negative: 0,
        true_negative: 0,
        false_positive: 0,
        uncertain_excluded: 0,
        recall: None,
        restraint: None,
        precision: None,
        by_relation: BTreeMap::new(),
    };
    for packet in packets {
        let record = record_map[&packet.set_id.as_str()];
        // The grounder is given ONLY this lane's vocabulary. In the end-to-end
        // lanes it has never seen the gold concepts, so a gold fact whose
        // concept is not in the vocabulary is a genuine miss — which is the
        // design gap the frozen-generic lane is meant to expose.
        let conditions = lane_conditions(lane, record, corpus);
        let space = clinical_space(&packet.product, &conditions);
        let text = packet
            .sections
            .iter()
            .map(|section| section.text.as_str())
            .collect::<Vec<_>>()
            .join("\n\n");
        let grounded: BTreeSet<_> = ground_chunk(&packet.set_id, &text, &space, &[])
            .iter()
            .map(|grounded| {
                (
                    grounded.fact.relation.to_uppercase(),
                    grounded.fact.target.to_lowercase(),
                )
            })
            .collect();
        for label in &record.facts {
            if label.verdict == ClinicalVerdict::Uncertain {
                score.uncertain_excluded += 1;
                continue;
            }
            let built = grounded.contains(&fact_key(label));
            let relation = score.by_relation.entry(label.relation.clone()).or_default();
            match (label.verdict, built) {
                (ClinicalVerdict::True, true) => {
                    score.true_positive += 1;
                    relation.true_positive += 1;
                }
                (ClinicalVerdict::True, false) => {
                    score.false_negative += 1;
                    relation.false_negative += 1;
                }
                (ClinicalVerdict::False, false) => {
                    score.true_negative += 1;
                    relation.true_negative += 1;
                }
                (ClinicalVerdict::False, true) => {
                    score.false_positive += 1;
                    relation.false_positive += 1;
                }
                (ClinicalVerdict::Unreviewed | ClinicalVerdict::Uncertain, _) => {}
            }
        }
    }
    score.recall = ratio(
        score.true_positive,
        score.true_positive + score.false_negative,
    );
    score.restraint = ratio(
        score.true_negative,
        score.true_negative + score.false_positive,
    );
    score.precision = ratio(
        score.true_positive,
        score.true_positive + score.false_positive,
    );
    Ok(score)
}

pub fn neuron_for_case(packet: &ReviewPacket, label: &ClinicalFactLabel) -> Neuron {
    Neuron {
        id: format!("clinical-reference:{}:{}", packet.set_id, label.condition),
        kind: NeuronKind::RelationHint,
        status: NeuronStatus::Proposed,
        confidence: 0.0,
        rationale: "Candidate clinical relation submitted for evidence review.".to_string(),
        evidence: vec![label.evidence.clone()],
        source: packet.product.clone(),
        relation: label.relation.clone(),
        target: label.condition.clone(),
        triggers: vec![label.evidence.clone()],
        ..Neuron::default()
    }
}

pub fn chunks_for_packet(packet: &ReviewPacket) -> Vec<Chunk> {
    packet
        .sections
        .iter()
        .map(|section| Chunk {
            id: format!("{}:{}", packet.set_id, section.code),
            title: section.title.clone(),
            text: section.text.clone(),
        })
        .collect()
}

pub fn finalize_judge_score(results: Vec<JudgeCaseResult>) -> JudgeScore {
    let mut score = JudgeScore {
        policy_threshold: JUDGE_POLICY_THRESHOLD,
        cases: results.len(),
        true_accepted: 0,
        true_rejected: 0,
        false_rejected: 0,
        false_accepted: 0,
        needs_human: 0,
        accepted_precision: None,
        results,
    };
    for result in &score.results {
        if result.verdict == "needs_human" {
            score.needs_human += 1;
        }
        match (result.gold, result.accepted) {
            (ClinicalVerdict::True, true) => score.true_accepted += 1,
            (ClinicalVerdict::True, false) => score.true_rejected += 1,
            (ClinicalVerdict::False, false) => score.false_rejected += 1,
            (ClinicalVerdict::False, true) => score.false_accepted += 1,
            _ => {}
        }
    }
    score.accepted_precision = ratio(
        score.true_accepted,
        score.true_accepted + score.false_accepted,
    );
    score
}

/// The (relation, concept) facts the construction mechanism ACTUALLY grounds on
/// one packet, given a vocabulary. Used both by scoring and by the ceiling
/// report, where it separates two very different losses: concepts the
/// vocabulary cannot express (the ceiling) versus concepts it can express but
/// whose trigger never fires (the shortfall below the ceiling).
pub fn grounded_concepts(
    packet: &ReviewPacket,
    conditions: &BTreeSet<String>,
) -> BTreeSet<(String, String)> {
    let space = clinical_space(&packet.product, conditions);
    let text = packet
        .sections
        .iter()
        .map(|section| section.text.as_str())
        .collect::<Vec<_>>()
        .join("\n\n");
    ground_chunk(&packet.set_id, &text, &space, &[])
        .iter()
        .map(|grounded| {
            (
                grounded.fact.relation.to_uppercase(),
                grounded.fact.target.to_lowercase(),
            )
        })
        .collect()
}

fn clinical_space(product: &str, conditions: &BTreeSet<String>) -> SpaceType {
    let mut entities = vec![json!({"name": product, "type": "product", "aliases": []})];
    let mut rules = Vec::new();
    for condition in conditions {
        entities.push(json!({"name": condition, "type": "condition", "aliases": []}));
        for (relation, triggers) in [
            (
                TREATS,
                vec![
                    "indicated for the treatment of {target}",
                    "indicated in the treatment of {target}",
                    "indicated for {target}",
                ],
            ),
            (
                CONTRAINDICATED_IN,
                vec![
                    "contraindicated in patients with {target}",
                    "contraindicated in {target}",
                ],
            ),
        ] {
            rules.push(json!({
                "source": product,
                "relation": relation,
                "target": condition,
                "when_any": triggers,
                "require_in_sentence": ["source"]
            }));
        }
    }
    serde_json::from_value(json!({
        "id": "dailymed-clinical-reference-v1",
        "entities": entities,
        "relation_rules": rules
    }))
    .expect("internally generated clinical space must deserialize")
}

fn ratio(numerator: usize, denominator: usize) -> Option<f64> {
    (denominator != 0).then(|| numerator as f64 / denominator as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn label(condition: &str, verdict: ClinicalVerdict, evidence: &str) -> ClinicalFactLabel {
        ClinicalFactLabel {
            relation: TREATS.to_string(),
            condition: condition.to_string(),
            verdict,
            section_code: "34067-9".to_string(),
            evidence: evidence.to_string(),
            notes: String::new(),
        }
    }

    #[test]
    fn judge_summary_uses_frozen_acceptance_policy() {
        let score = finalize_judge_score(vec![JudgeCaseResult {
            set_id: "x".to_string(),
            relation: TREATS.to_string(),
            condition: "depression".to_string(),
            gold: ClinicalVerdict::True,
            verdict: "accept".to_string(),
            confidence: 0.95,
            accepted: true,
            reasoning: String::new(),
        }]);
        assert_eq!(score.true_accepted, 1);
        assert_eq!(score.accepted_precision, Some(1.0));
    }

    #[test]
    fn grounding_score_counts_recall_and_restraint() {
        let affirmative = "Drug is indicated for the treatment of depression.";
        let negated = "Drug is not indicated for anxiety.";
        let facts = vec![
            label("depression", ClinicalVerdict::True, affirmative),
            label("anxiety", ClinicalVerdict::False, negated),
        ];
        let packet = ReviewPacket {
            schema_version: SCHEMA_VERSION.to_string(),
            set_id: "one".to_string(),
            spl_version: 1,
            selection_rank: 1,
            title: "Drug".to_string(),
            product: "Drug".to_string(),
            source_url: "https://example.test".to_string(),
            split: ReviewSplit::Evaluation,
            sections: vec![ClinicalSection {
                code: "34067-9".to_string(),
                title: String::new(),
                text: format!("{affirmative} {negated}"),
            }],
            candidates: facts
                .iter()
                .map(|fact| CandidateFact {
                    relation: fact.relation.clone(),
                    condition: fact.condition.clone(),
                    section_code: fact.section_code.clone(),
                    evidence: fact.evidence.clone(),
                })
                .collect(),
        };
        let record = AnnotationRecord {
            schema_version: SCHEMA_VERSION.to_string(),
            set_id: "one".to_string(),
            annotator: "expert".to_string(),
            complete: true,
            facts,
            notes: String::new(),
        };
        let score =
            score_grounding(&[packet], &[record], VocabularyLane::OracleDiagnostic, None).unwrap();
        assert!(!score.end_to_end, "oracle lane must never claim end-to-end");
        assert_eq!(score.true_positive, 1);
        assert_eq!(score.true_negative, 1);
        assert_eq!(score.recall, Some(1.0));
        assert_eq!(score.restraint, Some(1.0));
    }
}
