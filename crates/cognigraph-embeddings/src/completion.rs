//! Chat-completion providers for structured generation (neuron proposals).
//! Same provider pattern as embeddings: one trait, swappable backends.

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

mod config;
#[cfg(test)]
mod request_tests;
pub use config::{
    CompletionConfig, completion_from_env, provider_named, sideviews_provider_from_env,
};

const OPENAI_COMPLETION_MODEL_ENV: &str = "COGNIGRAPH_COMPLETION_MODEL";
const DEFAULT_OPENAI_COMPLETION_MODEL: &str = "gpt-5.6-luna";
const DEFAULT_GEMINI_COMPLETION_MODEL: &str = "gemini-3.8-flash";

#[async_trait::async_trait]
pub trait CompletionProvider: Send + Sync {
    /// Complete with a JSON-schema-constrained response; returns parsed JSON.
    async fn complete_json(&self, system: &str, user: &str, schema: &Value) -> Result<Value>;
    fn model_name(&self) -> &str;
}

/// OpenAI chat completions with `response_format: json_schema`.
pub struct OpenAiCompletion {
    client: reqwest::Client,
    api_key: String,
    base_url: String,
    model: String,
}

impl OpenAiCompletion {
    pub fn new(api_key: String, base_url: Option<String>, model: Option<String>) -> Result<Self> {
        Ok(Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .build()
                .context("HTTP client")?,
            api_key,
            base_url: base_url.unwrap_or_else(|| "https://api.openai.com/v1".into()),
            model: openai_completion_model(model),
        })
    }
}

/// Whether a schema qualifies for OpenAI structured-output *strict* mode: every
/// object must reject additional properties (`additionalProperties: false`) and
/// list every property as required. Strict mode is selected per schema, not
/// globally, because some older proposal schemas deliberately remain open while
/// their symbolic validators evolve — those must not fail at the provider
/// boundary. Exposed so schema authors can assert their response contract gets
/// server-side enforcement instead of best-effort prose JSON.
pub fn supports_openai_strict_mode(schema: &Value) -> bool {
    fn visit(schema: &Value) -> bool {
        let Some(object) = schema.as_object() else {
            return true;
        };
        if object.get("type").and_then(Value::as_str) == Some("object") {
            if object.get("additionalProperties").and_then(Value::as_bool) != Some(false) {
                return false;
            }
            let Some(properties) = object.get("properties").and_then(Value::as_object) else {
                return false;
            };
            let Some(required) = object.get("required").and_then(Value::as_array) else {
                return false;
            };
            if properties.len() != required.len()
                || !properties
                    .keys()
                    .all(|name| required.iter().any(|value| value.as_str() == Some(name)))
            {
                return false;
            }
            if !properties.values().all(visit) {
                return false;
            }
        }
        if let Some(items) = object.get("items")
            && !visit(items)
        {
            return false;
        }
        for keyword in ["allOf", "anyOf", "oneOf"] {
            if let Some(branches) = object.get(keyword).and_then(Value::as_array)
                && !branches.iter().all(visit)
            {
                return false;
            }
        }
        true
    }

    visit(schema)
}

#[async_trait::async_trait]
impl CompletionProvider for OpenAiCompletion {
    fn model_name(&self) -> &str {
        &self.model
    }

    async fn complete_json(&self, system: &str, user: &str, schema: &Value) -> Result<Value> {
        let strict = supports_openai_strict_mode(schema);
        let mut body = json!({
            "model": self.model,
            "messages": [
                { "role": "system", "content": system },
                { "role": "user", "content": user }
            ],
            "response_format": {
                "type": "json_schema",
                "json_schema": { "name": "response", "schema": schema, "strict": strict }
            }
        });
        // The evaluated economical baseline is Luna with low reasoning.
        // Apply it wherever Luna is selected; other model overrides retain
        // their provider defaults and compatibility with custom endpoints.
        if self.model == DEFAULT_OPENAI_COMPLETION_MODEL {
            body["reasoning_effort"] = json!("low");
        }
        let response = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await
            .context("OpenAI completion request")?;
        let status = response.status();
        let text = response.text().await.context("OpenAI response body")?;
        if !status.is_success() {
            bail!("OpenAI completion error ({status}): {text}");
        }
        let parsed: Value = serde_json::from_str(&text).context("OpenAI response JSON")?;
        let content = parsed["choices"][0]["message"]["content"]
            .as_str()
            .context("missing completion content")?;
        serde_json::from_str(content).context("completion content is not valid JSON")
    }
}

fn openai_completion_model(explicit: Option<String>) -> String {
    select_openai_completion_model(
        explicit.as_deref(),
        std::env::var(OPENAI_COMPLETION_MODEL_ENV).ok().as_deref(),
    )
}

fn select_openai_completion_model(explicit: Option<&str>, env_model: Option<&str>) -> String {
    [explicit, env_model]
        .into_iter()
        .flatten()
        .map(str::trim)
        .find(|model| !model.is_empty())
        .unwrap_or(DEFAULT_OPENAI_COMPLETION_MODEL)
        .to_string()
}

/// Gemini generateContent with a JSON response mime type and native JSON Schema.
pub struct GeminiCompletion {
    client: reqwest::Client,
    api_key: String,
    base_url: String,
    model: String,
}

impl GeminiCompletion {
    pub fn new(api_key: String, base_url: Option<String>, model: Option<String>) -> Result<Self> {
        Ok(Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .build()
                .context("HTTP client")?,
            api_key,
            base_url: base_url
                .unwrap_or_else(|| "https://generativelanguage.googleapis.com/v1beta".into()),
            model: model.unwrap_or_else(|| DEFAULT_GEMINI_COMPLETION_MODEL.into()),
        })
    }
}

#[async_trait::async_trait]
impl CompletionProvider for GeminiCompletion {
    fn model_name(&self) -> &str {
        &self.model
    }

    async fn complete_json(&self, system: &str, user: &str, schema: &Value) -> Result<Value> {
        // Native structured output (`responseJsonSchema`, full JSON
        // Schema) + a real system instruction. The previous
        // prompt-embedded schema produced ~14% malformed-JSON first
        // attempts in the 2026-07-07 cross-family judge measurement —
        // that run was testing our plumbing, not the model.
        let body = json!({
            "systemInstruction": { "parts": [{ "text": system }] },
            "contents": [{ "parts": [{ "text": user }] }],
            "generationConfig": {
                "responseMimeType": "application/json",
                "responseJsonSchema": schema
            }
        });
        let response = self
            .client
            .post(format!(
                "{}/models/{}:generateContent",
                self.base_url, self.model
            ))
            .header("x-goog-api-key", &self.api_key)
            .json(&body)
            .send()
            .await
            .context("Gemini completion request")?;
        let status = response.status();
        let text = response.text().await.context("Gemini response body")?;
        if !status.is_success() {
            bail!("Gemini completion error ({status}): {text}");
        }
        let parsed: Value = serde_json::from_str(&text).context("Gemini response JSON")?;
        let content = parsed["candidates"][0]["content"]["parts"][0]["text"]
            .as_str()
            .context("missing completion content")?;
        serde_json::from_str(content).context("completion content is not valid JSON")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openai_completion_model_prefers_explicit_value() {
        assert_eq!(
            select_openai_completion_model(Some("gpt-explicit"), Some("gpt-env")),
            "gpt-explicit"
        );
    }

    #[test]
    fn openai_completion_model_uses_env_when_explicit_missing() {
        assert_eq!(
            select_openai_completion_model(None, Some(" gpt-env ")),
            "gpt-env"
        );
    }

    #[test]
    fn openai_completion_model_falls_back_when_empty() {
        assert_eq!(
            select_openai_completion_model(Some(" "), Some("")),
            DEFAULT_OPENAI_COMPLETION_MODEL
        );
    }

    #[test]
    fn strict_mode_is_selected_only_for_closed_fully_required_schemas() {
        assert!(supports_openai_strict_mode(&json!({
            "type": "object",
            "properties": {
                "verdict": { "type": "string" },
                "evidence": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": { "id": { "type": "string" } },
                        "required": ["id"],
                        "additionalProperties": false
                    }
                }
            },
            "required": ["verdict", "evidence"],
            "additionalProperties": false
        })));
        assert!(!supports_openai_strict_mode(&json!({
            "type": "object",
            "properties": { "optional": { "type": "string" } },
            "required": [],
            "additionalProperties": false
        })));
        assert!(!supports_openai_strict_mode(&json!({
            "type": "object",
            "properties": { "value": { "type": "string" } },
            "required": ["value"],
            "additionalProperties": true
        })));
    }
}
