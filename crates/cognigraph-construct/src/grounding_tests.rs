use super::*;

#[test]
fn sentence_gate_blocks_wrong_subject_chunks() {
    let space: SpaceType = serde_json::from_value(serde_json::json!({
        "id": "s",
        "entities": [
            {"name": "Harbor", "type": "org", "aliases": []},
            {"name": "Northstar", "type": "org", "aliases": ["NS"]},
            {"name": "Stack+", "type": "platform", "aliases": []}
        ],
        "relation_rules": [
            {"source": "Harbor", "relation": "SELECTED", "target": "Stack+",
             "when_any": ["selected stack+"],
             "require_in_sentence": ["source"]}
        ]
    }))
    .unwrap();

    // The leakage case: trigger affirmed in a sentence about ANOTHER
    // company — the gate refuses even though the chunk mentions Harbor.
    let leak = "Harbor is discussed elsewhere in this report. \
                Northstar selected Stack+ for its commercial stack.";
    assert!(ground_chunk("c1", leak, &space, &[]).is_empty());

    // The legitimate case grounds, and the span sits in the right
    // sentence.
    let legit = "Northstar went another way. Harbor selected Stack+ last spring.";
    let grounded = ground_chunk("c2", legit, &space, &[]);
    assert_eq!(grounded.len(), 1);
    let (start, end) = grounded[0].trigger_span;
    assert_eq!(&legit[start..end], "selected Stack+");
    assert!(start > 28, "must be the second sentence's occurrence");

    // Per-occurrence: a first occurrence in a wrong-subject sentence
    // does not suppress a later one in a satisfying sentence.
    let both = "Northstar selected Stack+ in 2024. Later, Harbor selected Stack+ too.";
    let grounded = ground_chunk("c3", both, &space, &[]);
    assert_eq!(grounded.len(), 1);
    assert!(grounded[0].trigger_span.0 > 34);

    // Aliases count as surfaces for the gate.
    let space_gated_alias: SpaceType = serde_json::from_value(serde_json::json!({
        "id": "s",
        "entities": [
            {"name": "Northstar", "type": "org", "aliases": ["NS"]},
            {"name": "Stack+", "type": "platform", "aliases": []}
        ],
        "relation_rules": [
            {"source": "Northstar", "relation": "SELECTED", "target": "Stack+",
             "when_any": ["selected stack+"],
             "require_in_sentence": ["source"]}
        ]
    }))
    .unwrap();
    let via_alias = "NS selected Stack+ after a long evaluation.";
    assert_eq!(
        ground_chunk("c4", via_alias, &space_gated_alias, &[]).len(),
        1
    );

    // A typo'd gate value fails loudly at deserialization.
    let bad = serde_json::from_value::<SpaceType>(serde_json::json!({
        "id": "s", "entities": [],
        "relation_rules": [{"source": "A", "relation": "R", "target": "B",
                            "when_any": [], "require_in_sentence": ["sorce"]}]
    }));
    assert!(bad.is_err());
}

#[test]
fn template_triggers_are_direction_faithful() {
    let space: SpaceType = serde_json::from_value(serde_json::json!({
        "id": "s",
        "entities": [
            {"name": "CedarWorks", "type": "org", "aliases": []},
            {"name": "MapleSystems", "type": "org", "aliases": ["MapleSystems Inc"]}
        ],
        "relation_rules": [
            {"source": "CedarWorks", "relation": "ACQUIRED", "target": "MapleSystems",
             "when_any": ["{source} acquired {target}",
                          "{target} was acquired by {source}"]}
        ]
    }))
    .unwrap();

    // Correct direction grounds — via either phrasing, alias included.
    let forward = "In May, CedarWorks acquired MapleSystems for an undisclosed sum.";
    let grounded = ground_chunk("c1", forward, &space, &[]);
    assert_eq!(grounded.len(), 1);
    assert_eq!(grounded[0].trigger, "CedarWorks acquired MapleSystems");
    let (start, end) = grounded[0].trigger_span;
    assert_eq!(&forward[start..end], "CedarWorks acquired MapleSystems");

    let passive = "MapleSystems Inc was acquired by CedarWorks.";
    let grounded = ground_chunk("c2", passive, &space, &[]);
    assert_eq!(grounded.len(), 1);
    assert_eq!(
        grounded[0].trigger,
        "MapleSystems Inc was acquired by CedarWorks"
    );

    // The REVERSED phrasing does not match any expansion: the rule
    // whose direction contradicts the text stays silent.
    let reversed = "MapleSystems acquired CedarWorks, sources claimed.";
    assert!(ground_chunk("c3", reversed, &space, &[]).is_empty());

    // Negation awareness applies to expansions like any trigger.
    let negated = "It is not true that CedarWorks acquired MapleSystems.";
    assert!(ground_chunk("c4", negated, &space, &[]).is_empty());
}

#[test]
fn template_surfaces_are_substituted_literally_once() {
    let space: SpaceType = serde_json::from_value(serde_json::json!({
        "id": "s",
        "entities": [
            {"name": "Source", "type": "org", "aliases": ["{target}"]},
            {"name": "Target", "type": "org", "aliases": ["Widget"]}
        ],
        "relation_rules": [
            {"source": "Source", "relation": "SUPPLIES", "target": "Target",
             "when_any": ["{source} supplies {target}"]}
        ]
    }))
    .unwrap();

    let literal = "The contract says {target} supplies Widget.";
    let grounded = ground_chunk("literal", literal, &space, &[]);
    assert_eq!(grounded.len(), 1);
    assert_eq!(grounded[0].trigger, "{target} supplies Widget");
    let (start, end) = grounded[0].trigger_span;
    assert_eq!(&literal[start..end], "{target} supplies Widget");

    // A placeholder-shaped source surface must not be reinterpreted by
    // the later target substitution.
    assert!(ground_chunk("re-expanded", "Widget supplies Widget.", &space, &[]).is_empty());
}

#[test]
fn sentence_gate_caches_one_decision_per_sentence() {
    let text = "trigger trigger trigger. Source trigger.";
    let text_cf = text.to_lowercase();
    let required_surfaces = ["source".to_string()];
    let required = vec![required_surfaces.as_slice()];
    let sentence_ranges = sentence_ranges_cf(&text_cf);
    let cache = RefCell::new(HashMap::new());
    let decisions: Vec<bool> = text_cf
        .match_indices("trigger")
        .map(|(offset, _)| {
            sentence_gate_accepts(&text_cf, &required, &sentence_ranges, &cache, offset)
        })
        .collect();

    assert_eq!(decisions, vec![false, false, false, true]);
    assert_eq!(
        cache.borrow().len(),
        2,
        "repeated occurrences must share the sentence-scoped scan"
    );
}

#[test]
fn effective_config_indexes_large_alias_batches_without_changing_order() {
    let space: SpaceType = serde_json::from_value(serde_json::json!({
        "id": "s",
        "entities": [{"name": "Source", "type": "org", "aliases": []}],
        "relation_rules": []
    }))
    .unwrap();
    let neurons = (0..10_000)
        .map(|index| Neuron {
            id: format!("alias-{index}"),
            kind: NeuronKind::Alias,
            status: NeuronStatus::Accepted,
            evidence: vec!["reviewed evidence".into()],
            entity: "Source".into(),
            aliases: vec![format!("surface-{index}")],
            ..Neuron::default()
        })
        .collect::<Vec<_>>();

    let effective = effective_config(&space, &neurons);

    assert_eq!(effective.entities[0].aliases.len(), neurons.len());
    assert_eq!(effective.entities[0].aliases[0], "surface-0");
    assert_eq!(effective.entities[0].aliases[9_999], "surface-9999");
}

#[test]
fn trigger_span_points_at_the_licensing_occurrence() {
    let space: SpaceType = serde_json::from_value(serde_json::json!({
        "id": "s",
        "entities": [
            {"name": "Nimbus", "type": "vendor"},
            {"name": "DataCloud", "type": "platform"}
        ],
        "relation_rules": [
            {"source": "Nimbus", "relation": "SUPPLIES", "target": "DataCloud",
             "when_any": ["datacloud runs on nimbus"]}
        ]
    }))
    .unwrap();

    // Plain ASCII: the span slices the original text to the trigger.
    let text = "Everyone knows DataCloud runs on Nimbus these days.";
    let grounded = ground_chunk("c1", text, &space, &[]);
    let (start, end) = grounded[0].trigger_span;
    assert_eq!(&text[start..end], "DataCloud runs on Nimbus");

    // First occurrence negated, second affirms: the span must point
    // at the SECOND — the occurrence that actually licenses the fact.
    let text = "It is not true that DataCloud runs on Nimbus, some say. \
                But in production, DataCloud runs on Nimbus.";
    let grounded = ground_chunk("c2", text, &space, &[]);
    let (start, end) = grounded[0].trigger_span;
    assert_eq!(&text[start..end], "DataCloud runs on Nimbus");
    assert!(start > 60, "span must be the affirmed second occurrence");

    // Non-ASCII prefix whose casefold EXPANDS ('İ' folds to 2 chars):
    // casefolded offsets diverge from original bytes; the span must
    // still slice the original text correctly.
    let text = "İİ say: DataCloud runs on Nimbus.";
    let grounded = ground_chunk("c3", text, &space, &[]);
    let (start, end) = grounded[0].trigger_span;
    assert_eq!(&text[start..end], "DataCloud runs on Nimbus");
}

#[test]
fn sentence_bounds_keep_dotted_names_whole() {
    // "Chorus.ai" / "OpenProtein.AI": a '.' flanked by non-space is
    // part of the token — the chunk-sensitivity simulation showed
    // mid-name splits were the entire measured cost of adversarial
    // re-chunking, and they truncate the sentence a gate judges.
    let space: SpaceType = serde_json::from_value(serde_json::json!({
        "id": "s",
        "entities": [
            {"name": "Chorus.ai", "type": "org", "aliases": []},
            {"name": "ZoomInfo", "type": "org", "aliases": []}
        ],
        "relation_rules": [
            {"source": "Chorus.ai", "relation": "ACQUIRED_BY", "target": "ZoomInfo",
             "when_any": ["chorus.ai (owned by zoominfo)"],
             "require_in_sentence": ["source", "target"]}
        ]
    }))
    .unwrap();
    let text = "Rivals shifted. Chorus.ai (owned by ZoomInfo) grew fast.";
    let grounded = ground_chunk("c1", text, &space, &[]);
    assert_eq!(
        grounded.len(),
        1,
        "dotted name must not split the gated sentence"
    );
    let (start, end) = sentence_bounds(text, text.find("owned").unwrap());
    assert_eq!(
        &text[start..end].trim_start(),
        &"Chorus.ai (owned by ZoomInfo) grew fast."
    );
}

#[test]
fn sentence_gate_survives_decimal_points() {
    // "$3.09B" must not split the sentence at the decimal point: the
    // gate judges the whole sentence, which names the source entity.
    let space: SpaceType = serde_json::from_value(serde_json::json!({
        "id": "s",
        "entities": [
            {"name": "CIP Market", "type": "market", "aliases": []},
            {"name": "$3.09B in 2024", "type": "figure", "aliases": ["$3.09 billion"]}
        ],
        "relation_rules": [
            {"source": "CIP Market", "relation": "HAS_SIZE", "target": "$3.09B in 2024",
             "when_any": ["estimated at $3.09 billion"],
             "require_in_sentence": ["source"]}
        ]
    }))
    .unwrap();
    let text = "Analysts disagree. The CIP Market was estimated at $3.09 billion in 2024.";
    let grounded = ground_chunk("c1", text, &space, &[]);
    assert_eq!(
        grounded.len(),
        1,
        "decimal point must not truncate the gated sentence"
    );

    // A real sentence boundary right after a digit still bounds:
    // "in 2024. Elsewhere..." — '.' followed by a space is a boundary.
    let (start, end) = sentence_bounds(text, text.find("estimated").unwrap());
    assert_eq!(text[start..end].trim_start(), &text[19..]);
}

#[test]
fn case_bravo_negation_regression() {
    // The exact failure the research fixed: a retraction must not ground.
    assert!(!affirms_phrase(
        "Later analysis showed it is not true that the iPhone 14 contained Signal.",
        "iPhone 14 contained Signal"
    ));
    // An affirmative later clause still grounds despite an earlier negation.
    assert!(affirms_phrase(
        "It was not clear at first. The iPhone 14 contained Signal.",
        "iPhone 14 contained Signal"
    ));
    // Contractions count as negation cues.
    assert!(!affirms_phrase(
        "The device didn't run the Signal app build",
        "run the Signal app"
    ));
    assert!(affirms_phrase(
        "We stand with Ukraine.",
        "stand with Ukraine"
    ));
}

#[test]
fn repeated_negated_occurrences_use_one_linear_clause_index() {
    let text = format!("not {}", "x ".repeat(524_286));
    assert_eq!(text.len(), 1_048_576);
    assert!(!affirms_phrase(&text, "x"));
}

/// The motivating cases: an entity name that CONTAINS an abbreviation dot
/// must not be cut in half by sentence splitting, or the relation-semantics
/// detector reports a correct fact as target-absent.
#[test]
fn abbreviation_dots_do_not_end_a_sentence() {
    let text = "Avoid concomitant use with St. John's Wort or Rifampin.";
    let (start, end) = sentence_bounds(text, text.find("Avoid").unwrap());
    assert_eq!(
        text[start..end].trim(),
        text,
        "St. must not split the sentence"
    );

    let text = "The 10 mg strength contains D C Yellow No. 10 Aluminum Lake.";
    let (start, end) = sentence_bounds(text, 0);
    assert!(
        text[start..end].contains("Aluminum Lake"),
        "No. must not split mid-entity-name: {:?}",
        &text[start..end]
    );
}

/// The dangerous direction. Failing to split MERGES sentences, and a
/// `require_in_sentence` gate then judges a longer span and admits more
/// groundings — so tokens that genuinely end sentences in label prose must
/// keep splitting, and an ordinary sentence must be unaffected.
#[test]
fn genuine_sentence_ends_still_split() {
    for text in [
        // Corporate suffixes end the manufacturer line constantly.
        "Manufactured by Aurobindo Pharma USA, Inc. Distributed nationwide.",
        "Store below 25C, protect from light, etc. Dispense in a tight container.",
        "Alpha selected Beta. Gamma declined.",
    ] {
        let second = text.rfind(". ").expect("two sentences") + 2;
        let (start, end) = sentence_bounds(text, second);
        // `start` lands just after the boundary dot, i.e. on the space.
        assert_eq!(
            text[start..end].trim(),
            text[second..].trim(),
            "must still split before {:?} in {text:?}",
            &text[second..]
        );
    }
}

/// The abbreviation check runs on casefolded text too (the gate's hot path
/// indexes sentences over `text.to_lowercase()`), so both coordinate systems
/// must agree or a gate and its advisor would disagree about the sentence.
#[test]
fn abbreviation_handling_is_case_insensitive() {
    let upper = "TAKE WITH ST. JOHN'S WORT DAILY.";
    let (start, end) = sentence_bounds(upper, 0);
    assert_eq!(upper[start..end].trim(), upper);
    let lower = upper.to_lowercase();
    let (lstart, lend) = sentence_bounds(&lower, 0);
    assert_eq!(
        (start, end),
        (lstart, lend),
        "casefolded and original splitting must agree"
    );
}
