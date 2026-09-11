//! Accepted Semantic Neuron hints for the shared retrieval ranker.

use crate::types::{Neuron, NeuronKind, NeuronStatus};
pub use cognigraph_core::graph_ranking::*;
use std::collections::HashMap;
#[cfg(test)]
use std::collections::HashSet;

/// Per-relation score boosts from ACCEPTED `relation_rank_hint` neurons.
/// Additive and order-independent: multiple hints for one relation sum.
/// Proposed/rejected/retired hints contribute nothing — same governance
/// boundary as construction neurons.
pub fn rank_boosts(neurons: &[Neuron]) -> HashMap<String, f64> {
    let mut boosts: HashMap<String, f64> = HashMap::new();
    for neuron in neurons {
        if neuron.kind == NeuronKind::RelationRankHint && neuron.status == NeuronStatus::Accepted {
            *boosts.entry(neuron.relation.to_uppercase()).or_default() += neuron.boost;
        }
    }
    boosts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Neuron, NeuronKind, NeuronStatus};

    fn edge(source: &str, relation: &str, target: &str, chunk: &str) -> GraphEdge {
        GraphEdge {
            source: source.into(),
            relation: relation.into(),
            target: target.into(),
            evidence_chunk_id: chunk.into(),
        }
    }

    fn rank_hint(relation: &str, boost: f64, status: NeuronStatus) -> Neuron {
        Neuron {
            id: format!("boost-{}", relation.to_lowercase()),
            kind: NeuronKind::RelationRankHint,
            status,
            confidence: 0.9,
            rationale: "test".into(),
            evidence: vec!["trace review".into()],
            relation: relation.into(),
            boost,
            ..Neuron::default()
        }
    }

    #[test]
    fn static_scores_match_research() {
        let none = HashMap::new();
        assert_eq!(relation_score("SUPPORTS", &none), 100.0);
        assert_eq!(relation_score("MENTIONS", &none), 20.0);
        assert_eq!(relation_score("CO_OCCURS_WITH", &none), 10.0);
        assert_eq!(relation_score("mentions", &none), 20.0); // case-insensitive
    }

    #[test]
    fn accepted_boost_reorders_proposed_stays_inert() {
        let edges = vec![
            edge("a", "SUPPORTS", "b", "c1"),
            edge("a", "MENTIONS", "b", "c1"),
        ];
        let seeds = HashSet::new();
        let opts = EdgeSelectOpts::default();

        // Proposed hint: no effect — governance boundary holds.
        let proposed = [rank_hint("MENTIONS", 90.0, NeuronStatus::Proposed)];
        let picked = select_graph_edges(&edges, &seeds, &rank_boosts(&proposed), &opts);
        assert_eq!(picked[0].relation, "SUPPORTS");

        // Accepted hint: MENTIONS (20 + 90 = 110) now outranks SUPPORTS.
        let accepted = [rank_hint("MENTIONS", 90.0, NeuronStatus::Accepted)];
        let picked = select_graph_edges(&edges, &seeds, &rank_boosts(&accepted), &opts);
        assert_eq!(picked[0].relation, "MENTIONS");
    }

    #[test]
    fn negative_boost_demotes_noisy_relation() {
        let edges = vec![
            edge("a", "CO_LOCATED", "b", "c1"),
            edge("a", "SUPPORTS", "b", "c1"),
        ];
        let accepted = [rank_hint("CO_LOCATED", -50.0, NeuronStatus::Accepted)];
        let picked = select_graph_edges(
            &edges,
            &HashSet::new(),
            &rank_boosts(&accepted),
            &EdgeSelectOpts::default(),
        );
        assert_eq!(picked[0].relation, "SUPPORTS");
    }

    #[test]
    fn boosts_for_one_relation_sum() {
        let hints = [rank_hint("MENTIONS", 30.0, NeuronStatus::Accepted), {
            let mut second = rank_hint("MENTIONS", 25.0, NeuronStatus::Accepted);
            second.id = "boost-mentions-2".into();
            second
        }];
        assert_eq!(rank_boosts(&hints).get("MENTIONS"), Some(&55.0));
    }

    #[test]
    fn low_signal_caps_hold_even_when_boosted() {
        // A boost changes ordering, not the low-signal budget: MENTIONS
        // edges still cannot crowd out the trace.
        let mut edges: Vec<GraphEdge> = (0..8)
            .map(|i| edge(&format!("e{i}"), "MENTIONS", &format!("f{i}"), "c1"))
            .collect();
        edges.push(edge("a", "SUPPORTS", "b", "c1"));
        let accepted = [rank_hint("MENTIONS", 500.0, NeuronStatus::Accepted)];
        let picked = select_graph_edges(
            &edges,
            &HashSet::new(),
            &rank_boosts(&accepted),
            &EdgeSelectOpts::default(),
        );
        let low = picked.iter().filter(|e| is_low_signal(&e.relation)).count();
        assert_eq!(low, 4, "global low-signal cap");
        assert!(picked.iter().any(|e| e.relation == "SUPPORTS"));
    }

    #[test]
    fn per_entity_low_signal_cap_and_seed_tiebreak() {
        let edges = vec![
            edge("hub", "MENTIONS", "x1", "c1"),
            edge("hub", "MENTIONS", "x2", "c2"),
            edge("hub", "MENTIONS", "x3", "c3"),
            edge("other", "MENTIONS", "y", "c4"),
        ];
        let picked = select_graph_edges(
            &edges,
            &HashSet::new(),
            &HashMap::new(),
            &EdgeSelectOpts::default(),
        );
        let hub = picked.iter().filter(|e| e.source == "hub").count();
        assert_eq!(hub, 2, "per-entity low-signal cap");

        // Seed adjacency breaks ties within equal relation scores.
        let seeds: HashSet<String> = ["x2".to_string()].into();
        let picked =
            select_graph_edges(&edges, &seeds, &HashMap::new(), &EdgeSelectOpts::default());
        assert_eq!(picked[0].target, "x2");
    }

    #[test]
    fn selection_is_deterministic_and_dedupes() {
        let edges = vec![
            edge("a", "SUPPORTS", "b", "c1"),
            edge("a", "SUPPORTS", "b", "c2"), // same semantic triple, other chunk
            edge("a", "MENTIONS", "b", "c1"),
            edge("a", "MENTIONS", "b", "c1"), // exact duplicate
            edge("a", "MENTIONS", "b", "c2"), // low-signal: distinct per chunk
        ];
        let picked = select_graph_edges(
            &edges,
            &HashSet::new(),
            &HashMap::new(),
            &EdgeSelectOpts::default(),
        );
        let triples: Vec<&str> = picked.iter().map(|e| e.relation.as_str()).collect();
        assert_eq!(triples, ["SUPPORTS", "MENTIONS", "MENTIONS"]);
    }

    #[test]
    fn validation_bounds_rank_hints() {
        use crate::types::{NeuronSet, SpaceType};
        use crate::validate::{NeuronError, validate_neurons};
        let space = SpaceType {
            id: "test-space".into(),
            relation_rules: vec![crate::types::RelationRule {
                source: "A".into(),
                relation: "SUPPORTS".into(),
                target: "B".into(),
                when_any: vec!["supports".into()],
                require_in_sentence: vec![],
                trigger_provenance: Default::default(),
            }],
            ..serde_json::from_str(r#"{"id": "test-space"}"#).unwrap()
        };
        let set = |neuron: Neuron| NeuronSet {
            space_type: "test-space".into(),
            neurons: vec![neuron],
        };

        // Configured relation and built-in low-signal relation both validate.
        validate_neurons(
            &set(rank_hint("SUPPORTS", -30.0, NeuronStatus::Proposed)),
            &space,
        )
        .unwrap();
        validate_neurons(
            &set(rank_hint("MENTIONS", 15.0, NeuronStatus::Proposed)),
            &space,
        )
        .unwrap();

        // A relation the space cannot produce is rejected.
        let err = validate_neurons(
            &set(rank_hint("INVENTED", 5.0, NeuronStatus::Proposed)),
            &space,
        )
        .unwrap_err();
        assert!(matches!(err, NeuronError::UnknownRelation(_, _)));

        // Zero and non-finite boosts are rejected.
        for bad in [0.0, f64::NAN, f64::INFINITY] {
            let err = validate_neurons(
                &set(rank_hint("SUPPORTS", bad, NeuronStatus::Proposed)),
                &space,
            )
            .unwrap_err();
            assert!(matches!(err, NeuronError::BadBoost(_, _)));
        }
    }

    #[test]
    fn rank_hints_never_touch_construction_config() {
        use crate::grounding::effective_config;
        use crate::types::SpaceType;
        let space: SpaceType = serde_json::from_str(r#"{"id": "test-space"}"#).unwrap();
        let hint = rank_hint("MENTIONS", 50.0, NeuronStatus::Accepted);
        let config = effective_config(&space, &[hint]);
        assert!(config.relation_rules.is_empty());
        assert!(config.entities.is_empty());
    }

    #[test]
    fn from_value_reads_backend_edges() {
        let value = serde_json::json!({
            "_from": "entities/crowdstrike",
            "_to": "entities/faulty-update",
            "relation_type": "RESPONDED_WITH",
            "evidence_chunk_id": "cs-003"
        });
        let parsed = GraphEdge::from_value(&value).unwrap();
        assert_eq!(parsed.relation, "RESPONDED_WITH");
        assert_eq!(parsed.evidence_chunk_id, "cs-003");
        assert!(GraphEdge::from_value(&serde_json::json!({"_from": "x"})).is_none());
    }
}
