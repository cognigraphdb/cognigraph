//! CG-89 route-level tests: the inert rank-hint warning on graph-augmented
//! search. Shared fixtures live in `graph_augmented_tests.rs`.

#[cfg(feature = "enterprise")]
use super::tests::request;
use super::tests::seeded_state;
use super::*;
#[cfg(feature = "enterprise")]
use cognigraph_cache::{CacheConfig, InMemoryCache, QueryCache};
#[cfg(feature = "enterprise")]
use cognigraph_core::SearchHit;
use serde_json::json;

// ---------------------------------------------------------------------------

/// Insert a neuron through the internal handle; the public backend refuses
/// writes to the managed `neurons` collection.
async fn insert_neuron(state: &AppState, key: &str, doc: serde_json::Value) {
    let mut doc = doc;
    doc["_key"] = json!(key);
    doc["id"] = json!(key);
    state
        .managed_backend
        .create_document("neurons", doc)
        .await
        .unwrap();
}

fn accepted_rank_hint(relation: &str) -> serde_json::Value {
    json!({
        "type": "relation_rank_hint",
        "status": "accepted",
        "confidence": 0.9,
        "evidence": ["trace review"],
        "relation": relation,
        "boost": 25.0,
    })
}

async fn search(state: &AppState, overrides: serde_json::Value) -> serde_json::Value {
    let mut wire =
        json!({"query": "who supplies what", "collection": "embeddings", "threshold": 0.0});
    wire.as_object_mut()
        .unwrap()
        .extend(overrides.as_object().unwrap().clone());
    graph_augmented_search(
        State(state.clone()),
        Json(serde_json::from_value(wire).unwrap()),
    )
    .await
    .unwrap()
    .0
}

#[cfg(feature = "enterprise")]
fn inert_warning(response: &serde_json::Value) -> Option<&serde_json::Value> {
    response["warnings"]
        .as_array()?
        .iter()
        .find(|w| w["code"] == "inert_rank_hints")
}

#[cfg(feature = "enterprise")]
#[tokio::test]
async fn accepted_rank_hint_outside_facts_warns_on_fresh_responses() {
    let state = seeded_state().await;
    insert_neuron(&state, "boost-supplies", accepted_rank_hint("SUPPLIES")).await;

    let response = search(&state, json!({})).await;
    let warning = inert_warning(&response).unwrap_or_else(|| panic!("{response}"));
    assert_eq!(warning["accepted_rank_hints"], 1);
    assert_eq!(warning["edge_collection"], "document_relations");
    assert!(
        warning["message"]
            .as_str()
            .unwrap()
            .contains("edge_collection")
    );
    assert!(response.get("cached").is_none(), "{response}");
}

#[cfg(not(feature = "enterprise"))]
#[tokio::test]
async fn community_never_emits_rank_hint_warnings() {
    // Community has no neurons collection semantics; a rank-hint document
    // that happens to exist is data, not configuration.
    let state = seeded_state().await;
    insert_neuron(&state, "boost-supplies", accepted_rank_hint("SUPPLIES")).await;
    let response = search(&state, json!({})).await;
    assert!(response.get("warnings").is_none(), "{response}");
}

#[cfg(feature = "enterprise")]
#[tokio::test]
async fn traversing_facts_or_inert_lifecycle_states_do_not_warn() {
    let state = seeded_state().await;
    insert_neuron(&state, "boost-supplies", accepted_rank_hint("SUPPLIES")).await;
    let facts = search(&state, json!({"edge_collection": "facts"})).await;
    assert!(facts.get("warnings").is_none(), "{facts}");

    let state = seeded_state().await;
    for status in ["proposed", "rejected", "retired"] {
        let mut doc = accepted_rank_hint("SUPPLIES");
        doc["status"] = json!(status);
        insert_neuron(&state, &format!("hint-{status}"), doc).await;
    }
    let response = search(&state, json!({})).await;
    assert!(response.get("warnings").is_none(), "{response}");
}

#[cfg(feature = "enterprise")]
#[tokio::test]
async fn assisted_cache_hit_warns_but_strong_cache_hit_omits_warnings() {
    // The fixed embedder maps every query to [1, 0]; a cached entry stored
    // under [0.8, 0.6] is a weak match, so the first call is cache-assisted.
    let cache = InMemoryCache::new(CacheConfig {
        enabled: true,
        ..Default::default()
    });
    let mut old_key = request().cache_key().unwrap();
    old_key.normalized_query = normalize_query("old supplies question");
    cache
        .put_results(
            &old_key,
            Some(vec![0.8, 0.6]),
            vec![SearchHit {
                document: json!({"document_id": "entities/c", "score": 1.0}),
                score: 1.0,
                source: Some("graph-augmented".into()),
            }],
            Some(json!([{"fact": "stale cached fact"}])),
        )
        .await;
    let state = seeded_state().await.with_cache(cache);
    insert_neuron(&state, "boost-supplies", accepted_rank_hint("SUPPLIES")).await;

    let assisted = search(&state, json!({})).await;
    assert_eq!(assisted["cached"], "assisted", "{assisted}");
    assert!(inert_warning(&assisted).is_some(), "{assisted}");

    // Identical query again: strong hit served from cache without a neurons
    // read, so no warnings field even though the hint is still accepted.
    let strong = search(&state, json!({})).await;
    assert_eq!(strong["cached"], true, "{strong}");
    assert!(strong.get("warnings").is_none(), "{strong}");
}

#[cfg(feature = "enterprise")]
#[tokio::test]
async fn unreadable_neurons_collection_neither_warns_nor_fails() {
    let state = seeded_state().await;
    insert_neuron(&state, "boost-supplies", accepted_rank_hint("SUPPLIES")).await;
    let response = search(&state, json!({"neurons_collection": "no_such_collection"})).await;
    assert!(response.get("warnings").is_none(), "{response}");
    assert_eq!(
        response["graph_facts"][0]["relation"], "SUPPLIES",
        "{response}"
    );
}

#[cfg(feature = "enterprise")]
#[tokio::test]
async fn malformed_neuron_documents_are_excluded_from_the_count() {
    let state = seeded_state().await;
    insert_neuron(&state, "boost-supplies", accepted_rank_hint("SUPPLIES")).await;
    insert_neuron(
        &state,
        "bogus",
        json!({"type": "not_a_kind", "status": "accepted"}),
    )
    .await;
    insert_neuron(
        &state,
        "no-type",
        json!({"status": "accepted", "boost": 5.0}),
    )
    .await;
    let response = search(&state, json!({})).await;
    assert_eq!(
        inert_warning(&response).unwrap()["accepted_rank_hints"],
        1,
        "{response}"
    );
}

#[cfg(feature = "enterprise")]
#[tokio::test]
async fn warning_is_independent_of_trace_size_and_traversal_outcome() {
    let state = seeded_state().await;
    insert_neuron(&state, "boost-supplies", accepted_rank_hint("SUPPLIES")).await;
    // No facts requested and an edge collection that does not exist: the
    // traversal is skipped, the trace is empty, the hints are still inert.
    let response = search(
        &state,
        json!({"graph_facts_limit": 0, "edge_collection": "no_such_edges"}),
    )
    .await;
    assert_eq!(response["graph_facts"], json!([]), "{response}");
    let warning = inert_warning(&response).unwrap_or_else(|| panic!("{response}"));
    assert_eq!(warning["edge_collection"], "no_such_edges");
}
