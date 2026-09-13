//! Google Gemini embeddings provider.
//!
//! Uses the Generative Language API `batchEmbedContents` endpoint with the
//! `gemini-embedding-2` model by default. Docs:
//! https://ai.google.dev/gemini-api/docs/models/gemini-embedding-2

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::EmbeddingProvider;

pub struct GeminiProvider {
    client: reqwest::Client,
    api_key: String,
    base_url: String,
    model: String,
    /// Optional embedding size (Gemini supports Matryoshka truncation).
    output_dimensionality: Option<usize>,
}

impl GeminiProvider {
    pub fn new(
        api_key: String,
        base_url: Option<String>,
        model: Option<String>,
        output_dimensionality: Option<usize>,
    ) -> Result<Self> {
        let client = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(30))
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .context("Failed to build HTTP client for Gemini provider")?;
        Ok(Self {
            client,
            api_key,
            base_url: base_url
                .unwrap_or_else(|| "https://generativelanguage.googleapis.com/v1beta".to_string()),
            model: model.unwrap_or_else(|| "gemini-embedding-2".to_string()),
            output_dimensionality,
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BatchRequest<'a> {
    requests: Vec<EmbedRequest<'a>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EmbedRequest<'a> {
    model: String,
    content: Content<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    output_dimensionality: Option<usize>,
}

#[derive(Serialize)]
struct Content<'a> {
    parts: Vec<Part<'a>>,
}

#[derive(Serialize)]
struct Part<'a> {
    text: &'a str,
}

#[derive(Deserialize)]
struct BatchResponse {
    embeddings: Vec<EmbeddingValues>,
}

#[derive(Deserialize)]
struct EmbeddingValues {
    values: Vec<f64>,
}

#[derive(Deserialize)]
struct ErrorResponse {
    error: ErrorDetail,
}

#[derive(Deserialize)]
struct ErrorDetail {
    message: String,
}

#[async_trait::async_trait]
impl EmbeddingProvider for GeminiProvider {
    fn model_name(&self) -> &str {
        &self.model
    }

    fn dimension(&self) -> Option<usize> {
        self.output_dimensionality
    }

    async fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f64>>> {
        if texts.is_empty() {
            return Ok(vec![]);
        }

        let url = format!("{}/models/{}:batchEmbedContents", self.base_url, self.model);
        let request = BatchRequest {
            requests: texts
                .iter()
                .map(|text| EmbedRequest {
                    model: format!("models/{}", self.model),
                    content: Content {
                        parts: vec![Part { text }],
                    },
                    output_dimensionality: self.output_dimensionality,
                })
                .collect(),
        };

        let response = self
            .client
            .post(&url)
            .header("x-goog-api-key", &self.api_key)
            .json(&request)
            .send()
            .await
            .context("Failed to send request to Gemini embeddings API")?;

        let status = response.status();
        let body = response
            .text()
            .await
            .context("Failed to read Gemini response body")?;

        if !status.is_success() {
            if let Ok(err) = serde_json::from_str::<ErrorResponse>(&body) {
                bail!("Gemini API error ({}): {}", status, err.error.message);
            }
            bail!("Gemini API error ({}): {}", status, body);
        }

        let parsed: BatchResponse =
            serde_json::from_str(&body).context("Failed to parse Gemini embeddings response")?;
        let embeddings: Vec<Vec<f64>> = parsed.embeddings.into_iter().map(|e| e.values).collect();

        if embeddings.len() != texts.len() {
            bail!(
                "Gemini returned {} embeddings for {} inputs",
                embeddings.len(),
                texts.len()
            );
        }
        Ok(embeddings)
    }
}
