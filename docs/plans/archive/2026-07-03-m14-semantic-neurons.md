# M14 — Semantic Neurons port (all five decisions accepted)

Design: docs/semantic-neurons-port-design.md (ACCEPTED). Source research: research-hugegraph repo.

- [ ] Fixtures: their 3 space types + 4 evals + SOTU neurons + 4 chunk sets converted YAML→JSON under fixtures/semantic-neurons/ (provenance noted).
- [ ] New crate `cognigraph-construct` (deps: core + serde_json; dev: native, tokio):
  - types: SpaceType/EntityDef/RelationRule, Neuron (alias|relation_hint, lifecycle), EvalSpec, Fact
  - validate: neuron-vs-ontology (unknown entity/relation, empty evidence/triggers, kebab id, confidence range, status)
  - grounding: apply accepted neurons → effective config; mentions (names+aliases); negation-aware trigger grounding (clause-bounded lookback, ported `_affirms_phrase` semantics); GroundedFact carries trigger + evidence chunk
  - ingest: chunks → backend (chunks collection, entities global, MENTIONS edges, fact edges with space_id/evidence_chunk_id/neuron_id)
  - eval: construction-level recall (expected facts exist with evidence) + restraint (forbidden facts absent) — noted as construction coverage, not their retrieval metric
  - report: base-vs-neurons ablation (unchanged/recovered/redundant + attribution)
- [ ] CompletionProvider trait in cognigraph-embeddings (OpenAI chat + Gemini generateContent, JSON-schema constrained); gap-directed proposal engine emitting status=proposed neurons; env-gated live test.
- [ ] Parity tests: SOTU space grounding recall 8/8 expected facts (incl. 3 neuron-recovered), case:alpha 6/6, study:px-101 5/5, Case Bravo 9/9 recall + 0/4 forbidden (negation regression).
- [ ] Generalization experiment = documented follow-up runbook (harness ready; needs a third-party doc + human review pass).
- [ ] Docs (decision_semantic_neurons.md, plan/changelog), gates, push.
