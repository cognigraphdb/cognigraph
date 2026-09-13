//! Payload.

use super::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(super) enum JobPayload {
    Ingest {
        space_type: SpaceType,
        accepted_neurons: Vec<Neuron>,
        chunks: Vec<Chunk>,
        batch_size: usize,
    },
    Evaluate {
        eval: EvalSpec,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        promotion_context: Option<Box<crate::promotions::PromotionContext>>,
    },
    /// Generate + embed + store retrieval side-views for each source document.
    /// The source document `keys` are frozen at submission (as chunks are for
    /// Ingest) so the job is a durable, resumable, per-document unit of work.
    SideviewsGenerate {
        collection: String,
        text_field: String,
        count: usize,
        keys: Vec<String>,
        batch_size: usize,
        regenerate: bool,
    },
    /// Per-document ontology drafting. The `documents` (chunks already grouped
    /// by title) are frozen at submission exactly as Ingest freezes its chunks,
    /// so the job is a durable, resumable per-document unit of work: each pass
    /// drafts a batch of documents and merges them into the stored draft.
    Draft {
        space_type: String,
        documents: Vec<Vec<Chunk>>,
        sample_cap: usize,
        batch_size: usize,
    },
}
