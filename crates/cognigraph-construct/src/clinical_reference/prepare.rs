use std::collections::HashSet;
use std::fs;
use std::path::Path;

/// R1 requires qualified concepts, and qualified concepts are long: "severe or
/// incapacitating atopic dermatitis intractable to adequate trials of
/// conventional treatment" is 99 characters. The previous 90-char cap silently
/// deleted exactly the qualifiers the rule exists to preserve.
pub(crate) const CONCEPT_MAX_CHARS: usize = 140;

use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};

use super::model::*;

pub(crate) const INDICATIONS: &str = "34067-9";
const CONTRAINDICATIONS: &str = "34070-3";

/// Cues that introduce an indication concept (frozen rule R2: adjunctive use
/// counts as treatment).
const TREATS_CUES: &[&str] = &[
    "indicated for the treatment of",
    "indicated in the treatment of",
    "indicated for the management of",
    "indicated in the management of",
    "indicated for the relief of",
    "indicated for the prevention of",
    "indicated as adjunctive therapy for the treatment of",
    "indicated as adjunctive therapy in",
    "indicated as adjunctive therapy for",
    "indicated in the prophylaxis of",
    "indicated for prophylaxis of",
    "adjunctive therapy for the treatment of",
    "adjunctive therapy in",
    "indicated for",
    "indicated in",
    "management of",
    "treatment of",
    "prevention of",
    "prophylaxis of",
    "relief of",
    "control of",
];

/// Cues that introduce a contraindication concept. Frozen rule R4: a
/// contraindication is decided on clinical meaning, never on phrasing, so
/// "should not be used in" counts, and a bare list under CONTRAINDICATIONS
/// counts even with no cue at all.
const CONTRA_CUES: &[&str] = &[
    "contraindicated in patients with",
    "contraindicated in patients who",
    "contraindicated in individuals with",
    "contraindicated in persons who have shown",
    "contraindicated in persons with",
    "contraindicated for use in",
    "contraindicated in",
    "contraindicated for",
    "should not be used in patients with",
    "should not be used in patients who",
    "should not be used in",
    "should not be given to patients with",
    "is contraindicated",
    "are contraindicated",
];

/// Markers that end a concept span: what follows is population, purpose, or
/// cross-reference, not part of the clinical concept.
const CLAUSE_CUTS: &[&str] = &[
    " in patients",
    " in adults",
    " in children",
    " in pediatric",
    " in adolescents",
    " in individuals",
    " in persons",
    " to lower",
    " to reduce",
    " to enhance",
    " to tide",
    " who ",
    " when ",
    " see ",
    " as an adjunct",
    " because ",
    // Prose/modal markers: what follows is commentary, not a concept.
    " should ",
    " is most",
    " are most",
    " have occurred",
    " may contribute",
    " that are proven",
];

pub fn prepare_reference(
    corpus: &Path,
    output: &Path,
    documents: usize,
    calibration: usize,
    seed: &str,
) -> Result<Vec<ReviewPacket>> {
    if documents == 0 {
        bail!("--documents must be greater than zero");
    }
    if calibration > documents {
        bail!("--calibration cannot exceed --documents");
    }
    if output.exists() {
        bail!(
            "output {} already exists; refusing to overwrite expert work",
            output.display()
        );
    }

    let manifest_path = corpus.join("manifest-rx.jsonl");
    let mut eligible = Vec::new();
    for (line_no, line) in fs::read_to_string(&manifest_path)
        .with_context(|| format!("read {}", manifest_path.display()))?
        .lines()
        .enumerate()
    {
        if line.trim().is_empty() {
            continue;
        }
        let entry: ManifestEntry = serde_json::from_str(line)
            .with_context(|| format!("parse manifest line {}", line_no + 1))?;
        let doc_path = corpus.join(&entry.document_path);
        let doc: NormalizedDoc = serde_json::from_str(
            &fs::read_to_string(&doc_path)
                .with_context(|| format!("read {}", doc_path.display()))?,
        )?;
        let sections: Vec<_> = doc
            .sections
            .into_iter()
            .filter(|section| section.code == INDICATIONS || section.code == CONTRAINDICATIONS)
            .collect();
        if sections.is_empty() {
            continue;
        }
        let digest = Sha256::digest(format!("{seed}:{}", entry.set_id).as_bytes());
        eligible.push((digest.to_vec(), entry, sections));
    }
    eligible.sort_by(|a, b| a.0.cmp(&b.0));
    if eligible.len() < documents {
        bail!(
            "requested {documents} documents but only {} contain relevant sections",
            eligible.len()
        );
    }

    let mut packets = Vec::with_capacity(documents);
    for (index, (_, entry, sections)) in eligible.into_iter().take(documents).enumerate() {
        let product = product_from_title(&entry.title);
        let candidates = candidate_facts(&sections);
        packets.push(ReviewPacket {
            schema_version: SCHEMA_VERSION.to_string(),
            set_id: entry.set_id,
            spl_version: entry.spl_version,
            selection_rank: entry.selection_rank,
            title: entry.title,
            product,
            source_url: entry.source_url,
            split: if index < calibration {
                ReviewSplit::Calibration
            } else {
                ReviewSplit::Evaluation
            },
            sections,
            candidates,
        });
    }

    fs::create_dir_all(output.join("annotations"))?;
    write_jsonl(&output.join("packets.jsonl"), &packets)?;
    for annotator in ["expert-a", "expert-b"] {
        let records: Vec<_> = packets
            .iter()
            .map(|packet| AnnotationRecord {
                schema_version: SCHEMA_VERSION.to_string(),
                set_id: packet.set_id.clone(),
                annotator: annotator.to_string(),
                complete: false,
                facts: packet
                    .candidates
                    .iter()
                    .map(|candidate| ClinicalFactLabel {
                        relation: candidate.relation.clone(),
                        condition: candidate.condition.clone(),
                        verdict: ClinicalVerdict::Unreviewed,
                        section_code: candidate.section_code.clone(),
                        evidence: candidate.evidence.clone(),
                        notes: String::new(),
                    })
                    .collect(),
                notes: String::new(),
            })
            .collect();
        write_jsonl(
            &output
                .join("annotations")
                .join(format!("{annotator}.jsonl")),
            &records,
        )?;
    }
    fs::write(output.join("ANNOTATION_GUIDE.md"), annotation_guide())?;
    Ok(packets)
}

/// Refuse to run against a workspace the CURRENT extractor would not produce.
///
/// This exists because of a real failure: phase-2 changed the extractor (R6
/// distribution, the concept length cap), but the prepared workspace on disk was
/// never regenerated. Every downstream artifact was then computed against a
/// STALE candidate set — 314 candidates when the code produced 334 — while being
/// described as "rebuilt from nothing". A from-scratch rebuild that silently
/// skips the first step is not a from-scratch rebuild.
pub fn assert_workspace_fresh(packets: &[ReviewPacket]) -> Result<()> {
    for packet in packets {
        let expected = candidate_facts(&packet.sections);
        if expected.len() != packet.candidates.len()
            || expected
                .iter()
                .zip(&packet.candidates)
                .any(|(a, b)| a.relation != b.relation || a.condition != b.condition)
        {
            bail!(
                "workspace is STALE: document {} holds {} candidates but the current extractor \
                 produces {}. The prepared workspace predates a code change; re-run `prepare` \
                 (it holds no expert work until reviewers label it).",
                packet.set_id,
                packet.candidates.len(),
                expected.len()
            );
        }
    }
    Ok(())
}

pub fn product_from_title(title: &str) -> String {
    title
        .rfind('[')
        .map(|index| title[..index].trim())
        .unwrap_or(title.trim())
        .to_string()
}

/// Surface the label's OWN concept phrases (frozen rule R1: the reference is
/// concept-preserving, so candidates must carry every essential qualifier —
/// subtype, severity, anatomy, timing, causative drug). A generic condition
/// vocabulary cannot express these and is deliberately NOT used: calibration
/// showed it surfaced lossy broad terms that are almost all FALSE while
/// missing nearly every real concept.
///
/// This surfaces candidates for review; it never assigns a verdict, and no
/// LLM is involved (an LLM proposer would shape the reference by omission).
/// Imperfect spans are acceptable — reviewers reject or restate them, and the
/// exhaustiveness rule still requires reading the full section.
pub(crate) fn candidate_facts(sections: &[ClinicalSection]) -> Vec<CandidateFact> {
    let mut seen = HashSet::new();
    let mut candidates = Vec::new();
    for section in sections {
        let (relation, cues, bare_list) = match section.code.as_str() {
            INDICATIONS => (TREATS, TREATS_CUES, false),
            // R4: a bare list under CONTRAINDICATIONS is a contraindication.
            CONTRAINDICATIONS => (CONTRAINDICATED_IN, CONTRA_CUES, true),
            _ => continue,
        };
        for unit in units(&section.text) {
            for concept in concepts_in(&unit, cues, bare_list) {
                if seen.insert((relation, concept.clone())) {
                    candidates.push(CandidateFact {
                        relation: relation.to_string(),
                        condition: concept,
                        section_code: section.code.clone(),
                        evidence: unit.clone(),
                    });
                }
            }
        }
    }
    candidates
}

/// Split a section into review units: bullet items and sentences. Evidence
/// must be an exact substring of the section, so units are taken verbatim.
pub(crate) fn units(text: &str) -> Vec<String> {
    text.split('•')
        .flat_map(|part| part.split_inclusive(['.', '!', '?', ';']))
        .map(|unit| unit.trim().to_string())
        .filter(|unit| unit.len() > 8)
        .collect()
}

/// Extract concept phrases from one unit: find a cue, take the span after it,
/// cut trailing population/purpose clauses, then split coordinated lists.
pub(crate) fn concepts_in(unit: &str, cues: &[&str], bare_list: bool) -> Vec<String> {
    let lower = unit.to_lowercase();
    // The LONGEST matching cue wins (earliest position breaks ties). Matching
    // the earliest cue instead would let a short greedy cue like "are
    // contraindicated" beat "contraindicated in patients with", leaving the
    // span starting with a dangling "in patients with:".
    let span = match cues
        .iter()
        .filter_map(|cue| lower.find(cue).map(|at| (cue.len(), at, at + cue.len())))
        .max_by_key(|(len, at, _)| (*len, std::cmp::Reverse(*at)))
    {
        Some((_, _, after)) => &lower[after..],
        // No cue: only a CONTRAINDICATIONS section licenses a bare list (R4).
        None if bare_list => lower.as_str(),
        None => return Vec::new(),
    };
    let mut span = span.trim();
    // Belt-and-braces: a population preamble is never part of the concept.
    for prefix in [
        "in patients with:",
        "in patients with",
        "in patients who have",
        "in patients who",
        "in patients",
        "patients with:",
        "patients with",
        "persons who have shown",
        "individuals with",
        "the following:",
        ":",
    ] {
        if let Some(rest) = span.strip_prefix(prefix) {
            span = rest.trim();
        }
    }
    for cut in CLAUSE_CUTS {
        if let Some(at) = span.find(cut) {
            span = span[..at].trim();
        }
    }
    // Split ONLY on commas/semicolons. Splitting on " and "/" or " would tear
    // coordinated modifiers off their head noun ("moderate and severe
    // hypertension" -> "moderate" + "severe hypertension"), destroying the
    // very qualifiers rule R1 requires us to preserve.
    // R6, applied here as well as in the matcher: a shared refractory
    // restriction scopes over EVERY item of the coordinated list, so the plain
    // item is never a concept — only the qualified one is. Without this, the
    // vocabulary (which is built from this extractor) could never contain the
    // qualified concept, and the matcher may only assert vocabulary it was
    // given. The two modules implement the same authored rule independently;
    // they deliberately do not share a parser.
    const REFRACTORY: &str = "severe or incapacitating allergic conditions intractable to \
                              adequate trials of conventional treatment in ";
    if let Some(items) = span.strip_prefix(REFRACTORY) {
        return items
            .split([',', ';'])
            .filter_map(clean_concept)
            .map(|item| {
                format!(
                    "severe or incapacitating {item} intractable to adequate trials of \
                     conventional treatment"
                )
            })
            .collect();
    }

    span.split([',', ';']).filter_map(clean_concept).collect()
}

/// Normalize one concept fragment. Rule R1: "known" may be dropped (it does
/// not alter the condition); every other qualifier is essential and kept.
fn clean_concept(fragment: &str) -> Option<String> {
    let mut concept = fragment.trim();
    // Parentheticals and bracketed cross-references are never part of the
    // concept ("symptomatic orthostatic hypotension (OH)" -> the hypotension).
    if let Some(at) = concept.find(['(', '[']) {
        concept = concept[..at].trim();
    }
    concept = concept.trim_matches(|c: char| {
        c == '.' || c == ',' || c == ':' || c == ')' || c == ']' || c == '"' || c == '-'
    });
    concept = concept.trim();
    // R1: "known" does not alter the condition and may be dropped. Leading
    // conjunctions and section labels are artifacts of list splitting.
    for lead in [
        "known ",
        "and ",
        "or ",
        "the ",
        "a ",
        "an ",
        "other ",
        "including ",
        "contraindications: ",
        "contraindications ",
        "such as ",
        "e.g. ",
        "with ",
        "to ",
    ] {
        if let Some(rest) = concept.strip_prefix(lead) {
            concept = rest.trim();
        }
    }
    // Drop dangling conjunctions/prepositions left by clause cutting
    // ("hypertension either as the sole therapeutic agent or" -> drop the tail).
    loop {
        let trimmed = [
            " or", " and", " either", " as", " to", " with", " in", " for", " of", " the", " a",
        ]
        .iter()
        .find_map(|tail| concept.strip_suffix(tail));
        match trimmed {
            Some(rest) => concept = rest.trim_end_matches([' ', ',']),
            None => break,
        }
    }
    if concept.len() < 4 || concept.len() > CONCEPT_MAX_CHARS {
        return None;
    }
    // Fragments that are not clinical concepts.
    const JUNK: &[&str] = &[
        "its components",
        "any of its components",
        "this product",
        "this drug",
        "use",
        "patients",
        "therapy",
        "treatment",
        "conventional treatment",
        "adequate trials",
        "in patients",
        "in persons",
        "monotherapy only",
        "use as monotherapy only",
        "as appropriate",
        "exercise",
        "oral solution products",
    ];
    if JUNK.contains(&concept) || !concept.chars().any(|c| c.is_alphabetic()) {
        return None;
    }
    Some(concept.to_string())
}

pub(crate) fn write_jsonl<T: serde::Serialize>(path: &Path, values: &[T]) -> Result<()> {
    let mut body = String::new();
    for value in values {
        body.push_str(&serde_json::to_string(value)?);
        body.push('\n');
    }
    fs::write(path, body).with_context(|| format!("write {}", path.display()))
}

fn annotation_guide() -> &'static str {
    r#"# DailyMed clinical reference annotation guide

This is a blinded, two-reviewer reference set. Do not inspect CogniGraph output or judge results while labeling. Review the calibration split together, then label the evaluation split independently.

For each packet, exhaustively record facts expressed by the included sections:

- `TREATS`: the product is indicated for treatment, prevention, management, or relief of the condition.
- `CONTRAINDICATED_IN`: the product is contraindicated for the condition or patient population.
- `true`: the relation is directly supported; `false`: negation, warning, monitoring instruction, differential co-mention, bare mention, boilerplate, or another therapy's fact; `uncertain`: adjudication required.
- Evidence must be an exact, non-empty substring of the named section.
- Candidates are surfaced mechanically from the label's own wording. They are NOT pre-labeled and are frequently imprecise — reject or restate them freely, and add every fact they missed. The section, not the candidate list, is the unit of review.
- Set `complete` to `true` only after every candidate is decided and the full section has been read for additional facts.

## Frozen calibration rules (2026-07-14)

**R1 — Concept preservation (governing).** Drop a qualifier only when it does not change the concept's extension. Subtype, severity, anatomy, timing, causative drug, and treatment combination are ESSENTIAL and must be kept. A lossy broad candidate is `false`; add the exact qualified concept as `true`.
  - systemic fungal infection != infection; moderate/severe hypertension != hypertension; supine hypertension != hypertension; partial-onset seizures != seizures; hypersensitivity to pregabalin != hypersensitivity.
  - "Known" may be dropped ("known hypersensitivity to pregabalin" -> "hypersensitivity to pregabalin"); the allergen may not.
  - Lost generic-vocabulary recall is a modeling limitation, never a reason to broaden a gold label.

**R2 — Adjunctive therapy counts as treatment**, but the target keeps its specific subtype.

**R3 — Treated target, not associated context.** "Adjunctive therapy in edema associated with congestive heart failure" is TREATS *edema associated with CHF*, not TREATS *heart failure*.

**R4 — Contraindications are decided on clinical meaning, never phrasing.** Bare list items under CONTRAINDICATIONS and "should not be used in ..." are valid contraindications. Never shape a label around whether CogniGraph's trigger would fire.

**R5 — Combination contraindications bind the combination.** "Do not co-administer aliskiren with olmesartan in patients with diabetes" does not make diabetes a contraindication for olmesartan alone.

**R6 — Restricted/refractory-only indications** do not support a plain TREATS; add the qualified concept (e.g. severe refractory disease) as `true` instead.

**R7 — Boilerplate is not an indication.** Antibiotic-stewardship language is not licensing evidence; find the real indication in the section.

Never use an LLM to decide a gold label.
"#
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_product_and_candidate_evidence() {
        assert_eq!(product_from_title("DRUG TABLET [LABELER]"), "DRUG TABLET");
        let candidates = candidate_facts(&[ClinicalSection {
            code: INDICATIONS.to_string(),
            title: String::new(),
            text: "Other context. This drug is indicated for depression.".to_string(),
        }]);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].condition, "depression");
        assert_eq!(
            candidates[0].evidence,
            "This drug is indicated for depression."
        );
    }

    /// R1: candidates must carry essential qualifiers, and coordinated lists
    /// must be split into separate concepts — a generic term like "seizures"
    /// or "infection" must never be surfaced in place of the real concept.
    #[test]
    fn surfaces_qualified_concepts_not_generic_terms() {
        let candidates = candidate_facts(&[ClinicalSection {
            code: INDICATIONS.to_string(),
            title: String::new(),
            text: "• Adjunctive therapy for the treatment of partial-onset seizures \
                   • Management of fibromyalgia"
                .to_string(),
        }]);
        let conditions: Vec<_> = candidates.iter().map(|c| c.condition.as_str()).collect();
        assert!(
            conditions.contains(&"partial-onset seizures"),
            "{conditions:?}"
        );
        assert!(conditions.contains(&"fibromyalgia"), "{conditions:?}");
        assert!(!conditions.contains(&"seizures"), "generic term leaked");
    }

    /// R4: a bare list under CONTRAINDICATIONS is a contraindication even with
    /// no "contraindicated" verb; each list item is its own concept, with the
    /// severity qualifier preserved (R1).
    #[test]
    fn surfaces_bare_contraindication_list_items() {
        let candidates = candidate_facts(&[ClinicalSection {
            code: CONTRAINDICATIONS.to_string(),
            title: String::new(),
            text: "Advanced arteriosclerosis, moderate and severe hypertension, and glaucoma."
                .to_string(),
        }]);
        let conditions: Vec<_> = candidates.iter().map(|c| c.condition.as_str()).collect();
        assert!(conditions.contains(&"glaucoma"), "{conditions:?}");
        assert!(
            conditions.contains(&"advanced arteriosclerosis"),
            "{conditions:?}"
        );
        // "moderate and severe hypertension" splits on " and "; both halves must
        // still be surfaced as qualified concepts, never as bare "hypertension".
        assert!(
            conditions.iter().any(|c| c.contains("severe hypertension")),
            "{conditions:?}"
        );
        assert!(!conditions.contains(&"hypertension"), "generic term leaked");
    }

    /// Evidence must be an exact substring of the section (the validator
    /// enforces this; the extractor must not violate it).
    #[test]
    fn candidate_evidence_is_an_exact_section_substring() {
        let text = "• Management of neuropathic pain associated with diabetic peripheral \
                    neuropathy • Management of fibromyalgia"
            .to_string();
        let candidates = candidate_facts(&[ClinicalSection {
            code: INDICATIONS.to_string(),
            title: String::new(),
            text: text.clone(),
        }]);
        assert!(!candidates.is_empty());
        for candidate in &candidates {
            assert!(
                text.contains(&candidate.evidence),
                "evidence not a substring: {:?}",
                candidate.evidence
            );
        }
    }
}
