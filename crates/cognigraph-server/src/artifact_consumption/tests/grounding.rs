//! Grounding.

use super::*;

#[test]
fn m22_work_guard_counts_every_blocker_phrase_and_template_scan() {
    let chunks = vec![Chunk {
        id: "chunk-1".into(),
        title: String::new(),
        text: "No construction phrase is present.".into(),
    }];
    let entities = |aliases: Vec<String>| EntityDef {
        name: if aliases.first().is_some_and(|alias| alias.starts_with('s')) {
            "Source".into()
        } else {
            "Target".into()
        },
        entity_type: "organization".into(),
        aliases,
    };
    let space = SpaceType {
        id: "medical".into(),
        name: "Medical".into(),
        version: 1,
        description: String::new(),
        entities: vec![
            entities((0..9).map(|index| format!("source-{index}")).collect()),
            entities((0..9).map(|index| format!("target-{index}")).collect()),
        ],
        relation_rules: vec![RelationRule {
            source: "Source".into(),
            relation: "SUPPLIES".into(),
            target: "Target".into(),
            when_any: vec!["{source} supplies {target}".into()],
            require_in_sentence: Vec::new(),
            trigger_provenance: BTreeMap::new(),
        }],
    };
    let vetoes = vec![VetoRule {
        source: "Source".into(),
        relation: "SUPPLIES".into(),
        target: "Target".into(),
        when_any: (0..10).map(|index| format!("blocker-{index}")).collect(),
    }];

    assert!(validate_grounding_work(&space, &vetoes, &chunks, u64::MAX, 100).is_err());
    validate_grounding_work(&space, &vetoes, &chunks, u64::MAX, 1_000).unwrap();

    let heavy_space = SpaceType {
        entities: vec![
            entities((0..99).map(|index| format!("source-{index}")).collect()),
            entities((0..99).map(|index| format!("target-{index}")).collect()),
        ],
        relation_rules: (0..20)
            .map(|index| RelationRule {
                source: "Source".into(),
                relation: format!("RELATION_{index:02}"),
                target: "Target".into(),
                when_any: vec!["{source} supplies {target}".into()],
                require_in_sentence: Vec::new(),
                trigger_provenance: BTreeMap::new(),
            })
            .collect(),
        ..space
    };
    assert!(matches!(
        validate_semantic_repair_grounding_work(&heavy_space, &[], &chunks),
        Err(CogniGraphError::CapacityExceeded(_))
    ));
}
