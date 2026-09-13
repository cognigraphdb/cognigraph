# GraphMAE — Graph Masked Autoencoder

**Paper:** Hou et al. (2022), "GraphMAE: Self-Supervised Masked Graph Autoencoders"
**ArXiv:** [2205.10803](https://arxiv.org/abs/2205.10803)

## What it is

A self-supervised method for learning node representations on a graph. Given a graph, randomly mask a subset of node features, then train a GAT (Graph Attention Network) encoder + decoder to reconstruct the masked features. The encoder learns representations that capture both local node content and graph structure — without requiring any labeled data.

Key differentiators from earlier graph representation methods:
- **Feature reconstruction, not contrastive learning** — simpler objective, fewer hyperparameters to tune
- **Scaled cosine error (SCE) loss** — robust to outlier features
- **Re-masking trick** — decoder sees masked nodes with `[MASK]` tokens, forcing the encoder to encode enough context in its representations

## Why we're watching it

CogniGraph's current embedding model is external (OpenAI, Ollama) and stateless — it doesn't know anything about the graph structure the documents participate in. Two documents that are structurally connected in the graph may have embeddings that place them far apart in vector space.

GraphMAE offers a path to **embeddings that are shaped by the graph itself**. A document's representation would reflect not just its text content but also:
- Which other documents cite it or are cited by it
- Its position in the typed-edge topology (e.g., documents at the hub of many "contradicts" edges)
- Latent communities discovered through graph structure

This would make graph-augmented search qualitatively better: the embedding space itself would already encode relational semantics, not just textual similarity.

## When it becomes relevant

We'd consider integrating GraphMAE-style training when:

1. **We have enough graph density to train on.** A graph with 10K documents and 100 edges won't benefit — GraphMAE needs meaningful connectivity to learn from.
2. **Users report that semantically similar documents are not graph-connected, and vice versa.** This would signal that the embedding/graph mismatch is costing retrieval quality.
3. **We commit to supporting per-deployment embedding training.** GraphMAE produces embeddings tuned to a specific graph — you can't use one model across deployments. This is significant operational complexity.
4. **Phase 7 (RAG) work reveals that retrieval quality plateaus despite cache tuning.** If the cache gets us 80% of the way and we need the last 20%, this is where to look.

## What we'd need to build

- A training pipeline that runs periodically (nightly?) against the current graph snapshot
- Storage for the trained encoder weights per collection
- A new `EmbeddingProvider` implementation that uses the trained encoder instead of an external API
- A migration strategy when the encoder updates (all embeddings need regeneration)

## Reference implementation

- Original: https://github.com/THUDM/GraphMAE
- Rust reimplementation exists in RuVector (`crates/ruvector-gnn/src/graphmae.rs`) — ~440 LOC, includes GAT encoder, masking, SCE loss. Worth reading if we ever start this work. Note: pulling in RuVector itself is not on the table; the code is reference material only.

## Why not now

We're solving retrieval quality through the **semantic query cache with continuous influence**. The cache learns which documents are relevant to which query patterns, without requiring any model training. This achieves the "learns from use" property at a fraction of the operational complexity.

GraphMAE would move learning from the query layer (where we have it) to the embedding layer (where we don't). That's a much larger commitment — new training infrastructure, new deployment model, new failure modes. We should only take it on if the cache approach hits a ceiling we can measure.

## Open questions

- How often does the graph need to change before retrained embeddings become stale?
- Can we use GraphMAE incrementally (fine-tune on deltas) rather than retraining from scratch?
- Does the embedding-graph alignment actually improve retrieval quality on our workloads, or is it a research result that doesn't generalize?

These are empirical questions that matter only if we actually start the work.
