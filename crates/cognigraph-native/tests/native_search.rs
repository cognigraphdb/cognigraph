use cognigraph_core::{CollectionType, GraphBackend, VectorSearchOpts};
use cognigraph_native::NativeBackend;
use serde_json::json;

#[tokio::test]
async fn vector_search_uses_exact_cosine() {
    let backend = seeded_backend().await;
    let hits = backend
        .vector_search(
            "documents",
            &[1.0, 0.0],
            &VectorSearchOpts {
                threshold: Some(0.0),
                limit: 2,
                model_name: None,
            },
        )
        .await
        .unwrap();

    assert_eq!(hits[0].document["_id"], json!("documents/a"));
    assert!(hits[0].score > hits[1].score);
}

async fn seeded_backend() -> NativeBackend {
    let backend = NativeBackend::new();
    backend
        .ensure_collection("documents", CollectionType::Document)
        .await
        .unwrap();
    backend
        .ensure_collection("relationships", CollectionType::Edge)
        .await
        .unwrap();

    for doc in [
        json!({
            "_key": "a",
            "title": "Alpha",
            "category": "research",
            "embedding": [1.0, 0.0]
        }),
        json!({
            "_key": "b",
            "title": "Beta",
            "category": "research",
            "embedding": [0.8, 0.2]
        }),
        json!({
            "_key": "c",
            "title": "Gamma",
            "category": "notes",
            "embedding": [0.0, 1.0]
        }),
    ] {
        backend.create_document("documents", doc).await.unwrap();
    }

    backend
        .create_edge(
            "relationships",
            json!({
                "_from": "documents/a",
                "_to": "documents/b",
                "relation_type": "links",
                "confidence": 0.9
            }),
        )
        .await
        .unwrap();
    backend
        .create_edge(
            "relationships",
            json!({
                "_from": "documents/b",
                "_to": "documents/c",
                "relation_type": "links",
                "confidence": 0.8
            }),
        )
        .await
        .unwrap();
    backend
        .create_edge(
            "relationships",
            json!({
                "_from": "documents/a",
                "_to": "documents/c",
                "relation_type": "weak",
                "confidence": 0.2
            }),
        )
        .await
        .unwrap();

    backend
}

#[tokio::test]
async fn text_search_ranks_by_bm25() {
    let backend = NativeBackend::new();
    backend
        .ensure_collection("articles", CollectionType::Document)
        .await
        .unwrap();
    for (key, title, content) in [
        (
            "a",
            "Rust ownership",
            "Ownership is the Rust memory model. Rust Rust Rust.",
        ),
        ("b", "Cooking pasta", "Boil water, add pasta, stir."),
        (
            "c",
            "Rust and cooking",
            "A rusty pan is bad for cooking pasta.",
        ),
    ] {
        backend
            .create_document(
                "articles",
                json!({ "_key": key, "title": title, "content": content }),
            )
            .await
            .unwrap();
    }

    let fields = vec!["title".to_string(), "content".to_string()];
    let hits = backend
        .text_search("articles", "rust", &fields, 10)
        .await
        .unwrap();
    assert_eq!(
        hits.len(),
        2,
        "case-insensitive; 'rusty' is a different token"
    );
    assert_eq!(hits[0].document["_key"], json!("a"));
    assert!(hits[0].score > hits[1].score);

    // Multi-term queries accumulate; only matching fields count.
    let title_only = vec!["title".to_string()];
    let hits = backend
        .text_search("articles", "cooking pasta", &title_only, 10)
        .await
        .unwrap();
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].document["_key"], json!("b"));

    // Empty query yields nothing; unknown collection errors.
    assert!(
        backend
            .text_search("articles", "  ", &fields, 10)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        backend
            .text_search("missing", "rust", &fields, 10)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn text_search_handles_multilingual_content() {
    let backend = NativeBackend::new();
    backend
        .ensure_collection("articles", CollectionType::Document)
        .await
        .unwrap();
    for (key, title, content) in [
        ("de", "Fußgängerzone", "Die Fußgänger überqueren die Straße"),
        (
            "ru",
            "Лиса",
            "Быстрая рыжая лиса прыгает через ленивую собаку",
        ),
        ("he", "ירושלים", "ירושלים של זהב ושל נחושת"),
        ("mixed", "Mixed", "Rust programming на русском ועם עברית"),
    ] {
        backend
            .create_document(
                "articles",
                json!({ "_key": key, "title": title, "content": content }),
            )
            .await
            .unwrap();
    }
    let fields = vec!["title".to_string(), "content".to_string()];

    // German umlauts and eszett survive tokenization; "Fußgängerzone" is a
    // different token than "Fußgänger".
    let hits = backend
        .text_search("articles", "Fußgänger", &fields, 10)
        .await
        .unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].document["_key"], json!("de"));
    let hits = backend
        .text_search("articles", "straße", &fields, 10)
        .await
        .unwrap();
    assert_eq!(hits[0].document["_key"], json!("de"));

    // Cyrillic with real Unicode case folding: uppercase query matches.
    let hits = backend
        .text_search("articles", "ЛИСА", &fields, 10)
        .await
        .unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].document["_key"], json!("ru"));

    // Hebrew (RTL, no case).
    let hits = backend
        .text_search("articles", "ירושלים", &fields, 10)
        .await
        .unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].document["_key"], json!("he"));

    // A mixed-script query accumulates scores across scripts in one doc.
    let hits = backend
        .text_search("articles", "rust русском עברית", &fields, 10)
        .await
        .unwrap();
    assert_eq!(hits[0].document["_key"], json!("mixed"));
}

/// The quantized two-stage search must return the same documents, in the
/// same order, with the same exact scores as a brute-force f64 scan.
#[tokio::test]
async fn quantized_vector_search_matches_exact_brute_force() {
    let backend = NativeBackend::new();
    backend
        .ensure_collection("vecs", CollectionType::Document)
        .await
        .unwrap();

    // Deterministic pseudo-random vectors.
    let mut seed = 99u64;
    let mut next = move || {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((seed >> 11) as f64 / (1u64 << 53) as f64) * 2.0 - 1.0
    };
    let mut vectors = Vec::new();
    for i in 0..300 {
        let v: Vec<f64> = (0..64).map(|_| next()).collect();
        backend
            .create_document(
                "vecs",
                json!({ "_key": format!("v{i:03}"), "embedding": v }),
            )
            .await
            .unwrap();
        vectors.push(v);
    }
    let query: Vec<f64> = (0..64).map(|_| next()).collect();

    // Brute-force exact reference.
    fn cosine(a: &[f64], b: &[f64]) -> f64 {
        let dot: f64 = a.iter().zip(b).map(|(x, y)| x * y).sum();
        let na: f64 = a.iter().map(|v| v * v).sum::<f64>().sqrt();
        let nb: f64 = b.iter().map(|v| v * v).sum::<f64>().sqrt();
        dot / (na * nb)
    }
    let mut expected: Vec<(usize, f64)> = vectors
        .iter()
        .enumerate()
        .map(|(i, v)| (i, cosine(&query, v)))
        .collect();
    expected.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    let hits = backend
        .vector_search(
            "vecs",
            &query,
            &VectorSearchOpts {
                threshold: None,
                limit: 10,
                model_name: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(hits.len(), 10);
    for (hit, (index, score)) in hits.iter().zip(expected.iter().take(10)) {
        assert_eq!(hit.document["_key"], json!(format!("v{index:03}")));
        assert!((hit.score - score).abs() < 1e-12, "scores must be exact");
    }
}

/// Writes invalidate the quantized index: a document created after a
/// search is immediately findable.
#[tokio::test]
async fn quantized_index_invalidates_on_write() {
    let backend = seeded_backend().await;
    let opts = VectorSearchOpts {
        threshold: None,
        limit: 1,
        model_name: None,
    };
    // Prime the index. Query [0.6, 0.8]: best existing angle is c [0.8, 0.6].
    let query = [0.6, 0.8];
    let hits = backend
        .vector_search("documents", &query, &opts)
        .await
        .unwrap();
    assert_eq!(hits[0].document["_key"], json!("c"));
    // A perfectly aligned document arrives after the index was built.
    backend
        .create_document(
            "documents",
            json!({ "_key": "fresh", "embedding": [0.6, 0.8] }),
        )
        .await
        .unwrap();
    let hits = backend
        .vector_search("documents", &query, &opts)
        .await
        .unwrap();
    assert_eq!(hits[0].document["_key"], json!("fresh"));
}
