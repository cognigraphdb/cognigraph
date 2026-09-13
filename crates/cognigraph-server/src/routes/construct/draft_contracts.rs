//! Draft contracts.

use super::*;

#[derive(Deserialize)]
pub(super) struct DraftRequest {
    /// Id for the NEW space — must not exist in `space_types` (D3).
    pub(super) space_type: String,
    /// The corpus to draft from, inline (nothing is ingested).
    pub(super) chunks: Vec<Chunk>,
    /// Chunks shown to the model (self-checks and the advisor always
    /// run on the full corpus). Default 40.
    pub(super) sample_cap: Option<usize>,
    /// Draft each source document separately and merge the catalogues/rules,
    /// instead of one broad draft over the interleaved corpus. Chunks are grouped
    /// into documents by their `title`. A single broad draft samples ~evenly and
    /// starves entities prominent in only one document (a drug's indicated
    /// condition), dropping the drug→condition rules closure then rejects; the
    /// 2026-07-20 A/B recovered 15/15 condition entities per-document vs 0/15
    /// broad (decision_neurons_real_label_construction.md). Default false.
    #[serde(default)]
    pub(super) per_document: bool,
    /// Draft in the background instead of inside this request. Requires an
    /// `Idempotency-Key` header; enqueues one durable, tenant-scoped
    /// `construct.draft` job and returns the job submission envelope (202, or
    /// 200 on idempotent replay) rather than a draft.
    ///
    /// Use it for anything larger than a handful of documents: drafting costs
    /// TWO LLM completions per document, so a synchronous request over a real
    /// corpus exceeds the request timeout (10 documents already returned HTTP
    /// 408). The job always drafts per-document — `per_document` and
    /// `documents` are implied — checkpointing after each batch into the same
    /// inert `space_type_drafts/{space_type}` artifact, which stays at status
    /// `drafting` (unacceptable) until the final pass writes `draft`.
    #[serde(default, rename = "async")]
    pub(super) run_async: bool,
}
/// Group chunks into documents by their `title` (first-seen order) for
/// per-document drafting. Chunks with the same title form one document; untitled
/// chunks collapse into a single group, so per-document drafting on an untitled
/// corpus degrades to one broad draft (the `documents` count in the response
/// makes that visible).
///
/// Shared with the durable `construct.draft` job (`jobs::prepare_payload`) so
/// synchronous and background drafting group an identical corpus identically.
pub(crate) fn group_chunks_by_title(chunks: &[Chunk]) -> Vec<Vec<Chunk>> {
    let mut order: Vec<String> = Vec::new();
    let mut groups: std::collections::HashMap<String, Vec<Chunk>> =
        std::collections::HashMap::new();
    for chunk in chunks {
        let key = chunk.title.trim().to_string();
        if !groups.contains_key(&key) {
            order.push(key.clone());
        }
        groups.entry(key).or_default().push(chunk.clone());
    }
    order
        .into_iter()
        .filter_map(|key| groups.remove(&key))
        .collect()
}
