//! Corpus-derived clinical vocabulary.
//!
//! Calibration showed the hand-picked 30-term generic condition vocabulary
//! cannot express a concept-preserving clinical gold (only 3 of 24 surfaced
//! candidates were TRUE; ~20 required concepts were never surfaced). The fix
//! is to derive the vocabulary from the corpus's own indication and
//! contraindication wording, at the granularity the gold actually uses.
//!
//! Two integrity rules govern this, both load-bearing:
//!
//! 1. **It is derived from label TEXT, never from gold labels.** The reference
//!    verdicts play no part; the same deterministic concept extractor used to
//!    surface review candidates is reused here.
//! 2. **The 50 reference documents are EXCLUDED.** A vocabulary fitted to the
//!    evaluation set would score the mechanism on concepts lifted from the
//!    documents it is about to be tested on. Excluding them makes the held-out
//!    evaluation a genuine test of whether a corpus-bootstrapped vocabulary
//!    generalizes to unseen labels.
//!
//! The artifact is frozen (content-hashed) BEFORE any scoring runs.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::model::*;
use super::prepare::{INDICATIONS, candidate_facts};

/// A concept must appear in at least this many DISTINCT documents to enter the
/// vocabulary. Document frequency (not raw count) so one verbose label cannot
/// inject its idiosyncratic phrasing.
pub const DEFAULT_MIN_DOCUMENT_FREQUENCY: usize = 3;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClinicalVocabulary {
    pub version: String,
    /// How it was built — recorded so a reader can never mistake this for a
    /// gold-derived (oracle) vocabulary.
    pub provenance: String,
    pub min_document_frequency: usize,
    pub source_documents: usize,
    pub excluded_documents: usize,
    pub treats: Vec<String>,
    pub contraindicated_in: Vec<String>,
    /// Content hash of the sorted term lists — the freeze.
    pub digest: String,
}

impl ClinicalVocabulary {
    pub fn terms(&self, relation: &str) -> &[String] {
        if relation == TREATS {
            &self.treats
        } else {
            &self.contraindicated_in
        }
    }

    pub fn len(&self) -> usize {
        self.treats.len() + self.contraindicated_in.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn load(path: &Path) -> Result<Self> {
        let body = fs::read_to_string(path)
            .with_context(|| format!("read frozen vocabulary {}", path.display()))?;
        let vocabulary: Self = serde_json::from_str(&body)?;
        let expected = digest_of(&vocabulary.treats, &vocabulary.contraindicated_in);
        if expected != vocabulary.digest {
            bail!(
                "frozen vocabulary {} failed its integrity check (expected {expected}, found {})",
                path.display(),
                vocabulary.digest
            );
        }
        Ok(vocabulary)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, serde_json::to_string_pretty(self)?)
            .with_context(|| format!("write {}", path.display()))
    }
}

fn digest_of(treats: &[String], contra: &[String]) -> String {
    let mut hasher = Sha256::new();
    for term in treats {
        hasher.update(b"T:");
        hasher.update(term.as_bytes());
        hasher.update(b"\n");
    }
    for term in contra {
        hasher.update(b"C:");
        hasher.update(term.as_bytes());
        hasher.update(b"\n");
    }
    let out = hasher.finalize();
    out.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Build the vocabulary from the corpus's own wording, excluding the reference
/// documents so the held-out evaluation stays clean.
pub fn build_vocabulary(
    corpus: &Path,
    excluded: &BTreeSet<String>,
    min_document_frequency: usize,
    drugs: &BTreeSet<String>,
) -> Result<ClinicalVocabulary> {
    if min_document_frequency == 0 {
        bail!("--min-df must be greater than zero");
    }
    let manifest_path = corpus.join("manifest-rx.jsonl");
    let manifest = fs::read_to_string(&manifest_path)
        .with_context(|| format!("read {}", manifest_path.display()))?;

    // Document frequency per (relation, concept).
    let mut frequency: BTreeMap<(String, String), usize> = BTreeMap::new();
    let (mut used, mut skipped) = (0usize, 0usize);

    for line in manifest.lines().filter(|line| !line.trim().is_empty()) {
        let entry: ManifestEntry = serde_json::from_str(line)?;
        if excluded.contains(&entry.set_id) {
            skipped += 1;
            continue;
        }
        let doc_path = corpus.join(&entry.document_path);
        let Ok(body) = fs::read_to_string(&doc_path) else {
            continue;
        };
        let doc: NormalizedDoc = serde_json::from_str(&body)?;
        let sections: Vec<ClinicalSection> = doc
            .sections
            .into_iter()
            .filter(|section| section.code == INDICATIONS || section.code == CONTRAINDICATIONS)
            .collect();
        if sections.is_empty() {
            continue;
        }
        used += 1;
        // Count each concept once per document.
        let mut here: BTreeSet<(String, String)> = BTreeSet::new();
        for candidate in candidate_facts(&sections) {
            here.insert((candidate.relation, candidate.condition));
        }
        for key in here {
            *frequency.entry(key).or_default() += 1;
        }
    }

    let mut treats: Vec<String> = Vec::new();
    let mut contraindicated_in: Vec<String> = Vec::new();
    for ((relation, concept), count) in frequency {
        if count < min_document_frequency {
            continue;
        }
        // PHASE 2: a term that is a substance, a clause, or a bare modifier is
        // not a condition and must never enter the vocabulary — otherwise the
        // matcher's bare-list path would happily assert `doxazosin` as a
        // contraindicated condition.
        if !super::typing::is_condition(&concept, drugs) {
            continue;
        }
        if relation == TREATS {
            treats.push(concept);
        } else {
            contraindicated_in.push(concept);
        }
    }
    treats.sort();
    treats.dedup();
    contraindicated_in.sort();
    contraindicated_in.dedup();

    let digest = digest_of(&treats, &contraindicated_in);
    Ok(ClinicalVocabulary {
        version: "dailymed-clinical-vocabulary-v2".to_string(),
        provenance: format!(
            "corpus-derived from DailyMed indication/contraindication wording; \
             gold labels NOT used; {} reference documents excluded; \
             concept must appear in >= {min_document_frequency} distinct documents; \
             condition-TYPED (substances, clauses and bare modifiers removed)",
            excluded.len()
        ),
        min_document_frequency,
        source_documents: used,
        excluded_documents: skipped,
        treats,
        contraindicated_in,
        digest,
    })
}

const CONTRAINDICATIONS: &str = "34070-3";

/// The three losses, measured as FACT INSTANCES — `(set_id, relation, concept)`
/// — never as globally deduplicated concept strings, and always against the
/// vocabulary **for that relation**. Deduplicating concepts across documents
/// silently merges a concept asserted by ten labels into one, which flatters
/// whichever stage you are defending.
///
/// The three losses are distinct bugs and must never be summed or conflated:
///  1. **vocabulary loss** — the concept is not in the relation's vocabulary, so
///     nothing could ever assert it.
///  2. **trigger/structure loss** — the vocabulary has it, but no clinical
///     assertion matched the label's wording.
///  3. **extractor/type noise** — the "concept" is not a condition at all (a
///     drug name or prose fragment from the bare-list path), so it pollutes the
///     denominator and must be reported, not hidden.
///
/// NONE of these is recall. There is no gold here.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LossReport {
    pub matcher_version: String,
    pub caveat: String,
    /// Fact instances the extractor surfaced. NOT gold — the denominator is
    /// candidates, some of which are noise (see `type_noise_instances`).
    pub candidate_instances: usize,
    pub vocabulary_expressible: usize,
    pub vocabulary_loss: usize,
    /// Candidate instances the matcher asserted. THIS is the partition member:
    /// asserted_candidates + vocabulary_loss + trigger_structure_loss +
    /// type_noise_instances == candidate_instances.
    pub asserted_candidates: usize,
    /// TOTAL assertions the matcher made — NOT a partition member. It exceeds
    /// `asserted_candidates` by the facts the matcher found that the reference
    /// extractor never surfaced (the two do not share a parser). Summing this
    /// with the losses is arithmetically wrong.
    pub asserted_by_matcher: usize,
    pub asserted_outside_candidates: usize,
    pub trigger_structure_loss: usize,
    pub type_noise_instances: usize,
    /// Project-owner-accepted development-showcase calibration result.
    pub calibration_true_total: usize,
    pub calibration_true_in_vocabulary: usize,
    pub calibration_true_asserted: usize,
    pub calibration_true_missed: Vec<String>,
}

/// Partition every candidate fact instance into exactly ONE bucket, so the three
/// losses are a true partition rather than overlapping tallies. (Before this,
/// a junk term that condition-typing had removed from the vocabulary was counted
/// as "vocabulary loss", conflating a *typing* bug with a *coverage* bug.)
///
/// Order matters and encodes the causal question "why was this not asserted?":
///   1. it is not a condition at all       -> type noise
///   2. it is, but the vocabulary lacks it -> vocabulary loss
///   3. vocabulary has it, but no assertion matched -> trigger/structure loss
///   4. otherwise -> asserted
#[allow(clippy::too_many_arguments)]
pub fn compute_losses(
    candidate_instances: &BTreeSet<(String, String, String)>,
    asserted: &BTreeSet<(String, String, String)>,
    vocabulary: &ClinicalVocabulary,
    drugs: &BTreeSet<String>,
    calibration_true_total: usize,
    calibration_true_in_vocabulary: usize,
    calibration_true_asserted: usize,
    calibration_true_missed: Vec<String>,
) -> LossReport {
    let (mut noise, mut vocabulary_loss, mut trigger_loss, mut asserted_candidates) =
        (0usize, 0usize, 0usize, 0usize);
    for instance in candidate_instances {
        let (_, relation, concept) = instance;
        if !super::typing::is_condition(concept, drugs) {
            noise += 1;
        } else if !vocabulary.terms(relation).iter().any(|t| t == concept) {
            vocabulary_loss += 1;
        } else if !asserted.contains(instance) {
            trigger_loss += 1;
        } else {
            asserted_candidates += 1;
        }
    }
    debug_assert_eq!(
        asserted_candidates + vocabulary_loss + trigger_loss + noise,
        candidate_instances.len(),
        "the three losses plus asserted candidates MUST partition the candidates"
    );

    LossReport {
        matcher_version: super::matcher::MATCHER_VERSION.to_string(),
        caveat: "NOT RECALL. There is no gold here. The denominator is EXTRACTOR CANDIDATES \
                 (instance-level: set_id + relation + concept), part of which is type noise. The \
                 three losses DO sum to the unasserted candidates (that is what a partition \
                 means), and asserted_candidates + the three losses == candidate_instances. What \
                 must NOT happen: reading that sum as a performance score, or adding it to \
                 asserted_by_matcher, which counts TOTAL assertions including facts found \
                 outside the candidate list. The three are distinct BUGS with distinct fixes, so \
                 a single combined figure hides which one is costing you. No number here may be \
                 described as a recall improvement until the held-out expert reference is \
                 complete."
            .to_string(),
        candidate_instances: candidate_instances.len(),
        vocabulary_expressible: candidate_instances.len() - noise - vocabulary_loss,
        vocabulary_loss,
        asserted_candidates,
        asserted_by_matcher: asserted.len(),
        asserted_outside_candidates: asserted.len().saturating_sub(asserted_candidates),
        trigger_structure_loss: trigger_loss,
        type_noise_instances: noise,
        calibration_true_total,
        calibration_true_in_vocabulary,
        calibration_true_asserted,
        calibration_true_missed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digest_freezes_the_term_lists() {
        let a = digest_of(&["psoriasis".into()], &["glaucoma".into()]);
        let b = digest_of(&["psoriasis".into()], &["glaucoma".into()]);
        let c = digest_of(&["psoriasis".into()], &["anuria".into()]);
        assert_eq!(a, b);
        assert_ne!(a, c, "digest must change when a term changes");
    }

    #[test]
    fn load_rejects_a_tampered_vocabulary() {
        let dir = std::env::temp_dir().join("cognigraph-vocab-test");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("tampered.json");
        let mut vocabulary = ClinicalVocabulary {
            version: "v1".into(),
            provenance: "test".into(),
            min_document_frequency: 1,
            source_documents: 1,
            excluded_documents: 0,
            treats: vec!["psoriasis".into()],
            contraindicated_in: vec![],
            digest: String::new(),
        };
        vocabulary.digest = digest_of(&vocabulary.treats, &vocabulary.contraindicated_in);
        vocabulary.save(&path).unwrap();
        assert!(ClinicalVocabulary::load(&path).is_ok());

        // Tamper with a term without updating the digest.
        vocabulary.treats.push("smuggled concept".into());
        vocabulary.save(&path).unwrap();
        assert!(
            ClinicalVocabulary::load(&path).is_err(),
            "a tampered vocabulary must not load"
        );
        fs::remove_dir_all(&dir).ok();
    }
}
