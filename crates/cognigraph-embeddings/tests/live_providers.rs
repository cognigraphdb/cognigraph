//! Live embedding-provider tests over the State of the Union corpus.
//! Explicit opt-in: `cargo test -p cognigraph-embeddings --test live_providers -- --ignored`.
//! Ordinary tests never load provider keys or make these external requests.

use cognigraph_embeddings::EmbeddingProvider;

fn sotu_paragraphs() -> Vec<String> {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/sotu.txt"
    ))
    .expect("fixtures/sotu.txt missing");
    text.split("\n\n")
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .take(4)
        .map(str::to_string)
        .collect()
}

fn cosine(a: &[f64], b: &[f64]) -> f64 {
    let dot: f64 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f64 = a.iter().map(|v| v * v).sum::<f64>().sqrt();
    let nb: f64 = b.iter().map(|v| v * v).sum::<f64>().sqrt();
    dot / (na * nb)
}

async fn exercise(provider: &dyn EmbeddingProvider) {
    let paragraphs = sotu_paragraphs();
    let refs: Vec<&str> = paragraphs.iter().map(String::as_str).collect();
    let embeddings = provider.embed(&refs).await.unwrap();
    assert_eq!(embeddings.len(), refs.len());
    let dim = embeddings[0].len();
    assert!(dim >= 128, "unexpectedly small embedding: {dim}");
    assert!(embeddings.iter().all(|e| e.len() == dim));

    // A paragraph must be more similar to itself than to a different one.
    let self_sim = cosine(&embeddings[0], &embeddings[0]);
    let cross_sim = cosine(&embeddings[0], &embeddings[3]);
    assert!(self_sim > 0.999);
    assert!(cross_sim < self_sim);
}

#[tokio::test]
#[ignore = "external OpenAI qualification; run explicitly with --ignored"]
async fn openai_embeds_sotu() {
    dotenvy::from_path(concat!(env!("CARGO_MANIFEST_DIR"), "/../../.env")).ok();
    let api_key = std::env::var("OPENAI_API_KEY")
        .expect("OPENAI_API_KEY is required for opted-in provider qualification");
    let provider = cognigraph_embeddings::openai::OpenAiProvider::new(
        api_key,
        std::env::var("OPENAI_BASE_URL").ok(),
        std::env::var("EMBEDDING_MODEL").ok(),
    )
    .unwrap();
    exercise(&provider).await;
}

#[tokio::test]
#[ignore = "external Gemini qualification; run explicitly with --ignored"]
async fn gemini_embeds_sotu() {
    dotenvy::from_path(concat!(env!("CARGO_MANIFEST_DIR"), "/../../.env")).ok();
    let api_key = std::env::var("GEMINI_API_KEY")
        .expect("GEMINI_API_KEY is required for opted-in provider qualification");
    let provider = cognigraph_embeddings::gemini::GeminiProvider::new(
        api_key, None, None, // gemini-embedding-2 default
        None,
    )
    .unwrap();
    exercise(&provider).await;
}
