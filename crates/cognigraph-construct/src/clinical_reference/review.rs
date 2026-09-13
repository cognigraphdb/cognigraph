use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};

use super::model::*;
use super::prepare::write_jsonl;
use crate::{EvalQuestion, EvalSpec, Fact};

pub fn read_jsonl<T: serde::de::DeserializeOwned>(path: &Path) -> Result<Vec<T>> {
    fs::read_to_string(path)
        .with_context(|| format!("read {}", path.display()))?
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(index, line)| {
            serde_json::from_str(line)
                .with_context(|| format!("parse {} line {}", path.display(), index + 1))
        })
        .collect()
}

pub fn validate_annotations(packets: &[ReviewPacket], records: &[AnnotationRecord]) -> Result<()> {
    let packet_map: BTreeMap<_, _> = packets
        .iter()
        .map(|packet| (packet.set_id.as_str(), packet))
        .collect();
    if records.len() != packet_map.len() {
        bail!(
            "expected {} annotation records, found {}",
            packet_map.len(),
            records.len()
        );
    }
    let mut seen_docs = BTreeSet::new();
    for record in records {
        let packet = packet_map
            .get(record.set_id.as_str())
            .with_context(|| format!("annotation has unknown set_id {}", record.set_id))?;
        if !seen_docs.insert(record.set_id.as_str()) {
            bail!("duplicate annotation record for {}", record.set_id);
        }
        if record.schema_version != SCHEMA_VERSION {
            bail!("{} has unsupported schema version", record.set_id);
        }
        if record.annotator.trim().is_empty() {
            bail!("{} has an empty annotator", record.set_id);
        }
        if !record.complete {
            bail!("{} is not marked complete", record.set_id);
        }
        let mut seen_facts = BTreeSet::new();
        for fact in &record.facts {
            validate_fact(packet, fact)?;
            let key = fact_key(fact);
            if !seen_facts.insert(key) {
                bail!("{} contains a duplicate fact", record.set_id);
            }
        }
        for candidate in &packet.candidates {
            let key = candidate_key(candidate);
            if !seen_facts.contains(&key) {
                bail!(
                    "{} is missing review of candidate {} --{}",
                    record.set_id,
                    candidate.condition,
                    candidate.relation
                );
            }
        }
    }
    Ok(())
}

pub fn compare_annotations(
    packets: &[ReviewPacket],
    a: &[AnnotationRecord],
    b: &[AnnotationRecord],
    output: &Path,
) -> Result<(usize, usize)> {
    validate_annotations(packets, a)?;
    validate_annotations(packets, b)?;
    let b_by_doc: BTreeMap<_, _> = b.iter().map(|r| (r.set_id.as_str(), r)).collect();
    let mut agreements = 0;
    let mut disagreements = 0;
    let mut merged = Vec::new();
    for left in a {
        let right = b_by_doc[&left.set_id.as_str()];
        let left_facts: BTreeMap<_, _> = left.facts.iter().map(|f| (fact_key(f), f)).collect();
        let right_facts: BTreeMap<_, _> = right.facts.iter().map(|f| (fact_key(f), f)).collect();
        let keys: BTreeSet<_> = left_facts
            .keys()
            .chain(right_facts.keys())
            .cloned()
            .collect();
        let mut facts = Vec::new();
        let mut doc_complete = true;
        for key in keys {
            match (left_facts.get(&key), right_facts.get(&key)) {
                (Some(l), Some(r)) if l.verdict == r.verdict => {
                    agreements += 1;
                    facts.push((*l).clone());
                }
                (l, r) => {
                    disagreements += 1;
                    doc_complete = false;
                    let template = l.or(r).expect("key came from at least one fact");
                    let mut fact = (*template).clone();
                    fact.verdict = ClinicalVerdict::Unreviewed;
                    fact.notes = format!(
                        "ADJUDICATE: expert-a={}; expert-b={}",
                        verdict_name(l.map(|f| f.verdict)),
                        verdict_name(r.map(|f| f.verdict))
                    );
                    facts.push(fact);
                }
            }
        }
        merged.push(AnnotationRecord {
            schema_version: SCHEMA_VERSION.to_string(),
            set_id: left.set_id.clone(),
            annotator: "adjudicator".to_string(),
            complete: doc_complete,
            facts,
            notes: if doc_complete {
                "Independent reviewers agreed on all recorded facts.".to_string()
            } else {
                "Resolve every ADJUDICATE item, then set complete=true.".to_string()
            },
        });
    }
    write_jsonl(output, &merged)?;
    Ok((agreements, disagreements))
}

pub fn compile_eval_spec(
    packets: &[ReviewPacket],
    records: &[AnnotationRecord],
) -> Result<EvalSpec> {
    validate_annotations(packets, records)?;
    let record_map: BTreeMap<_, _> = records
        .iter()
        .map(|record| (record.set_id.as_str(), record))
        .collect();
    let questions = packets
        .iter()
        .map(|packet| {
            let mut expected_facts = Vec::new();
            let mut forbidden_facts = Vec::new();
            for label in &record_map[&packet.set_id.as_str()].facts {
                let fact = Fact {
                    source: packet.product.clone(),
                    relation: label.relation.clone(),
                    target: label.condition.clone(),
                };
                match label.verdict {
                    ClinicalVerdict::True => expected_facts.push(format_fact(&fact)),
                    ClinicalVerdict::False => forbidden_facts.push(format_fact(&fact)),
                    ClinicalVerdict::Uncertain => {}
                    ClinicalVerdict::Unreviewed => unreachable!("validation rejects unreviewed"),
                }
            }
            EvalQuestion {
                id: packet.set_id.clone(),
                question: format!(
                    "Clinical relations supported by DailyMed label {}",
                    packet.set_id
                ),
                expected_facts,
                forbidden_facts,
            }
        })
        .collect();
    Ok(EvalSpec {
        space_id: "dailymed-clinical-reference-v1".to_string(),
        questions,
    })
}

fn validate_fact(packet: &ReviewPacket, fact: &ClinicalFactLabel) -> Result<()> {
    if fact.relation != TREATS && fact.relation != CONTRAINDICATED_IN {
        bail!(
            "{} has unsupported relation {}",
            packet.set_id,
            fact.relation
        );
    }
    if fact.condition.trim().is_empty() {
        bail!("{} has an empty condition", packet.set_id);
    }
    if fact.verdict == ClinicalVerdict::Unreviewed {
        bail!("{} still contains an unreviewed fact", packet.set_id);
    }
    if fact.evidence.trim().is_empty() {
        bail!("{} has empty evidence", packet.set_id);
    }
    let section = packet
        .sections
        .iter()
        .find(|section| section.code == fact.section_code)
        .with_context(|| {
            format!(
                "{} references absent section {}",
                packet.set_id, fact.section_code
            )
        })?;
    if !section.text.contains(&fact.evidence) {
        bail!(
            "{} evidence is not an exact section substring",
            packet.set_id
        );
    }
    Ok(())
}

pub(crate) fn fact_key(fact: &ClinicalFactLabel) -> (String, String) {
    (
        fact.relation.trim().to_uppercase(),
        fact.condition.trim().to_lowercase(),
    )
}

fn candidate_key(fact: &CandidateFact) -> (String, String) {
    (
        fact.relation.trim().to_uppercase(),
        fact.condition.trim().to_lowercase(),
    )
}

fn verdict_name(verdict: Option<ClinicalVerdict>) -> &'static str {
    match verdict {
        Some(ClinicalVerdict::Unreviewed) => "unreviewed",
        Some(ClinicalVerdict::True) => "true",
        Some(ClinicalVerdict::False) => "false",
        Some(ClinicalVerdict::Uncertain) => "uncertain",
        None => "missing",
    }
}

fn format_fact(fact: &Fact) -> String {
    format!("{} --{}--> {}", fact.source, fact.relation, fact.target)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn packet() -> ReviewPacket {
        ReviewPacket {
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
                text: "Drug is indicated for depression.".to_string(),
            }],
            candidates: vec![CandidateFact {
                relation: TREATS.to_string(),
                condition: "depression".to_string(),
                section_code: "34067-9".to_string(),
                evidence: "Drug is indicated for depression.".to_string(),
            }],
        }
    }

    fn record(verdict: ClinicalVerdict) -> AnnotationRecord {
        AnnotationRecord {
            schema_version: SCHEMA_VERSION.to_string(),
            set_id: "one".to_string(),
            annotator: "expert".to_string(),
            complete: true,
            facts: vec![ClinicalFactLabel {
                relation: TREATS.to_string(),
                condition: "depression".to_string(),
                verdict,
                section_code: "34067-9".to_string(),
                evidence: "Drug is indicated for depression.".to_string(),
                notes: String::new(),
            }],
            notes: String::new(),
        }
    }

    #[test]
    fn validation_accepts_complete_exact_evidence() {
        validate_annotations(&[packet()], &[record(ClinicalVerdict::True)]).unwrap();
    }

    #[test]
    fn validation_rejects_unreviewed() {
        let error =
            validate_annotations(&[packet()], &[record(ClinicalVerdict::Unreviewed)]).unwrap_err();
        assert!(error.to_string().contains("unreviewed"));
    }
}
