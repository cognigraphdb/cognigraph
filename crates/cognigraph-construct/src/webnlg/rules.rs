//! Per-lane rule sets and the provided-entity SpaceType builder (W2). A rule set
//! maps a DBpedia predicate to its trigger templates; the predicate IS the
//! constructed relation (W6), so a grounded fact's relation compares directly to
//! the oracle predicate. Templates use {source}/{target}, expanded by
//! `ground_chunk` against the provided entity surfaces.
//!
//! Predicate-matching rules and the frozen generic relation vocabulary (W4-W5).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::types::{EntityDef, RelationRule, SpaceType};

/// A frozen predicate -> trigger-templates map. `generic()` is the built-in
/// naive baseline; the neuron-authored set is loaded from JSON via `from_json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleSet {
    pub name: String,
    pub predicates: BTreeMap<String, Vec<String>>,
}

impl RuleSet {
    /// The frozen `generic` baseline: a small hand-authored set over common
    /// WebNLG predicates. Deliberately small — it is the low baseline (W1).
    pub fn generic() -> Self {
        let mut predicates = BTreeMap::new();
        predicates.insert(
            "leader".to_string(),
            vec![
                "leader of {source} is {target}".to_string(),
                "{source} is led by {target}".to_string(),
            ],
        );
        predicates.insert(
            "location".to_string(),
            vec![
                "{source} is located in {target}".to_string(),
                "{source} is located at {target}".to_string(),
            ],
        );
        predicates.insert(
            "runwayLength".to_string(),
            vec!["{source} runway length is {target}".to_string()],
        );
        predicates.insert(
            "country".to_string(),
            vec!["{source} is in {target}".to_string()],
        );
        Self {
            name: "generic".to_string(),
            predicates,
        }
    }

    /// Load a frozen neuron-authored rule set from JSON (the reviewed artifact).
    pub fn from_json(raw: &str) -> anyhow::Result<Self> {
        Ok(serde_json::from_str(raw)?)
    }
}

/// Human-readable aliases for a `YYYY-MM-DD` date literal, covering the common
/// WebNLG prose renderings so the grounder can match a date in text while the
/// constructed edge still carries the ISO token (W6). Optional surrounding
/// double-quotes (some oracle literals are quoted) are tolerated. Returns empty
/// for anything that is not an ISO calendar date.
pub fn date_aliases(surface: &str) -> Vec<String> {
    let core = surface.trim().trim_matches('"');
    let parts: Vec<&str> = core.split('-').collect();
    if parts.len() != 3 {
        return Vec::new();
    }
    let (Ok(year), Ok(month), Ok(day)) = (
        parts[0].parse::<i32>(),
        parts[1].parse::<u32>(),
        parts[2].parse::<u32>(),
    ) else {
        return Vec::new();
    };
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return Vec::new();
    }
    const MONTHS: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    let m = MONTHS[(month - 1) as usize];
    let ord = ordinal(day);
    // Grounding lowercases, so case here is immaterial; the spread covers the
    // day/month order, ordinal, and comma variants seen in WebNLG prose.
    vec![
        format!("{m} {day}, {year}"),
        format!("{m} {day} {year}"),
        format!("{day} {m} {year}"),
        format!("{day} {m}, {year}"),
        format!("{ord} {m} {year}"),
        format!("{ord} {m}, {year}"),
        format!("{m} {ord}, {year}"),
        format!("{m} {ord} {year}"),
    ]
}

/// `1` -> `1st`, `2` -> `2nd`, `3` -> `3rd`, `4..` -> `4th`, with the 11/12/13
/// exception.
fn ordinal(day: u32) -> String {
    let suffix = match (day % 100, day % 10) {
        (11..=13, _) => "th",
        (_, 1) => "st",
        (_, 2) => "nd",
        (_, 3) => "rd",
        _ => "th",
    };
    format!("{day}{suffix}")
}

/// Build the SpaceType for one document: provided entities (W2) plus one
/// RelationRule per (ordered entity pair) x (predicate) with that predicate's
/// trigger templates. `only_predicates`, when `Some`, restricts instantiation
/// to the gold predicates — used by the oracle-diagnostic lane.
///
/// Entity tokens are DBpedia surfaces (e.g. `Jacob_Bundsgaard`). The token is
/// kept verbatim as the `EntityDef.name` and the `RelationRule` endpoints, so a
/// grounded fact's endpoints compare directly against the oracle triple (W6).
/// The human-readable form (`Jacob Bundsgaard`) is carried as an alias so that
/// `{source}`/`{target}` expansion still matches natural-language text, and an
/// ISO-date token additionally carries its prose renderings (see
/// [`date_aliases`]) so a date literal grounds from text like "July 23, 1982".
pub fn build_space(
    entities: &[String],
    rules: &RuleSet,
    only_predicates: Option<&[String]>,
) -> SpaceType {
    let entity_defs: Vec<EntityDef> = entities
        .iter()
        .map(|token| {
            let readable = token.replace('_', " ");
            let mut aliases = if readable == *token {
                Vec::new()
            } else {
                vec![readable]
            };
            aliases.extend(date_aliases(token));
            EntityDef {
                name: token.clone(),
                entity_type: "entity".to_string(),
                aliases,
            }
        })
        .collect();

    let mut relation_rules = Vec::new();
    for source in entities {
        for target in entities {
            if source == target {
                continue;
            }
            for (predicate, triggers) in &rules.predicates {
                if let Some(allow) = only_predicates
                    && !allow.iter().any(|p| p == predicate)
                {
                    continue;
                }
                relation_rules.push(RelationRule {
                    source: source.clone(),
                    relation: predicate.clone(),
                    target: target.clone(),
                    when_any: triggers.clone(),
                    require_in_sentence: Vec::new(),
                    trigger_provenance: BTreeMap::new(),
                });
            }
        }
    }

    SpaceType {
        id: "webnlg-pilot-v1".to_string(),
        name: String::new(),
        version: 1,
        description: String::new(),
        entities: entity_defs,
        relation_rules,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::webnlg::Lane;
    use crate::webnlg::construct_document;
    use crate::webnlg::model::Triple;

    fn oracle(s: &str, p: &str, o: &str) -> Triple {
        Triple {
            subject: s.into(),
            predicate: p.into(),
            object: o.into(),
        }
    }

    #[test]
    fn date_aliases_cover_common_prose_renderings() {
        let a = date_aliases("1982-07-23");
        assert!(a.contains(&"July 23, 1982".to_string()));
        assert!(a.contains(&"23 July 1982".to_string()));
        assert!(a.contains(&"23rd July 1982".to_string()));
        // Ordinal exceptions and non-dates.
        assert!(date_aliases("1988-01-08").contains(&"8th January 1988".to_string()));
        assert!(date_aliases("1900-01-11").contains(&"11th January 1900".to_string()));
        assert!(date_aliases("Aarhus").is_empty());
        assert!(date_aliases("1982-13-40").is_empty());
    }

    #[test]
    fn date_entity_grounds_from_prose_and_keeps_the_iso_token() {
        // The oracle object is an ISO date; the text renders it in prose. The
        // date alias lets the trigger fire, and the constructed object is still
        // the ISO token, so it matches the oracle (W6).
        let entities = vec!["Ace_Wilder".to_string(), "1982-07-23".to_string()];
        let gold = vec![oracle("Ace_Wilder", "birthDate", "1982-07-23")];
        let mut rules = RuleSet {
            name: "t".to_string(),
            predicates: BTreeMap::new(),
        };
        rules.predicates.insert(
            "birthDate".to_string(),
            vec!["{source} was born on {target}".to_string()],
        );
        let built = construct_document(
            Lane::Generic,
            "Ace Wilder was born on July 23, 1982.",
            &entities,
            &gold,
            &rules,
        );
        assert!(built.contains(&oracle("Ace_Wilder", "birthDate", "1982-07-23")));
    }

    #[test]
    fn generic_rule_grounds_a_provided_pair_from_text() {
        // Entities are provided (W2); the generic 'leader' trigger must fire.
        let entities = vec!["Aarhus".to_string(), "Jacob_Bundsgaard".to_string()];
        let gold = vec![oracle("Aarhus", "leader", "Jacob_Bundsgaard")];
        let built = construct_document(
            Lane::Generic,
            "The leader of Aarhus is Jacob Bundsgaard.",
            &entities,
            &gold,
            &RuleSet::generic(),
        );
        assert!(built.contains(&oracle("Aarhus", "leader", "Jacob_Bundsgaard")));
    }

    #[test]
    fn oracle_diagnostic_restricts_to_gold_predicate() {
        // Text supports BOTH a leader and a location edge for the same pair.
        let entities = vec!["Aarhus".to_string(), "Denmark".to_string()];
        let text = "The leader of Aarhus is Denmark. Aarhus is located in Denmark.";
        let gold = vec![oracle("Aarhus", "leader", "Denmark")];

        // Generic instantiates every predicate, so both edges ground.
        let generic =
            construct_document(Lane::Generic, text, &entities, &gold, &RuleSet::generic());
        assert!(generic.contains(&oracle("Aarhus", "leader", "Denmark")));
        assert!(generic.contains(&oracle("Aarhus", "location", "Denmark")));

        // OracleDiagnostic instantiates ONLY the gold predicate, so the
        // text-supported non-gold 'location' edge is excluded.
        let diagnostic = construct_document(
            Lane::OracleDiagnostic,
            text,
            &entities,
            &gold,
            &RuleSet::generic(),
        );
        assert!(diagnostic.contains(&oracle("Aarhus", "leader", "Denmark")));
        assert!(!diagnostic.iter().any(|t| t.predicate == "location"));
    }
}
