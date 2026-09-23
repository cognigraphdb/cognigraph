//! Side views as a proposal source (CG-88).

use super::*;
use crate::system_collections::{SIDE_VIEWS_COLLECTION, is_generated_collection};
use cognigraph_core::CollectionType;

/// `fleet`: Nimbus HOSTS DataCloud (no triggers, so never grounded) and
/// DataCloud LOCATED_IN Oslo; Acme has no rule with anyone.
async fn fleet_state() -> AppState {
    let state = seeded_state().await;
    state
        .managed_backend
        .create_document(
            "space_types",
            json!({
                "_key": "fleet",
                "id": "fleet",
                "entities": [
                    {"name": "Nimbus", "type": "vendor", "aliases": ["Nimbus Cloud"]},
                    {"name": "DataCloud", "type": "platform", "aliases": []},
                    {"name": "Oslo", "type": "city", "aliases": []},
                    {"name": "Acme", "type": "customer", "aliases": []}
                ],
                "relation_rules": [
                    {"source": "Nimbus", "relation": "HOSTS", "target": "DataCloud", "when_any": []},
                    {"source": "DataCloud", "relation": "LOCATED_IN", "target": "Oslo", "when_any": []}
                ]
            }),
        )
        .await
        .unwrap();
    let views = [
        (
            "sv1",
            "notes/n1",
            "Who hosts DataCloud?",
            "Nimbus Cloud does.",
        ),
        ("sv2", "notes/n2", "Where does DataCloud run?", "On Nimbus."),
        ("sv3", "notes/n2", "Where is DataCloud?", "In Oslo."),
        ("sv4", "notes/n3", "Who uses Nimbus?", "Acme."),
        // Outside the selection: another collection and a prefix look-alike.
        ("sv5", "tickets/t1", "Who hosts DataCloud?", "Nimbus."),
        ("sv6", "notes2/x", "Who hosts DataCloud?", "Nimbus."),
    ];
    for (key, parent, question, answer) in views {
        state
            .managed_backend
            .create_document(
                SIDE_VIEWS_COLLECTION,
                json!({"_key": key, "kind": "side_view", "document_id": parent,
                       "question": question, "answer": answer}),
            )
            .await
            .unwrap();
    }
    // DataCloud and Oslo are already connected in this space.
    state
        .managed_backend
        .ensure_collection("facts", CollectionType::Edge)
        .await
        .unwrap();
    state
        .managed_backend
        .create_edge(
            "facts",
            json!({"_from": "entities/oslo", "_to": "entities/datacloud",
                   "relation_type": "SERVES", "space_id": "fleet"}),
        )
        .await
        .unwrap();
    state
}

fn select(extra: Value) -> SideViewSelection {
    let mut body = json!({"collection": "notes"});
    body.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    serde_json::from_value(body).unwrap()
}

fn request(selection: SideViewSelection) -> ProposeRequest {
    ProposeRequest {
        space_type: "fleet".into(),
        gaps: None,
        eval: None,
        side_views: Some(selection),
    }
}

async fn snapshot(state: &AppState, collections: &[&str]) -> Vec<Vec<Value>> {
    let mut out = Vec::new();
    for name in collections {
        out.push(
            state
                .managed_backend
                .list_documents(name, None, None)
                .await
                .unwrap_or_default(),
        );
    }
    out
}

const GRAPH: [&str; 6] = [
    "facts",
    "entities",
    "space_types",
    "chunks",
    "mentions",
    SIDE_VIEWS_COLLECTION,
];

#[tokio::test]
async fn dry_run_reports_candidates_without_a_provider_and_writes_nothing() {
    let state = fleet_state().await; // no completion provider
    let before = snapshot(&state, &GRAPH).await;
    let Json(response) = propose(
        State(state.clone()),
        None,
        Json(request(select(json!({"dry_run": true})))),
    )
    .await
    .unwrap();
    assert_eq!(response["source"], "side_views");
    assert_eq!(response["dry_run"], true);
    let report = &response["side_views"];
    assert_eq!(report["collection"], "notes");
    assert_eq!(report["scanned"], 4, "sv5 and sv6 are outside `notes/`");
    assert_eq!(
        report["candidates"],
        json!([{"fact": "Nimbus --HOSTS--> DataCloud", "support": 2,
                "side_views": ["sv1", "sv2"]}])
    );
    let reasons: Vec<&str> = report["skipped"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["reason"].as_str().unwrap())
        .collect();
    assert_eq!(reasons, vec!["no_matching_rule", "endpoints_connected"]);
    assert_eq!(snapshot(&state, &GRAPH).await, before);
    assert!(
        state
            .managed_backend
            .list_documents(NEURONS, None, None)
            .await
            .unwrap_or_default()
            .is_empty()
    );
}

#[tokio::test]
async fn proposals_are_stored_inert_with_side_view_provenance_and_nothing_else_changes() {
    let state = fleet_state().await;
    let _ = ingest(
        State(state.clone()),
        Json(IngestRequest {
            space_type: "fleet".into(),
            chunks: chunks("Nimbus hosts the DataCloud platform for Acme."),
        }),
    )
    .await
    .unwrap();
    let state = state.with_completion(ScriptedCompletion(json!({
        "id": "sv-hosts",
        "type": "relation_hint",
        "confidence": 0.8,
        "rationale": "phrasing the ontology missed",
        "source": "Nimbus",
        "relation": "HOSTS",
        "target": "DataCloud",
        "triggers": ["nimbus hosts the datacloud platform"],
        "evidence": ["Nimbus hosts the DataCloud platform for Acme."]
    })));
    let before = snapshot(&state, &GRAPH).await;
    let Json(response) = propose(State(state.clone()), None, Json(request(select(json!({})))))
        .await
        .unwrap();
    assert_eq!(response["source"], "side_views");
    assert_eq!(response["gaps"], 1);
    assert_eq!(response["stored"], 1);
    assert_eq!(response["side_views"]["candidates"][0]["support"], 2);

    let stored = state
        .backend
        .get_document(NEURONS, "sv-hosts")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored["status"], "proposed");
    assert_eq!(stored["proposed_by"], "anonymous");
    assert_eq!(stored["proposed_from"], "side_views");
    assert_eq!(
        stored["side_view_source"],
        json!({"collection": "notes", "side_views": ["sv1", "sv2"], "support": 2})
    );
    // The path wrote one proposed neuron and nothing else.
    assert_eq!(snapshot(&state, &GRAPH).await, before);
    let neurons = state
        .managed_backend
        .list_documents(NEURONS, None, None)
        .await
        .unwrap();
    assert_eq!(neurons.len(), 1);
    assert!(is_generated_collection(SIDE_VIEWS_COLLECTION));
    assert!(is_generated_collection("fact_semantics"));
}

#[tokio::test]
async fn explicit_gap_proposals_keep_their_shape_and_carry_no_side_view_provenance() {
    let state = seeded_state()
        .await
        .with_completion(ScriptedCompletion(json!({
            "id": "p1",
            "type": "relation_hint",
            "confidence": 0.8,
            "rationale": "phrasing the ontology missed",
            "source": "Nimbus",
            "relation": "HOSTS",
            "target": "DataCloud",
            "triggers": ["nimbus hosts the datacloud platform"],
            "evidence": ["Nimbus hosts the DataCloud platform for Acme."]
        })));
    let _ = ingest(
        State(state.clone()),
        Json(IngestRequest {
            space_type: "acme".into(),
            chunks: chunks("Nimbus hosts the DataCloud platform for Acme."),
        }),
    )
    .await
    .unwrap();
    let Json(response) = propose(
        State(state.clone()),
        None,
        Json(ProposeRequest {
            space_type: "acme".into(),
            gaps: Some(vec!["Nimbus --HOSTS--> DataCloud".into()]),
            eval: None,
            side_views: None,
        }),
    )
    .await
    .unwrap();
    assert_eq!(response["source"], "gaps");
    assert!(response.get("side_views").is_none());
    let stored = state
        .backend
        .get_document(NEURONS, "p1")
        .await
        .unwrap()
        .unwrap();
    assert!(stored.get("proposed_from").is_none());
    assert!(stored.get("side_view_source").is_none());
}

#[tokio::test]
async fn documents_narrow_the_selection_and_support_and_candidate_bounds_apply() {
    let state = fleet_state().await;
    let run = |extra: Value| {
        let state = state.clone();
        async move {
            let mut extra = extra;
            extra["dry_run"] = json!(true);
            let Json(response) = propose(State(state), None, Json(request(select(extra))))
                .await
                .unwrap();
            response["side_views"].clone()
        }
    };
    let only_n1 = run(json!({"documents": ["n1"]})).await;
    assert_eq!(only_n1["scanned"], 1);
    assert_eq!(only_n1["candidates"][0]["side_views"], json!(["sv1"]));

    let strict = run(json!({"min_support": 3})).await;
    assert_eq!(strict["candidates"], json!([]));
    assert!(
        strict["skipped"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["reason"] == "below_min_support")
    );

    let capped = run(json!({"max_candidates": 1})).await;
    assert_eq!(capped["candidates"].as_array().unwrap().len(), 1);

    let missing = run(json!({"documents": ["absent"]})).await;
    assert_eq!(
        (missing["scanned"].clone(), missing["candidates"].clone()),
        (json!(0), json!([]))
    );
}

#[tokio::test]
async fn candidates_over_the_cap_are_reported_not_proposed() {
    let state = fleet_state().await;
    // Disconnect DataCloud and Oslo so two candidates exist.
    let facts = state
        .managed_backend
        .list_documents("facts", None, None)
        .await
        .unwrap();
    for fact in facts {
        state
            .managed_backend
            .delete_document("facts", fact["_key"].as_str().unwrap())
            .await
            .unwrap();
    }
    let Json(response) = propose(
        State(state),
        None,
        Json(request(select(
            json!({"dry_run": true, "max_candidates": 1}),
        ))),
    )
    .await
    .unwrap();
    let report = &response["side_views"];
    assert_eq!(
        report["candidates"][0]["fact"],
        "Nimbus --HOSTS--> DataCloud"
    );
    let over: Vec<&Value> = report["skipped"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["reason"] == "over_max_candidates")
        .collect();
    assert_eq!(over.len(), 1);
    assert_eq!(over[0]["detail"], "DataCloud --LOCATED_IN--> Oslo");
}

#[tokio::test]
async fn a_space_without_any_side_views_yields_an_empty_report() {
    let state = seeded_state().await;
    let Json(response) = propose(
        State(state),
        None,
        Json(ProposeRequest {
            space_type: "acme".into(),
            gaps: None,
            eval: None,
            side_views: Some(select(json!({"dry_run": true}))),
        }),
    )
    .await
    .unwrap();
    assert_eq!(response["side_views"]["scanned"], 0);
    assert_eq!(response["side_views"]["candidates"], json!([]));
}

#[tokio::test]
async fn invalid_side_view_selections_are_refused_before_any_work() {
    let state = fleet_state().await;
    let refused = |req: ProposeRequest| {
        let state = state.clone();
        async move {
            match propose(State(state), None, Json(req)).await {
                Err(AppError(CogniGraphError::ValidationError(message))) => message,
                Err(other) => panic!("unexpected error {:?}", other.0),
                Ok(_) => panic!("accepted"),
            }
        }
    };
    let mut with_gaps = request(select(json!({"dry_run": true})));
    with_gaps.gaps = Some(vec!["Nimbus --HOSTS--> DataCloud".into()]);
    assert!(refused(with_gaps).await.contains("side_views"));
    let mut with_eval = request(select(json!({"dry_run": true})));
    with_eval.eval =
        Some(serde_json::from_value(json!({"space_id": "fleet", "questions": []})).unwrap());
    assert!(refused(with_eval).await.contains("side_views"));
    for (field, value) in [
        ("collection", json!("")),
        ("collection", json!("notes/n1")),
        ("min_support", json!(0)),
        ("max_candidates", json!(0)),
        ("max_candidates", json!(101)),
        ("documents", json!([])),
        ("documents", json!([""])),
        ("documents", json!(["a/b"])),
    ] {
        let mut body = json!({"dry_run": true});
        body[field] = value.clone();
        let message = refused(request(select(body))).await;
        assert!(message.contains(field), "{field}={value}: {message}");
    }
    let too_many: Vec<String> = (0..1001).map(|i| format!("d{i}")).collect();
    let message = refused(request(select(
        json!({"documents": too_many, "dry_run": true}),
    )))
    .await;
    assert!(message.contains("documents"), "{message}");
    // A misspelt option is an error, not a silently ignored setting.
    let typo: Result<SideViewSelection, _> =
        serde_json::from_value(json!({"collection": "notes", "min_suport": 2}));
    assert!(typo.is_err());
}

#[tokio::test]
async fn proposing_from_side_views_still_needs_a_provider() {
    let state = fleet_state().await;
    let result = propose(State(state), None, Json(request(select(json!({}))))).await;
    assert!(matches!(
        result,
        Err(AppError(CogniGraphError::BackendError(message))) if message.contains("completion provider")
    ));
}
