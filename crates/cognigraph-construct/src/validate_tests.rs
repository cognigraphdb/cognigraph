use std::collections::BTreeMap;

use super::*;
use crate::types::{EntityDef, RelationRule};

#[test]
fn large_candidate_validation_uses_indexed_membership_and_conflicts() {
    let entities = (0..4_001)
        .map(|index| EntityDef {
            name: format!("entity-{index}"),
            entity_type: "node".into(),
            aliases: Vec::new(),
        })
        .collect::<Vec<_>>();
    let space = SpaceType {
        id: "indexed-space".into(),
        name: String::new(),
        version: 1,
        description: String::new(),
        entities,
        relation_rules: vec![RelationRule {
            source: "entity-0".into(),
            relation: "LINKS".into(),
            target: "entity-1".into(),
            when_any: vec!["links".into()],
            require_in_sentence: Vec::new(),
            trigger_provenance: BTreeMap::new(),
        }],
    };
    let mut neurons = Vec::with_capacity(4_000);
    for index in 1..=2_000 {
        neurons.push(Neuron {
            id: format!("hint-{index}"),
            kind: NeuronKind::RelationHint,
            status: NeuronStatus::Accepted,
            evidence: vec!["reviewed evidence".into()],
            source: "entity-0".into(),
            relation: "LINKS".into(),
            target: format!("entity-{index}"),
            triggers: vec![format!("hint {index}")],
            ..Neuron::default()
        });
        neurons.push(Neuron {
            id: format!("blocker-{index}"),
            kind: NeuronKind::RelationBlocker,
            status: NeuronStatus::Accepted,
            evidence: vec!["reviewed evidence".into()],
            source: format!("entity-{index}"),
            relation: "LINKS".into(),
            target: "entity-0".into(),
            triggers: vec![format!("blocked {index}")],
            ..Neuron::default()
        });
    }

    validate_neurons(
        &NeuronSet {
            space_type: space.id.clone(),
            neurons,
        },
        &space,
    )
    .unwrap();
}
