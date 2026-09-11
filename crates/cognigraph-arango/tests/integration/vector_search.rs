use cognigraph_arango::VectorSearchMode;
use cognigraph_core::{CollectionType, GraphBackend, VectorSearchOpts};
use serde_json::Value;

async fn mixed_model_contract(mode: VectorSearchMode, collection: &str) {
    let Some(backend) = super::backend() else {
        eprintln!("Skipping: ARANGO_PASSWORD not set");
        return;
    };
    let backend = backend.with_vector_mode(mode);
    let fixture: Value =
        serde_json::from_str(include_str!("../fixtures/vector-model-filter.json")).unwrap();
    backend.drop_collection(collection).await.ok();
    backend
        .ensure_collection(collection, CollectionType::Document)
        .await
        .unwrap();
    for row in fixture["rows"].as_array().unwrap() {
        backend
            .create_document(collection, row.clone())
            .await
            .unwrap();
    }
    // One list searches the entire small fixture, isolating filtering/dedup
    // correctness from approximate recall differences between Voronoi cells.
    backend
        .create_vector_index(collection, "embedding", 2, None)
        .await
        .unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let opts: VectorSearchOpts = serde_json::from_value(case.clone()).unwrap();
        let hits = backend
            .vector_search(collection, &[1.0, 0.0], &opts)
            .await
            .unwrap();
        let keys: Vec<_> = hits
            .iter()
            .map(|hit| hit.document["_key"].clone())
            .collect();
        assert_eq!(keys, *case["expected_keys"].as_array().unwrap(), "{name}");
        for (hit, score) in hits.iter().zip(case["expected_scores"].as_array().unwrap()) {
            assert!(
                (hit.score - score.as_f64().unwrap()).abs() < 1e-6,
                "{name}: {}",
                hit.score
            );
        }
    }
    backend.drop_collection(collection).await.unwrap();
}

#[tokio::test]
async fn indexed_mixed_model_contract() {
    mixed_model_contract(VectorSearchMode::Native, "cg20_indexed_contract").await;
}

#[tokio::test]
async fn fallback_mixed_model_contract() {
    mixed_model_contract(VectorSearchMode::Fallback, "cg20_fallback_contract").await;
}
