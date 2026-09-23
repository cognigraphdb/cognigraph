use super::*;
use crate::types::{EntityDef, RelationRule, SpaceType};
use cognigraph_core::{CollectionType, GraphBackend};
use cognigraph_native::NativeBackend;
use serde_json::json;

fn entity(name: &str, kind: &str, aliases: &[&str]) -> EntityDef {
    EntityDef {
        name: name.into(),
        entity_type: kind.into(),
        aliases: aliases.iter().map(|a| a.to_string()).collect(),
    }
}

fn rule(source: &str, relation: &str, target: &str) -> RelationRule {
    serde_json::from_value(json!({"source": source, "relation": relation, "target": target}))
        .unwrap()
}

fn space(rules: Vec<RelationRule>) -> SpaceType {
    SpaceType {
        id: "supply".into(),
        name: String::new(),
        version: 1,
        description: String::new(),
        entities: vec![
            entity("Meridian Labs", "Company", &["Meridian"]),
            entity("Compound X", "Product", &["CX-9"]),
            entity("Northwind", "Company", &[]),
            entity("Oslo", "City", &[]),
        ],
        relation_rules: rules,
    }
}

fn view(id: &str, question: &str, answer: &str) -> SideViewText {
    SideViewText {
        id: id.into(),
        question: question.into(),
        answer: answer.into(),
    }
}

fn fact(source: &str, relation: &str, target: &str) -> Fact {
    Fact {
        source: source.into(),
        relation: relation.into(),
        target: target.into(),
    }
}

#[test]
fn one_matching_rule_yields_a_candidate_in_the_rules_direction() {
    let space = space(vec![rule("Meridian Labs", "SUPPLIES", "Compound X")]);
    // The product is named first; the rule still orients the fact.
    let report = pair_candidates(
        &space,
        &[view(
            "sv1",
            "Who makes CX-9?",
            "Compound X is supplied by Meridian.",
        )],
    );
    assert_eq!(report.scanned, 1);
    assert_eq!(report.candidates.len(), 1, "{report:?}");
    assert_eq!(
        report.candidates[0].fact,
        fact("Meridian Labs", "SUPPLIES", "Compound X")
    );
    assert_eq!(report.candidates[0].side_views, vec!["sv1"]);
    assert!(report.skipped.is_empty());
}

#[test]
fn several_fitting_relations_or_directions_are_ambiguous_and_never_proposed() {
    let space = space(vec![
        rule("Meridian Labs", "SUPPLIES", "Compound X"),
        rule("Meridian Labs", "RECALLS", "Compound X"),
    ]);
    let report = pair_candidates(&space, &[view("sv1", "q", "Meridian and Compound X")]);
    assert!(report.candidates.is_empty());
    assert_eq!(report.skipped.len(), 1);
    let skip = &report.skipped[0];
    assert_eq!(skip.reason, "ambiguous_relation");
    assert_eq!(
        skip.detail.as_deref(),
        Some("Meridian Labs --RECALLS--> Compound X; Meridian Labs --SUPPLIES--> Compound X")
    );

    // A relation declared both ways between two entities fits both directions.
    let space = self::space(vec![
        rule("Meridian Labs", "PARTNERS_WITH", "Northwind"),
        rule("Northwind", "PARTNERS_WITH", "Meridian Labs"),
    ]);
    let report = pair_candidates(
        &space,
        &[view("sv2", "q", "Meridian partners with Northwind")],
    );
    assert_eq!(report.skipped[0].reason, "ambiguous_relation");
}

#[test]
fn a_pair_no_rule_fits_is_reported_as_unmatched() {
    let space = space(vec![rule("Meridian Labs", "SUPPLIES", "Compound X")]);
    let report = pair_candidates(&space, &[view("sv1", "Where is Meridian?", "In Oslo.")]);
    assert!(report.candidates.is_empty());
    assert_eq!(
        (
            report.skipped[0].reason,
            report.skipped[0].source.as_str(),
            report.skipped[0].target.as_str()
        ),
        ("no_matching_rule", "Meridian Labs", "Oslo")
    );
}

#[test]
fn entities_are_found_by_alias_case_insensitively_across_question_and_answer() {
    let space = space(vec![rule("Meridian Labs", "SUPPLIES", "Compound X")]);
    let report = pair_candidates(
        &space,
        &[view("sv1", "WHO SUPPLIES cx-9?", "meridian does.")],
    );
    assert_eq!(
        report.candidates[0].fact,
        fact("Meridian Labs", "SUPPLIES", "Compound X")
    );
}

#[test]
fn single_mentions_repeated_mentions_and_three_entities() {
    let space = space(vec![
        rule("Meridian Labs", "SUPPLIES", "Compound X"),
        rule("Meridian Labs", "LOCATED_IN", "Oslo"),
    ]);
    let lone = pair_candidates(
        &space,
        &[view(
            "sv1",
            "Meridian?",
            "Meridian Labs, also called Meridian.",
        )],
    );
    assert!(lone.candidates.is_empty() && lone.skipped.is_empty());
    let three = pair_candidates(
        &space,
        &[view("sv2", "q", "Meridian in Oslo supplies Compound X")],
    );
    let facts: Vec<&Fact> = three.candidates.iter().map(|c| &c.fact).collect();
    assert_eq!(
        facts,
        vec![
            &fact("Meridian Labs", "LOCATED_IN", "Oslo"),
            &fact("Meridian Labs", "SUPPLIES", "Compound X")
        ]
    );
    assert_eq!(three.skipped.len(), 1, "Compound X / Oslo has no rule");
}

#[test]
fn repeated_evidence_aggregates_into_one_candidate_ordered_by_support() {
    let space = space(vec![
        rule("Meridian Labs", "SUPPLIES", "Compound X"),
        rule("Meridian Labs", "LOCATED_IN", "Oslo"),
    ]);
    let report = pair_candidates(
        &space,
        &[
            view("sv3", "q", "Meridian ships Compound X"),
            view("sv1", "q", "Meridian is in Oslo"),
            view("sv2", "q", "CX-9 comes from Meridian"),
        ],
    );
    assert_eq!(report.scanned, 3);
    assert_eq!(
        report.candidates[0].fact,
        fact("Meridian Labs", "SUPPLIES", "Compound X")
    );
    assert_eq!(report.candidates[0].side_views, vec!["sv2", "sv3"]);
    assert_eq!(report.candidates[1].side_views, vec!["sv1"]);
}

async fn graph_with(edges: &[(&str, &str, &str, &str)]) -> NativeBackend {
    let backend = NativeBackend::new();
    backend
        .ensure_collection("facts", CollectionType::Edge)
        .await
        .unwrap();
    for (i, (source, relation, target, space_id)) in edges.iter().enumerate() {
        backend
            .bulk_insert(
                "facts",
                vec![json!({
                    "_key": format!("f{i}"),
                    "_from": format!("entities/{}", crate::ingest::entity_key(source)),
                    "_to": format!("entities/{}", crate::ingest::entity_key(target)),
                    "relation_type": relation,
                    "space_id": space_id,
                })],
            )
            .unwrap();
    }
    backend
}

#[tokio::test]
async fn endpoints_already_connected_in_this_space_are_skipped_in_either_direction() {
    let space = space(vec![rule("Meridian Labs", "SUPPLIES", "Compound X")]);
    let views = [view("sv1", "q", "Meridian supplies Compound X")];
    // Connected in this space, by any relation: not a gap.
    let backend = graph_with(&[("Meridian Labs", "RECALLS", "Compound X", "s1")]).await;
    let report = detect_sideview_gaps(&backend, &space, "s1", &views, 1)
        .await
        .unwrap();
    assert!(report.candidates.is_empty());
    assert_eq!(report.skipped[0].reason, "endpoints_connected");
    // An edge the other way round connects the same endpoints too.
    let reversed = graph_with(&[("Compound X", "SUPPLIES", "Meridian Labs", "s1")]).await;
    let report = detect_sideview_gaps(&reversed, &space, "s1", &views, 1)
        .await
        .unwrap();
    assert_eq!(report.skipped[0].reason, "endpoints_connected");
    // The same edge in another space does not.
    let other = detect_sideview_gaps(&backend, &space, "s2", &views, 1)
        .await
        .unwrap();
    assert_eq!(other.candidates.len(), 1);
    // Nor does an edge to a third entity.
    let elsewhere = graph_with(&[("Meridian Labs", "LOCATED_IN", "Oslo", "s1")]).await;
    assert_eq!(
        detect_sideview_gaps(&elsewhere, &space, "s1", &views, 1)
            .await
            .unwrap()
            .candidates
            .len(),
        1
    );
    // A graph without a facts collection at all has no edges.
    let empty = NativeBackend::new();
    assert_eq!(
        detect_sideview_gaps(&empty, &space, "s1", &views, 1)
            .await
            .unwrap()
            .candidates
            .len(),
        1
    );
}

#[tokio::test]
async fn candidates_below_the_support_threshold_are_reported_not_proposed() {
    let space = space(vec![
        rule("Meridian Labs", "SUPPLIES", "Compound X"),
        rule("Meridian Labs", "LOCATED_IN", "Oslo"),
    ]);
    let views = [
        view("sv1", "q", "Meridian ships Compound X"),
        view("sv2", "q", "CX-9 comes from Meridian"),
        view("sv3", "q", "Meridian is in Oslo"),
    ];
    let backend = NativeBackend::new();
    let report = detect_sideview_gaps(&backend, &space, "s1", &views, 2)
        .await
        .unwrap();
    assert_eq!(report.candidates.len(), 1);
    assert_eq!(report.candidates[0].side_views.len(), 2);
    assert_eq!(report.skipped[0].reason, "below_min_support");
    assert_eq!(report.skipped[0].side_views, vec!["sv3"]);
    let none = detect_sideview_gaps(&backend, &space, "s1", &[], 1)
        .await
        .unwrap();
    assert_eq!(
        (none.scanned, none.candidates.len(), none.skipped.len()),
        (0, 0, 0)
    );
}

#[test]
fn a_rule_listed_twice_is_one_fit_and_a_repeated_view_is_one_support() {
    let space = space(vec![
        rule("Meridian Labs", "SUPPLIES", "Compound X"),
        rule("Meridian Labs", "SUPPLIES", "Compound X"),
    ]);
    let report = pair_candidates(
        &space,
        &[
            view("sv1", "q", "Meridian ships Compound X"),
            view("sv1", "q", "Meridian ships Compound X"),
        ],
    );
    assert_eq!(report.scanned, 2);
    assert_eq!(report.candidates.len(), 1);
    assert_eq!(report.candidates[0].side_views, vec!["sv1"]);
}

#[test]
fn rule_endpoints_must_name_the_entities_exactly() {
    // Rules name entity instances; neither a type nor an alias nor a case
    // variant of the name is an endpoint.
    let space = space(vec![
        rule("Company", "SUPPLIES", "Product"),
        rule("Meridian", "SUPPLIES", "CX-9"),
        rule("meridian labs", "SUPPLIES", "compound x"),
    ]);
    let report = pair_candidates(&space, &[view("sv1", "q", "Meridian ships Compound X")]);
    assert!(report.candidates.is_empty());
    assert_eq!(report.skipped[0].reason, "no_matching_rule");
}
