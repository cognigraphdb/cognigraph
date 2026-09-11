//! Clinical assertion matcher — `clinical-matcher-v2`.
//!
//! **Scope is deliberately narrow and isolated.** This does NOT touch
//! `ground_chunk`, the generic trigger semantics, or the frozen gate-1/2/3
//! policies. It runs ONLY over the two selected clinical sections
//! (INDICATIONS, CONTRAINDICATIONS) and exists because template triggers of the
//! form `indicated for {target}` cannot see how drug labels actually assert
//! things: they enumerate (`indicated for the treatment of A, B, and C` — a
//! template only ever matches `A`), they bullet (`• Management of
//! fibromyalgia`), and they prohibit in prose (`should not be given to patients
//! with ...`). The accepted instance-level calibration demonstrates that
//! vocabulary coverage and actual assertion behavior must be measured
//! separately.
//!
//! What it preserves, non-negotiably:
//! - **Exact evidence spans** — every assertion carries a verbatim substring of
//!   its section.
//! - **Product scoping** — the fact's source is always the document's own
//!   product, and a unit whose relation belongs to a *different* subject
//!   ("Unlike corticosteroids indicated for eczema...", "Agents contraindicated
//!   in asthma include...") is refused.
//! - **Negation handling** — a cue inside a negated clause does not assert
//!   (reuses `affirms_phrase`; it is read, never modified).
//! - **R1 qualifiers** — concepts keep subtype/severity/anatomy/timing/causative
//!   drug. A generic term is never substituted for a qualified one.
//! - **Governed vocabulary** — the matcher can only assert a concept present in
//!   the relation's frozen vocabulary. It cannot invent vocabulary, exactly as
//!   neurons cannot.
//!
//! **Phase 2 — the typed bare-list path.** Labels state most contraindications
//! as a bare list with no cue ("Anuria.", "Advanced arteriosclerosis, ...,
//! glaucoma."), and rule R4 says those ARE contraindications. Phase 1 refused
//! them wholesale, because the extractor emitted drug names (`doxazosin`) and
//! prose fragments (`severe`, `rarely fatal`) as if they were conditions — six
//! real contraindications were the accepted cost. Phase 2 admits them through
//! CONDITION TYPING (`typing::is_condition`): a substance, a clause, or a bare
//! modifier is not a condition. Crucially the substance test comes from the
//! corpus's own STRUCTURED ingredient data, so a drug is refused because it IS a
//! drug — not because it was deny-listed. INDICATIONS get no bare-list path: a
//! cue-less sentence there is prose, not an indication.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::model::*;
use super::typing::is_condition;
use crate::grounding::affirms_phrase;

pub const MATCHER_VERSION: &str = "clinical-matcher-v2";
const INDICATIONS: &str = "34067-9";
const CONTRAINDICATIONS_CODE: &str = "34070-3";

// Deliberately owned by the matcher rather than imported from reference
// preparation. The matcher and the annotation candidate extractor must not
// validate one another through a shared parser.
const TREATS_CUES: &[&str] = &[
    "indicated as adjunctive therapy for the treatment of",
    "indicated as adjunctive therapy in",
    "adjunctive therapy for the treatment of",
    "indicated for the treatment of",
    "indicated in the treatment of",
    "indicated for the management of",
    "indicated in the management of",
    "indicated for the relief of",
    "indicated for the prevention of",
    "indicated for",
    "indicated in",
    "management of",
    "treatment of",
    "prevention of",
    "prophylaxis of",
    "relief of",
    "control of",
];

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
];

/// A unit whose relation is attributed to some OTHER subject. Refusing these is
/// what keeps product scoping honest when the section names other agents.
const FOREIGN_SUBJECT_MARKERS: &[&str] = &[
    "unlike ",
    "agents ",
    "other agents",
    "other drugs",
    "drugs such as",
    "compared with",
    "compared to",
    "in contrast",
    "similar to",
    "as with other",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClinicalAssertion {
    pub relation: String,
    pub concept: String,
    /// Verbatim substring of the section that licenses this assertion.
    pub evidence: String,
}

/// Assert clinical facts from ONE selected section, for the document's product.
///
/// `vocabulary` is the frozen term list **for this relation** (never the merged
/// list — a TREATS concept must not be assertable as a contraindication).
pub fn match_section(
    section: &ClinicalSection,
    vocabulary: &BTreeSet<String>,
    drugs: &BTreeSet<String>,
) -> Vec<ClinicalAssertion> {
    let (relation, cues) = match section.code.as_str() {
        INDICATIONS => (TREATS, TREATS_CUES),
        CONTRAINDICATIONS_CODE => (CONTRAINDICATED_IN, CONTRA_CUES),
        _ => return Vec::new(),
    };
    let mut asserted = Vec::new();
    let mut seen = BTreeSet::new();

    for unit in matcher_units(&section.text) {
        let lower = unit.to_lowercase();

        // Product scoping: the relation must belong to THIS product.
        if FOREIGN_SUBJECT_MARKERS
            .iter()
            .any(|marker| lower.contains(marker))
        {
            continue;
        }

        let cue = cues
            .iter()
            .filter(|cue| lower.contains(**cue))
            .max_by_key(|cue| cue.len());

        let concepts = match cue {
            Some(cue) => {
                // Negation: a cue inside a negated clause asserts nothing.
                // Reuses the frozen lookback rather than reimplementing it.
                if !affirms_phrase(&unit, cue) {
                    continue;
                }
                // Enumeration/bullet aware: every item after the cue, not just
                // the first.
                matcher_concepts(&unit, cue)
            }
            // PHASE 2 — the typed bare-list path. A contraindication section
            // states most of its contraindications as a bare list with no cue
            // at all ("Anuria.", "Advanced arteriosclerosis, ... glaucoma."),
            // and R4 says those are real contraindications. Phase 1 refused
            // them wholesale because the extractor emitted drug names and prose
            // fragments as conditions. They are now admitted, but ONLY through
            // condition typing: a substance, a clause, or a bare modifier is not
            // a condition and is refused. Indications are NOT given a bare-list
            // path — a cue-less sentence there is prose, not an indication.
            None if relation == CONTRAINDICATED_IN => split_matcher_list(&lower),
            None => continue,
        };

        for concept in concepts {
            // Typing gate (phase 2): never assert a drug, a clause, or a bare
            // modifier as a condition.
            if !is_condition(&concept, drugs) {
                continue;
            }
            // Governed: the matcher may only assert vocabulary it was given,
            // and only for THIS relation.
            if !vocabulary.contains(&concept) {
                continue;
            }
            if seen.insert(concept.clone()) {
                asserted.push(ClinicalAssertion {
                    relation: relation.to_string(),
                    concept,
                    evidence: unit.clone(),
                });
            }
        }
    }
    asserted
}

/// Matcher-owned segmentation. This intentionally does not call the reference
/// candidate extractor: held-out expert labels, not shared parsing code, are
/// the eventual oracle.
fn matcher_units(text: &str) -> Vec<String> {
    text.split('•')
        .flat_map(|part| part.split_inclusive(['.', '!', '?', ';']))
        .map(|unit| unit.trim().to_string())
        .filter(|unit| unit.len() > 8)
        .collect()
}

fn matcher_concepts(unit: &str, cue: &str) -> Vec<String> {
    let lower = unit.to_lowercase();
    let Some(at) = lower.find(cue) else {
        return Vec::new();
    };
    let mut scope = lower[at + cue.len()..].trim();
    for prefix in [":", "the following:"] {
        scope = scope.strip_prefix(prefix).unwrap_or(scope).trim();
    }
    for cut in [
        " in patients",
        " in adults",
        " in children",
        " to lower",
        " to reduce",
        " who ",
        " when ",
        " see ",
        " because ",
    ] {
        if let Some(index) = scope.find(cut) {
            scope = scope[..index].trim();
        }
    }

    // R6: the restriction scopes over every item in the coordinated list. A
    // plain item is never emitted; the shared restriction is distributed into
    // each normalized target.
    const REFRACTORY: &str = "severe or incapacitating allergic conditions intractable to adequate trials of conventional treatment in ";
    if let Some(items) = scope.strip_prefix(REFRACTORY) {
        return split_matcher_list(items)
            .into_iter()
            .map(|item| {
                format!(
                    "severe or incapacitating {item} intractable to adequate trials of conventional treatment"
                )
            })
            .collect();
    }
    split_matcher_list(scope)
}

fn split_matcher_list(scope: &str) -> Vec<String> {
    scope
        .split([',', ';'])
        .filter_map(|fragment| {
            let mut concept = fragment.trim();
            // A section heading is glued to the first bare-list item
            // ("CONTRAINDICATIONS: Anuria."); it is a label, not part of the
            // concept.
            for label in ["contraindications:", "contraindications"] {
                concept = concept.strip_prefix(label).unwrap_or(concept).trim();
            }
            for prefix in ["and ", "or ", "known ", "the ", "a ", "an "] {
                concept = concept.strip_prefix(prefix).unwrap_or(concept).trim();
            }
            if let Some(index) = concept.find(['(', '[']) {
                concept = concept[..index].trim();
            }
            let concept = concept.trim_matches(['.', ':', ')', ']']);
            (concept.len() >= 4).then(|| concept.to_string())
        })
        .collect()
}

/// Assert over every selected section of a packet.
pub fn match_packet(
    packet: &ReviewPacket,
    treats_vocabulary: &BTreeSet<String>,
    contra_vocabulary: &BTreeSet<String>,
    drugs: &BTreeSet<String>,
) -> Vec<ClinicalAssertion> {
    packet
        .sections
        .iter()
        .flat_map(|section| {
            let vocabulary = if section.code == INDICATIONS {
                treats_vocabulary
            } else {
                contra_vocabulary
            };
            match_section(section, vocabulary, drugs)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn section(code: &str, text: &str) -> ClinicalSection {
        ClinicalSection {
            code: code.to_string(),
            title: String::new(),
            text: text.to_string(),
        }
    }

    fn vocab(terms: &[&str]) -> BTreeSet<String> {
        terms.iter().map(|t| t.to_string()).collect()
    }

    /// The failure that motivated the matcher: a template trigger only ever
    /// matched the FIRST item of an enumeration.
    #[test]
    fn asserts_every_item_of_an_enumeration_not_just_the_first() {
        let s = section(
            INDICATIONS,
            "Dexamethasone is indicated for the treatment of asthma, atopic dermatitis, \
             and serum sickness.",
        );
        let v = vocab(&["asthma", "atopic dermatitis", "serum sickness"]);
        let got: Vec<_> = match_section(&s, &v, &BTreeSet::new())
            .into_iter()
            .map(|a| a.concept)
            .collect();
        assert!(got.contains(&"asthma".to_string()), "{got:?}");
        assert!(got.contains(&"atopic dermatitis".to_string()), "{got:?}");
        assert!(got.contains(&"serum sickness".to_string()), "{got:?}");
    }

    #[test]
    fn distributes_a_shared_refractory_restriction_without_leaking_broad_targets() {
        let text = "Control of severe or incapacitating allergic conditions intractable to \
                    adequate trials of conventional treatment in asthma, atopic dermatitis, \
                    and serum sickness.";
        let s = section(INDICATIONS, text);
        let qualified = "severe or incapacitating atopic dermatitis intractable to adequate trials of conventional treatment";
        let got = match_section(
            &s,
            &vocab(&[qualified, "atopic dermatitis"]),
            &BTreeSet::new(),
        );
        assert!(got.iter().any(|assertion| assertion.concept == qualified));
        assert!(
            !got.iter()
                .any(|assertion| assertion.concept == "atopic dermatitis")
        );
    }

    #[test]
    fn asserts_bulleted_and_adjunctive_phrasing() {
        let s = section(
            INDICATIONS,
            "• Adjunctive therapy for the treatment of partial-onset seizures \
             • Management of fibromyalgia",
        );
        let v = vocab(&["partial-onset seizures", "fibromyalgia"]);
        let got: Vec<_> = match_section(&s, &v, &BTreeSet::new())
            .into_iter()
            .map(|a| a.concept)
            .collect();
        assert!(
            got.contains(&"partial-onset seizures".to_string()),
            "{got:?}"
        );
        assert!(got.contains(&"fibromyalgia".to_string()), "{got:?}");
    }

    #[test]
    fn asserts_explicit_prohibition_phrasing() {
        let s = section(
            CONTRAINDICATIONS_CODE,
            "Ketorolac should not be given to patients with advanced renal impairment.",
        );
        let v = vocab(&["advanced renal impairment"]);
        let got: Vec<_> = match_section(&s, &v, &BTreeSet::new())
            .into_iter()
            .map(|a| a.concept)
            .collect();
        assert_eq!(got, vec!["advanced renal impairment".to_string()]);
    }

    /// R1: a generic term is never substituted for the qualified concept.
    #[test]
    fn never_substitutes_a_generic_term_for_a_qualified_concept() {
        let s = section(
            INDICATIONS,
            "Indicated for the treatment of partial-onset seizures.",
        );
        // The vocabulary offers the generic term; the label says the subtype.
        let v = vocab(&["seizures"]);
        let got = match_section(&s, &v, &BTreeSet::new());
        assert!(
            got.is_empty(),
            "generic 'seizures' must not be asserted from 'partial-onset seizures': {got:?}"
        );
    }

    #[test]
    fn refuses_negated_assertions() {
        let s = section(
            CONTRAINDICATIONS_CODE,
            "This product is not contraindicated in pregnancy.",
        );
        assert!(match_section(&s, &vocab(&["pregnancy"]), &BTreeSet::new()).is_empty());
    }

    /// Product scoping: the relation belongs to another subject.
    #[test]
    fn refuses_a_foreign_subject() {
        let a = section(
            INDICATIONS,
            "Unlike corticosteroids indicated for eczema, this class acts differently.",
        );
        assert!(match_section(&a, &vocab(&["eczema"]), &BTreeSet::new()).is_empty());
        let b = section(
            CONTRAINDICATIONS_CODE,
            "Agents contraindicated in asthma include nonselective beta-blockers.",
        );
        assert!(match_section(&b, &vocab(&["asthma"]), &BTreeSet::new()).is_empty());
    }

    /// PHASE 2: a cue-less bare list IS asserted — but only for terms that TYPE
    /// as conditions.
    #[test]
    fn asserts_typed_bare_list_contraindications() {
        let s = section(
            CONTRAINDICATIONS_CODE,
            "Advanced arteriosclerosis, moderate and severe hypertension, hyperthyroidism, \
             and glaucoma.",
        );
        let v = vocab(&[
            "advanced arteriosclerosis",
            "moderate and severe hypertension",
            "hyperthyroidism",
            "glaucoma",
        ]);
        let got: Vec<_> = match_section(&s, &v, &BTreeSet::new())
            .into_iter()
            .map(|a| a.concept)
            .collect();
        assert!(got.contains(&"glaucoma".to_string()), "{got:?}");
        assert!(got.contains(&"hyperthyroidism".to_string()), "{got:?}");
        assert!(
            got.contains(&"moderate and severe hypertension".to_string()),
            "qualifier must survive (R1): {got:?}"
        );
    }

    /// PHASE 2 RESTRAINT: a drug in a bare contraindication list must NEVER
    /// become a condition fact — refused because it IS a drug (structured
    /// lexicon), even though it sits in the vocabulary.
    #[test]
    fn bare_list_never_asserts_a_drug_as_a_condition() {
        let s = section(CONTRAINDICATIONS_CODE, "Doxazosin, ketorolac tromethamine.");
        let v = vocab(&["doxazosin", "ketorolac tromethamine"]);
        let drugs: BTreeSet<String> = ["doxazosin", "ketorolac tromethamine"]
            .iter()
            .map(|d| d.to_string())
            .collect();
        assert!(
            match_section(&s, &v, &drugs).is_empty(),
            "a drug must never be asserted as a contraindicated condition"
        );
    }

    /// PHASE 2 RESTRAINT: prose in a contraindications section must not become
    /// facts via the bare-list path.
    #[test]
    fn does_not_assert_cueless_contraindication_prose() {
        let s = section(
            CONTRAINDICATIONS_CODE,
            "Severe, rarely fatal, anaphylactic-like reactions to NSAIDs have been \
             reported in such patients.",
        );
        let v = vocab(&["severe", "rarely fatal", "doxazosin"]);
        assert!(
            match_section(&s, &v, &BTreeSet::new()).is_empty(),
            "cue-less prose must not be asserted as conditions"
        );
    }

    /// An INDICATIONS section gets no bare-list path at all.
    #[test]
    fn indications_have_no_bare_list_path() {
        let s = section(INDICATIONS, "Psoriasis, eczema.");
        let v = vocab(&["psoriasis", "eczema"]);
        assert!(
            match_section(&s, &v, &BTreeSet::new()).is_empty(),
            "a cue-less sentence in INDICATIONS is prose, not an indication"
        );
    }

    /// The matcher is governed: it cannot assert a concept outside the frozen
    /// vocabulary for that relation, and cannot cross relations.
    #[test]
    fn cannot_assert_outside_its_relation_vocabulary() {
        let s = section(INDICATIONS, "Indicated for the treatment of hypertension.");
        assert!(match_section(&s, &vocab(&["glaucoma"]), &BTreeSet::new()).is_empty());
    }

    #[test]
    fn evidence_is_an_exact_section_substring() {
        let text = "• Management of fibromyalgia • Management of postherpetic neuralgia";
        let s = section(INDICATIONS, text);
        let v = vocab(&["fibromyalgia", "postherpetic neuralgia"]);
        for assertion in match_section(&s, &v, &BTreeSet::new()) {
            assert!(
                text.contains(&assertion.evidence),
                "evidence not a substring: {:?}",
                assertion.evidence
            );
        }
    }
}
