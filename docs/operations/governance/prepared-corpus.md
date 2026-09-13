# Prepared Corpus

### M22 prepared-corpus derivation

M22 keeps the M21 local-CAS path and custody rules but changes the required
corpus and graph contracts for a new context generation:

1. Use context schema version 5 and the exact server-supported consumption
   plan schema version 2 from
   [`docs/examples/m22-derivation-plan.json`](../../examples/m22-derivation-plan.json).
   Job schema version 3 is selected automatically. Keep
   `reproducibility.backend = "artifact-snapshot"` and retain the complete M19
   governance and M20 five-attestation bindings.
2. Attest the corpus as `cognigraph.prepared-chunk-corpus.v1`, containing
   exactly one non-executable `application/json` entry named `corpus.json`.
   Its canonical schema-v1 object contains `space_type`,
   `corpus_revision_id`, `preprocessing_digest`, and sorted unique
   `chunks[] { id, title, text }`. It must match the context. Chunk text is
   non-empty NFC, may use only LF and TAB controls, and is capped at 1 MiB;
   chunk ids must not collide under the construction storage-key transform.
3. Attest the graph as `cognigraph.reproducible-evaluation-graph.v1`, with
   exactly two sorted non-executable `application/json` entries:
   `candidate.json` and `graph.json`. Both byte streams must already be the
   integer-only NFC canonical JSON encoding; alternate whitespace or object-key
   order is rejected even when it would parse to the same value.
4. `candidate.json` schema version 1 must match the frozen candidate kind, id,
   revision, artifact media type/length, and SHA-256 candidate digest. It
   contains one strict base space plus accepted neurons of exactly the closed
   `alias`, `relation_hint`, and `relation_blocker` shapes. Entities, rules,
   aliases, triggers, sentence gates, neuron ids, and evidence arrays must obey
   their sorted/unique and referential constraints. The worker computes the
   effective space and vetoes and requires the canonical resolved-configuration
   digest to equal the context's construction-configuration digest.
5. The pinned grounder runs over the prepared chunks without reading or
   mutating `GraphBackend`. It applies the existing entity/alias matching,
   negation, sentence-gate, direction-faithful trigger, accepted-neuron, and
   veto rules and emits sorted unique
   `{source, relation, target, evidence_chunk_id}` rows. The plan retains at
   most 64 MiB of corpus JSON, 8 MiB of candidate JSON, and 32 MiB of graph
   JSON; it admits at most 100,000 chunks, 100,000 configuration items, a
   deterministic text- and expansion-weighted 10,000,000-unit grounding-work
   estimate, a 250,000-unit per-chunk ceiling, and 100,000 derived fact rows.
   Candidate/surface/veto lookup and clause-negation detection are indexed.
   Trigger-template candidates are streamed rather than materialized as a
   Cartesian product, inserted surfaces remain literal, and repeated
   occurrences share one sentence-gate decision per rule and sentence. These
   are admission guards, not exact CPU, wall-time, or memory measurements;
   M21's aggregate manifest, unique-blob, byte-budget, deadline, and
   cooperative-cancellation limits also apply.
6. `graph.json` schema version 1 binds the space and graph revision, corpus
   manifest and canonical semantic digests, candidate digest, resolved
   construction digest, derivation-plan digest, facts digest, and fact rows.
   The server reconstructs that complete object from its verified read set and
   fails closed unless the reconstructed object, canonical bytes, and SHA-256
   blob digest exactly equal the claimed `graph.json`. A forged graph fails
   even when dropping or changing evidence rows would leave the distinct fact
   score unchanged. Only the reproduced facts are scored.
7. A successful result carries artifact-consumption receipt schema version 2
   with nested corpus-to-graph derivation receipt schema version 1. It binds
   exact corpus, graph, and oracle manifest projections back to the signed M20
   manifest digests, then binds corpus exact/semantic material, candidate
   exact/semantic material and length, resolved configuration, pinned
   derivation plan, construction read set, exact fact rows, exact oracle JSON,
   and claimed/derived graph equality. Offline recovery reconstructs the signed
   graph and oracle addresses and recomputes the score before accepting the
   receipt. Evidence
   schema version 5 binds the four
   source receipts and an explicit candidate/baseline derivation-authority
   projection. The promoter signs that authority in domain
   `cognigraph.promotion-intent.v3`; decisions use schema version 5 and the
   selected head uses version 3.

Do not submit a receipt or derived authority yourself. The server creates them
only after successful derivation, exact graph comparison, scoring, and final
active-artifact-authority validation. Their internal digests provide
deterministic consistency checks; once downstream signed authority binds those
digests, an unauthorized record change fails validation. They remain unkeyed
values created by the same server that performed the evaluation. The Artifact
Attestor signs the input byte claims and the promoter later signs the authority
binding; neither action is an independent witness to execution, a trusted
timestamp, a remote attestation, or proof of host integrity.

The shorthand “corpus-to-graph” is bounded. `corpus.json` already contains
prepared chunks, so raw-document decoding, Unicode normalization policy,
segmentation, chunking, and chunk-id assignment are not replayed. The derived
output is the canonical evidence-bearing evaluation-fact projection only. M22
does not reconstruct persistent documents, chunk/entity/mention collections,
indexes, trigger spans, storage keys, or the complete operational graph. It
does not publish or deploy `graph.json`, execute staged code, change a selected
consumer, or add replication, quorum, consensus, or HA.
