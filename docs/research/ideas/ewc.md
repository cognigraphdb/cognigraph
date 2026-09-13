# Elastic Weight Consolidation (EWC)

**Paper:** Kirkpatrick et al. (2017), "Overcoming catastrophic forgetting in neural networks"
**Published:** PNAS, March 2017
**ArXiv:** [1612.00796](https://arxiv.org/abs/1612.00796)

## What it is

A technique for training neural networks on a sequence of tasks without forgetting earlier ones. Standard gradient descent causes "catastrophic forgetting" — when you train on task B, the weights that mattered for task A get overwritten. EWC fixes this by computing a **Fisher information matrix** that identifies which weights are important for previously learned tasks, then penalizing changes to those weights during subsequent training.

The core insight: not all weights are equally important. A small number of weights carry most of the task-specific knowledge, and those are the ones you need to protect.

In code, EWC adds a term to the loss function:

```
L_total = L_new_task + Σᵢ (λ/2) * Fᵢ * (θᵢ - θ*ᵢ)²
```

Where `Fᵢ` is the Fisher information for weight `i`, `θ*ᵢ` is its value after training on the previous task, and `λ` controls how strongly to protect old knowledge.

## Why we're watching it

CogniGraph's semantic cache is a form of **learned prior** — it accumulates knowledge about which documents are relevant to which query patterns. Right now, this prior is represented as cached `(query_embedding → results)` pairs with similarity-based lookup. It's a simple, explicit form of memory.

If we ever move to a richer representation — say, a small neural ranking model that learns from cache hits and user feedback — we'll face the same problem EWC solves: **how do you update the model on new data without forgetting what it learned about old queries?**

Without a forgetting mitigation strategy, the ranker would drift toward whatever queries arrived most recently. A burst of queries about one topic would degrade performance on every other topic. EWC prevents this by protecting the weights that encode past query patterns.

## When it becomes relevant

EWC becomes relevant when we move beyond the current cache design toward something that:

1. **Uses a trainable model to produce or re-rank results.** A learned-to-rank model, a neural RRF fusion weight predictor, a query embedding adapter — any of these would need continual learning.
2. **Updates incrementally on live traffic.** Batch retraining avoids the forgetting problem by always seeing the full distribution. Online or streaming updates are where EWC matters.
3. **Serves multiple tenants or topics.** A multi-tenant deployment where tenant A's queries shouldn't degrade tenant B's ranking quality is exactly the scenario EWC was designed for.

## What we'd need to build

- A differentiable ranking or fusion component (doesn't exist yet)
- Fisher information computation at task boundaries (relatively cheap — one forward+backward pass over a sample)
- Storage for `θ*` checkpoints per "task" (where "task" could mean "this tenant's query distribution")
- A λ hyperparameter that balances plasticity vs. stability

## Reference implementation

- Rust reimplementation exists in RuVector (`crates/ruvector-gnn/src/ewc.rs`) — ~580 LOC. Full Fisher matrix computation and EWC loss. Code is usable as a reference if we get there. Note: we are not importing RuVector; this is research reference material only.
- PyTorch reference implementations are abundant; the algorithm is simple to port.

## Why not now

Our current cache doesn't train anything. It stores and retrieves with explicit rules (cubic similarity curve, rank decay, LRU eviction). No gradients flow, no forgetting is possible. This is a feature, not a limitation — the cache is trivially debuggable and its behavior is fully predictable.

Moving to a trained component would give us more sophisticated adaptation but at real costs: training infrastructure, explainability loss, new failure modes (overfitting, drift, gradient instability). Those costs only make sense if we've hit a ceiling with explicit rules — and we haven't.

## Open questions

- For our scale of data, does continual learning beat periodic retraining? If we can afford to retrain nightly, we don't need EWC.
- Could we apply EWC-style weighting to our explicit cache entries — protecting high-hit-rate entries from eviction more aggressively than recent-but-rare ones? This would be a much simpler application of the same idea.
- Is the multi-tenant case real? If deployments are single-tenant, the "forgetting across tasks" problem doesn't arise.

These are questions to revisit when we have usage data, not to answer speculatively.

## Related reading

- **Synaptic Intelligence** (Zenke et al., 2017) — similar goal, different mechanism. Tracks path integral of gradient contributions instead of Fisher information.
- **Progressive Networks** (Rusu et al., 2016) — sidesteps forgetting entirely by allocating new capacity per task. Works but doesn't scale.
- **Replay buffers** (Rolnick et al., 2019) — mix old task data into training batches. Simple, effective, widely used. RuVector's `replay.rs` implements this alongside EWC.

If we ever need continual learning, EWC is a reasonable starting point but not the only option.
