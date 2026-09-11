//! Context-expansion "side-views": LLM-generated question/answer pairs that act
//! as alternative retrieval surfaces for a passage. At query time a user's
//! question often matches a generated side-view question far better than the raw
//! prose, so indexing side-views alongside the original text lifts retrieval
//! recall (the doc2query / hypothetical-questions / HyDE family).
//!
//! This is deliberately a RETRIEVAL aid, NOT governed fact construction: side
//! views are non-authoritative, non-deterministic, and must live in a
//! quarantined, provenance-linked layer separate from the Semantic Neurons fact
//! graph (respecting the M25 mutation barrier). This module owns only the
//! generation contract — the strict JSON-schema prompt — so both the pre-build
//! model benchmark and any future ingest job share one definition.
//!
//! Every call goes through [`CompletionProvider::complete_json`] with a strict,
//! closed schema: we never ask the model to emit JSON as prose.

use anyhow::{Context, Result};
use cognigraph_embeddings::completion::CompletionProvider;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// One generated side-view: a self-contained question a reader might ask that
/// the source passage answers, paired with a short answer grounded in that
/// passage. Both fields feed the retrieval index (the question as the primary
/// surface; the answer as a compact, on-topic companion).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QaPair {
    pub question: String,
    pub answer: String,
}

/// System instruction for side-view generation. Encodes the two properties that
/// make a side-view useful as a search surface: the question must be
/// *self-contained* (a standalone query, never "the passage above"), and the
/// answer must be *grounded strictly in the passage* (a search aid must not
/// hallucinate facts the source never stated).
pub const SIDEVIEWS_SYSTEM: &str = "\
You expand a passage into retrieval \"side-views\" for a search index. Given a \
passage, produce diverse questions a reader could ask that the passage directly \
answers, each paired with a short answer drawn only from the passage.\n\
\n\
Rules:\n\
- Each question must be SELF-CONTAINED: a standalone search query. Never refer \
to \"the passage\", \"the text\", \"this document\", \"above\", or \"mentioned\".\n\
- Each answer must be GROUNDED STRICTLY in the passage: no outside knowledge, no \
inference beyond what is stated, no speculation. If the passage does not state \
it, do not ask about it.\n\
- Vary the questions across the distinct facts in the passage and across \
phrasing (who / what / when / where / why / how, plus terse keyword-style \
queries). Avoid near-duplicate questions.\n\
- Keep answers concise (one sentence where possible).";

/// Strict JSON schema for the side-view response. Closed (`additionalProperties:
/// false`) and fully required at every level, so OpenAI selects strict mode and
/// Gemini gets a native `responseJsonSchema`. The requested count lives in the
/// prompt, not the schema (strict mode forbids array length constraints).
pub fn sideviews_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "pairs": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "question": { "type": "string" },
                        "answer": { "type": "string" }
                    },
                    "required": ["question", "answer"],
                    "additionalProperties": false
                }
            }
        },
        "required": ["pairs"],
        "additionalProperties": false
    })
}

/// The user turn: the passage plus the target pair count.
pub fn sideviews_user(text: &str, count: usize) -> String {
    format!(
        "Produce exactly {count} question/answer pairs for this passage.\n\n\
         Passage:\n\"\"\"\n{text}\n\"\"\"",
    )
}

/// Generate side-views for one passage via the given provider. Returns the
/// parsed pairs; the provider guarantees schema-valid JSON or errors, so no
/// prose parsing happens here.
pub async fn generate_sideviews(
    provider: &dyn CompletionProvider,
    text: &str,
    count: usize,
) -> Result<Vec<QaPair>> {
    let value = provider
        .complete_json(
            SIDEVIEWS_SYSTEM,
            &sideviews_user(text, count),
            &sideviews_schema(),
        )
        .await
        .context("side-view generation request")?;
    let pairs = value
        .get("pairs")
        .and_then(Value::as_array)
        .context("side-view response missing `pairs` array")?;
    pairs
        .iter()
        .map(|pair| serde_json::from_value(pair.clone()).context("side-view pair shape"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cognigraph_embeddings::completion::supports_openai_strict_mode;

    #[test]
    fn schema_qualifies_for_openai_strict_mode() {
        // A closed, fully-required schema is what lets both families enforce the
        // shape server-side instead of us parsing prose JSON.
        assert!(supports_openai_strict_mode(&sideviews_schema()));
    }

    #[test]
    fn user_turn_embeds_count_and_text() {
        let prompt = sideviews_user("Ada Lovelace wrote the first algorithm.", 7);
        assert!(prompt.contains("exactly 7"));
        assert!(prompt.contains("Ada Lovelace"));
    }

    #[test]
    fn qa_pair_round_trips_through_schema_shape() {
        let value = json!({ "question": "Who wrote it?", "answer": "Ada Lovelace." });
        let pair: QaPair = serde_json::from_value(value).unwrap();
        assert_eq!(pair.question, "Who wrote it?");
        assert_eq!(pair.answer, "Ada Lovelace.");
    }
}
