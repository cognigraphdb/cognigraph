//! Phase 2 — condition typing.
//!
//! Phase 1 refused every cue-less contraindication list, because the extractor
//! emitted drug names (`doxazosin`, `ketorolac tromethamine`) and prose
//! fragments (`severe`, `rarely fatal`) as if they were conditions. That
//! restraint cost six real contraindications (`anuria`, `glaucoma`,
//! `hyperthyroidism`, `systemic fungal infections`, ...) — bare lists are how
//! labels actually write contraindications.
//!
//! The fix is to TYPE the term, so bare lists can be asserted safely:
//!
//! - **A drug is not a condition**, and we do not have to guess which is which:
//!   the pilot corpus carries DailyMed's *structured* ingredient data (UNII
//!   substances). Every active substance across the 10k labels forms a drug
//!   lexicon derived from ground truth we already trust from gates 1-3. A bare
//!   term that IS a drug is refused because it is a drug — not because someone
//!   put it on a deny-list.
//! - **A clause is not a condition.** Prose carries finite verbs and reporting
//!   language ("have been reported", "should", "may"); a condition does not.
//! - **A modifier is not a condition.** A term made only of qualifiers
//!   (`severe`, `rarely fatal`) has no head noun and names nothing.
//!
//! No calibration-specific allowlist exists here. Nothing in this file mentions
//! a calibration concept; typing that happened to fix the showcase by naming its
//! answers would be worthless.

/// R1 requires qualified concepts, and qualified concepts are long: "severe or
/// incapacitating atopic dermatitis intractable to adequate trials of
/// conventional treatment" is 99 characters. The previous 90-char cap silently
/// deleted exactly the qualifiers the rule exists to preserve.
pub(crate) const CONCEPT_MAX_CHARS: usize = 140;

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use quick_xml::Reader;
use quick_xml::events::Event;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::model::*;

/// Words that qualify a condition but never name one. A term composed ONLY of
/// these has no head noun.
const MODIFIERS_ONLY: &[&str] = &[
    "severe",
    "mild",
    "moderate",
    "acute",
    "chronic",
    "rarely",
    "fatal",
    "serious",
    "significant",
    "excessive",
    "persistent",
    "advanced",
    "known",
    "other",
    "general",
    "special",
    "marked",
    "and",
    "or",
    "the",
    "a",
    "an",
    "in",
    "of",
    "to",
    "with",
];

/// Finite verbs and reporting language: the mark of a clause, not a concept.
const CLAUSE_MARKERS: &[&str] = &[
    " have been",
    " has been",
    " had been",
    " were ",
    " was ",
    " should",
    " must ",
    " may ",
    " can ",
    " will ",
    " reported",
    " occur",
    " include",
    " see ",
];

/// Section boilerplate that is not a clinical concept.
const BOILERPLATE: &[&str] = &[
    "drug interactions",
    "drug-drug interactions",
    "interactions",
    "precautions",
    "warnings",
    "contraindications",
    "adverse reactions",
    "use",
    "uses",
    "indications",
];

/// Salt and hydrate suffixes. Contraindication prose names the moiety
/// ("aliskiren"), while the label names the salt ("aliskiren hemifumarate"), so
/// both forms must type as drugs.
const SALT_SUFFIXES: &[&str] = &[
    "hydrochloride",
    "hcl",
    "sodium",
    "potassium",
    "calcium",
    "magnesium",
    "sulfate",
    "succinate",
    "tartrate",
    "maleate",
    "fumarate",
    "hemifumarate",
    "besylate",
    "mesylate",
    "citrate",
    "acetate",
    "phosphate",
    "bromide",
    "chloride",
    "nitrate",
    "tromethamine",
    "dipropionate",
    "valerate",
    "propionate",
    "furoate",
    "monohydrate",
    "dihydrate",
    "anhydrous",
    "bitartrate",
    "hydrobromide",
    "oxalate",
    "lactate",
    "gluconate",
    "carbonate",
];

/// Add `term` plus its salt-stripped moiety.
fn insert_drug(into: &mut BTreeSet<String>, term: &str) {
    let term = term.trim().trim_matches(['-', ',', '.', '"']).trim();
    if term.len() < 4 || term.len() > 60 || !term.chars().any(|c| c.is_alphabetic()) {
        return;
    }
    into.insert(term.to_string());
    // "aliskiren hemifumarate" -> also "aliskiren"
    if let Some((base, last)) = term.rsplit_once(' ')
        && SALT_SUFFIXES.contains(&last)
        && base.len() >= 4
    {
        into.insert(base.trim().to_string());
    }
}

/// Mine drug names from a DailyMed catalog title. The catalogs were enumerated
/// in full during collection (151,759 titles), so this covers every MARKETED
/// drug — not merely the ones sampled into the 10k corpus. That is what closes
/// the corpus-bounded hole: `dronedarone` and `aliskiren` are never active
/// ingredients of a sampled label, but they ARE marketed, so the catalog knows
/// them ("MULTAQ (DRONEDARONE)", "TEKTURNA (ALISKIREN HEMIFUMARATE)").
///
/// **Only the PARENTHESIZED generic name is mined, never the leading brand
/// segment.** Homeopathic products are named after the condition they claim to
/// treat — "GLAUCOMA (ACONITUM NAP., ...)", "ASTHMA THERAPY (...)", "PSORIASIS
/// SYMPTOM RELIEF (...)" — so mining brand segments would type *glaucoma*,
/// *asthma* and *psoriasis* as DRUGS and silently delete real conditions from
/// every contraindication list. The asymmetry is deliberate: a false drug
/// destroys true facts, while a missed drug merely risks a junk candidate that
/// the clause/modifier rules and the vocabulary's document-frequency threshold
/// still have to clear. Precision wins.
fn drugs_from_title(title: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let lower = title.to_lowercase();
    // Drop the labeler, then read only parenthesized substance names.
    let lower = lower.split('[').next().unwrap_or(&lower).trim().to_string();
    let mut rest = lower.as_str();
    let mut consumed = 0usize;
    while let Some(open) = rest.find('(') {
        // Text before this parenthesis — the brand segment.
        let brand = &lower[..consumed + open];
        let after = &rest[open + 1..];
        let Some(close) = after.find(')') else { break };
        let inner = &after[..close];
        // "BRAND (GENERIC)" only names a drug when the parenthesized text
        // actually DIFFERS from the brand. "NEKVNRO PSORIASIS (PSORIASIS)" is a
        // degenerate echo of a product named after its claimed indication, not a
        // generic name — mining it would type *psoriasis* as a drug.
        // Hyphens are NOT split: they belong inside substance names
        // ("beta-carotene"), and splitting them turned "pain-relieving" into
        // "pain".
        for part in inner.split(',').flat_map(|p| p.split(" and ")) {
            let part = part.trim();
            if part.is_empty() || brand.contains(part) {
                continue;
            }
            insert_drug(&mut found, part);
        }
        consumed += open + 1 + close + 1;
        rest = &after[close + 1..];
    }
    found
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrugLexicon {
    pub version: String,
    pub provenance: String,
    pub source_documents: usize,
    pub substances: Vec<String>,
    pub digest: String,
}

impl DrugLexicon {
    pub fn load(path: &Path) -> Result<Self> {
        let lexicon: Self = serde_json::from_str(
            &fs::read_to_string(path)
                .with_context(|| format!("read drug lexicon {}", path.display()))?,
        )?;
        let expected = digest_of(&lexicon.substances);
        if expected != lexicon.digest {
            anyhow::bail!("drug lexicon {} failed its integrity check", path.display());
        }
        Ok(lexicon)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, serde_json::to_string_pretty(self)?)
            .with_context(|| format!("write {}", path.display()))
    }

    pub fn set(&self) -> BTreeSet<String> {
        self.substances.iter().cloned().collect()
    }
}

fn digest_of(substances: &[String]) -> String {
    let mut hasher = Sha256::new();
    for substance in substances {
        hasher.update(substance.as_bytes());
        hasher.update(b"\n");
    }
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Active substance names from one SPL's structured data (the same UNII-anchored
/// ingredient block gates 1-3 used as ground truth).
fn active_substances(xml: &str) -> BTreeSet<String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut names = BTreeSet::new();
    let (mut in_substance, mut capture) = (false, false);
    loop {
        match reader.read_event() {
            Err(_) | Ok(Event::Eof) => break,
            Ok(Event::Start(e)) => match e.name().as_ref() {
                // EVERY substance, active or inactive: an excipient is not a
                // condition either, so a broader substance set only makes the
                // typing safer. Coverage is still corpus-bounded (a drug never
                // marketed in this corpus cannot be typed) — recorded as a
                // limitation, not hidden.
                "ingredientSubstance" => in_substance = true,
                "name" if in_substance && !capture => capture = true,
                _ => {}
            },
            Ok(Event::Text(t)) => {
                if capture {
                    let name = t.trim().to_lowercase();
                    if !name.is_empty() && name.len() < 90 {
                        names.insert(name);
                    }
                    capture = false;
                }
            }
            Ok(Event::End(e)) => {
                if e.name().as_ref() == "ingredientSubstance" {
                    in_substance = false;
                }
            }
            Ok(_) => {}
        }
    }
    names
}

/// Build the drug lexicon from the corpus's structured ingredient data.
pub fn build_drug_lexicon(corpus: &Path) -> Result<DrugLexicon> {
    let mut substances: BTreeSet<String> = BTreeSet::new();
    let mut used = 0usize;
    // Both populations: a drug named in one label's contraindications may only
    // be marketed under another.
    for manifest_name in ["manifest-rx.jsonl", "manifest-otc.jsonl"] {
        let manifest_path = corpus.join(manifest_name);
        let Ok(manifest) = fs::read_to_string(&manifest_path) else {
            continue;
        };
        for line in manifest.lines().filter(|line| !line.trim().is_empty()) {
            let entry: ManifestEntry = serde_json::from_str(line)?;
            let xml_path = corpus.join("raw").join(format!("{}.xml", entry.set_id));
            let Ok(xml) = fs::read_to_string(&xml_path) else {
                continue;
            };
            used += 1;
            substances.extend(active_substances(&xml));
        }
    }
    // The FULL catalogs (every marketed label, not just the sampled 10k). This is
    // what closes the corpus-bounded gap.
    for catalog in ["rx.json", "otc.json"] {
        let path = corpus.join("catalog").join(catalog);
        let Ok(body) = fs::read_to_string(&path) else {
            continue;
        };
        let parsed: serde_json::Value = serde_json::from_str(&body)?;
        let Some(entries) = parsed["entries"].as_array() else {
            continue;
        };
        for entry in entries {
            let Some(title) = entry["title"].as_str() else {
                continue;
            };
            substances.extend(drugs_from_title(title));
        }
    }

    let substances: Vec<String> = substances.into_iter().collect();
    let digest = digest_of(&substances);
    Ok(DrugLexicon {
        version: "dailymed-drug-lexicon-v1".to_string(),
        provenance: "substance names from DailyMed STRUCTURED ingredient data (the \
                     UNII-anchored block used as ground truth in gates 1-3), across both the \
                     prescription and OTC populations. A term that is a substance is refused as \
                     a condition because it IS a substance, not because it was deny-listed. \
                     PLUS drug names mined from the FULL DailyMed catalogs enumerated during \
                     collection (151,759 titles — every MARKETED label, not just the 10k \
                     sampled), with salt/hydrate suffixes stripped so both the moiety \
                     (aliskiren) and the salt (aliskiren hemifumarate) type as drugs. This \
                     closes the earlier corpus-bounded hole, where a drug named in \
                     contraindication prose but never an active ingredient of a SAMPLED label \
                     (dronedarone, aliskiren) could not be typed."
            .to_string(),
        source_documents: used,
        substances,
        digest,
    })
}

/// Is `term` a clinical CONDITION (as opposed to a drug, a clause, or a bare
/// modifier)? This is what makes bare-list contraindications safe to assert.
pub fn is_condition(term: &str, drugs: &BTreeSet<String>) -> bool {
    let term = term.trim().to_lowercase();
    if term.len() < 4 || term.len() > CONCEPT_MAX_CHARS {
        return false;
    }
    if BOILERPLATE.contains(&term.as_str()) {
        return false;
    }
    // A clause is not a concept.
    let padded = format!(" {term} ");
    if CLAUSE_MARKERS.iter().any(|m| padded.contains(m)) {
        return false;
    }
    // A drug is not a condition. Coordinated drug lists ("fluvoxamine or
    // ciprofloxacin") are refused when every part is a drug.
    let parts: Vec<&str> = term
        .split(" or ")
        .flat_map(|p| p.split(" and "))
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();
    if !parts.is_empty() && parts.iter().all(|part| drugs.contains(*part)) {
        return false;
    }
    if drugs.contains(&term) {
        return false;
    }
    // A term made only of qualifiers has no head noun.
    if term
        .split_whitespace()
        .all(|word| MODIFIERS_ONLY.contains(&word))
    {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drugs() -> BTreeSet<String> {
        [
            "doxazosin",
            "dronedarone",
            "ketorolac tromethamine",
            "fluvoxamine",
            "ciprofloxacin",
        ]
        .iter()
        .map(|d| d.to_string())
        .collect()
    }

    /// The six real contraindications phase 1 had to refuse must now type as
    /// conditions. (These are checked as TYPES, not allow-listed by name.)
    #[test]
    fn real_bare_list_contraindications_type_as_conditions() {
        for term in [
            "anuria",
            "glaucoma",
            "hyperthyroidism",
            "advanced arteriosclerosis",
            "moderate and severe hypertension",
            "systemic fungal infections",
            "symptomatic cardiovascular disease",
        ] {
            assert!(
                is_condition(term, &drugs()),
                "should be a condition: {term}"
            );
        }
    }

    /// A drug is refused because it IS a drug — from the structured lexicon.
    #[test]
    fn drug_names_are_not_conditions() {
        for term in [
            "doxazosin",
            "dronedarone",
            "ketorolac tromethamine",
            "fluvoxamine or ciprofloxacin",
        ] {
            assert!(
                !is_condition(term, &drugs()),
                "must not be a condition: {term}"
            );
        }
    }

    #[test]
    fn prose_fragments_and_bare_modifiers_are_not_conditions() {
        for term in [
            "severe",
            "rarely fatal",
            "drug interactions",
            "precautions",
            "anaphylactic-like reactions to nsaids have been reported in such patients",
            "patients should be monitored",
        ] {
            assert!(
                !is_condition(term, &drugs()),
                "must not be a condition: {term}"
            );
        }
    }

    /// The corpus-bounded hole, closed: these drugs are named in contraindication
    /// prose but are never active ingredients of a SAMPLED label. The FULL
    /// catalogs know them.
    #[test]
    fn catalog_titles_yield_generic_drug_names() {
        let multaq = drugs_from_title("MULTAQ (DRONEDARONE) TABLET, FILM COATED [SANOFI]");
        assert!(multaq.contains("dronedarone"), "{multaq:?}");

        let tekturna =
            drugs_from_title("TEKTURNA (ALISKIREN HEMIFUMARATE) TABLET, FILM COATED [LXO]");
        // Prose names the moiety, the label names the salt: BOTH must type.
        assert!(tekturna.contains("aliskiren"), "{tekturna:?}");
        assert!(tekturna.contains("aliskiren hemifumarate"), "{tekturna:?}");
    }

    /// THE TRAP: homeopathic products are named after the condition they claim to
    /// treat. Mining a brand segment — or a parenthesized echo of it — would type
    /// *psoriasis* and *glaucoma* as DRUGS and silently delete real conditions
    /// from every contraindication list. A false drug destroys true facts.
    #[test]
    fn a_product_named_after_a_condition_never_types_that_condition_as_a_drug() {
        let echo = drugs_from_title("NEKVNRO PSORIASIS (PSORIASIS) CREAM [GUOYU TRADING]");
        assert!(
            !echo.contains("psoriasis"),
            "a parenthesized echo of the brand is not a generic name: {echo:?}"
        );

        let homeopathic =
            drugs_from_title("GLAUCOMA (ACONITUM NAP., BELLADONNA, CAUSTICUM) PELLET [X]");
        assert!(
            !homeopathic.contains("glaucoma"),
            "the brand segment must never be mined: {homeopathic:?}"
        );
        assert!(homeopathic.contains("belladonna"), "{homeopathic:?}");

        // Hyphens belong INSIDE substance names; splitting them turned
        // "pain-relieving" into "pain".
        let pad = drugs_from_title(
            "NUMSTAT ANTISEPTIC AND PAIN-RELIEVING PREP PAD (ANTISEPTIC AND PAIN-RELIEVING PREP PAD) SWAB [S]",
        );
        assert!(!pad.contains("pain"), "{pad:?}");
    }

    /// Typing must not be so eager it eats a condition that merely CONTAINS a
    /// qualifier word.
    #[test]
    fn qualified_conditions_survive_typing() {
        for term in [
            "severe organic heart disease",
            "persistent and excessive supine hypertension",
            "acute renal disease",
        ] {
            assert!(is_condition(term, &drugs()), "should survive: {term}");
        }
    }
}
