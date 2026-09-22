//! Opt-in local MiniLM qualification; no downloads or hosted provider calls.
#![cfg(feature = "onnx")]

use cognigraph_embeddings::{EmbeddingProvider, onnx::OnnxProvider};

#[tokio::test]
#[ignore = "requires an explicitly supplied local all-MiniLM-L6-v2 ONNX model"]
async fn local_minilm_inference_is_finite_repeatable_and_batch_invariant() {
    let model = std::env::var("COGNIGRAPH_ONNX_TEST_MODEL_DIR")
        .expect("set COGNIGRAPH_ONNX_TEST_MODEL_DIR to the local model/tokenizer directory");
    let provider = OnnxProvider::new(&model).unwrap();
    let texts = [
        "Evidence",
        "A café keeps a provenance record for every document.",
    ];
    let batch = provider.embed(&texts).await.unwrap();
    assert_eq!(batch.len(), texts.len());
    assert!(provider.embed(&[]).await.unwrap().is_empty());
    for (text, expected) in texts.iter().zip(&batch) {
        assert_eq!(expected.len(), 384);
        assert!(expected.iter().all(|v| v.is_finite()));
        assert!(expected.iter().map(|v| v * v).sum::<f64>() > 0.01);
        for _ in 0..2 {
            let single = provider.embed(&[text]).await.unwrap();
            assert_eq!(single.len(), 1);
            assert_eq!(single[0].len(), expected.len());
            for (actual, expected) in single[0].iter().zip(expected) {
                assert!(
                    (actual - expected).abs() < 1e-4,
                    "batch padding changed the embedding"
                );
            }
        }
    }
    assert!(
        batch[0]
            .iter()
            .zip(&batch[1])
            .any(|(a, b)| (a - b).abs() > 0.01)
    );
}
