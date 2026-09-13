//! Embedding providers for CogniGraph.
//!
//! Supports: OpenAI, Ollama, ONNX, Cloudflare, OAuth.

#[cfg(feature = "cloudflare")]
pub mod cloudflare;
#[cfg(feature = "ollama")]
pub mod completion;
pub mod gemini;
#[cfg(feature = "oauth")]
pub mod oauth;
pub mod ollama;
#[cfg(feature = "onnx")]
pub mod onnx;
#[cfg(feature = "openai")]
pub mod openai;

/// Trait for embedding providers.
///
/// Object-safe via `async_trait` so it can be stored as `dyn EmbeddingProvider`.
#[async_trait::async_trait]
pub trait EmbeddingProvider: Send + Sync {
    async fn embed(&self, texts: &[&str]) -> anyhow::Result<Vec<Vec<f64>>>;

    fn dimension(&self) -> Option<usize> {
        None
    }

    fn model_name(&self) -> &str {
        "default"
    }
}
