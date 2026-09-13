# Blind Eval — authoring kit (B1)

The blind eval is the moat test the generalization assessment said we owe
ourselves: a document AND its ground-truth authored by a person, unseen by
the system's builder, run once, cold. Whatever the numbers, they are the
first genuinely-blind evidence.

## The three files you author

Copy this TEMPLATE directory to `fixtures/semantic-neurons/blind/<name>/`
and fill in:

- **space_type.json** — the ontology. `entities[]` (name required; `type`,
  `aliases` optional) and `relation_rules[]` (`source`/`relation`/`target`
  are entity names + a relation label; `when_any` are trigger phrases,
  matched case-insensitively and negation-aware). Leakage-prone rules may
  opt into `"require_in_sentence": ["source"]` (sentence-scoped endpoint
  gate) and/or `{source}`/`{target}` template triggers (direction-faithful
  matching) — see dataops guide 04.
- **chunks.jsonl** — the document, one `{"id","title?","text"}` per line.
  Generate with the chunker (below), then hand-edit if needed.
- **eval.json** — `space_id` (any stable namespace string; it need NOT
  equal `space_type.id`) and `questions[]`, each with `expected_facts` and
  `forbidden_facts` in `A --REL--> B` notation. Facts use canonical entity
  **names** (not aliases) and relations that exist in the ontology.

## The blindness protocol (the actual point)

1. **You author space_type.json and eval.json. The builder does not see
   them until the run.**
2. **Write the ontology NAIVE** — triggers from domain expectation, not
   copied from the document's wording. If you reverse-engineer perfect
   triggers there are no gaps and the repair loop has nothing to do. The
   gaps are the experiment.
3. **Write expected/forbidden facts from the document but before any run.**
   Expected = what a correct reader extracts. Forbidden = the tempting
   wrong extraction (an entity that co-occurs everywhere but didn't do the
   thing). No tuning them after seeing what grounds.
4. **Seal and run once.** "Tweak a trigger, rerun until it looks good" is
   exactly the contamination this test exists to expose.

Pick a real, messy, third-party document in a domain where restraint
matters, with at least one built-in trap. ~15-60 chunks, 4-8 questions.

**A measured authoring note on meta-questions.** Questions like "which
inferences should be avoided?" or "what is explicitly unavailable?" ask a
fact-selector to reason about absence — and the answer stage measurably
under-asserts on them (they account for most of the residual answer-level
recall loss on the 2026-07 blind kits: several scored 0/N while their
sibling questions scored near-perfect). If a meta-question's ground truth
matters to you, phrase its `expected_facts` as ASSERTABLE facts — things
that are true and present in the graph (e.g. `Report --STATES_LIMIT-->
Coverage Boundary`) — rather than facts whose relevance is their own
absence. Construction-level scoring is unaffected either way; this is
purely about what an answering model will select from a trace.

## Tooling (none of it authors content or grounds anything)

```sh
# 1. Chunk a plain document into chunks.jsonl:
cargo run -p cognigraph-construct --example chunk_text -- DOC.md > chunks.jsonl

# 2. Structural check (parses + cross-references; does NOT ground, so it
#    cannot leak whether facts pass — a typo-catcher only):
cargo run -p cognigraph-construct --example blind_validate -- fixtures/semantic-neurons/blind/<name>

# 3. Run the eval cold (needs OPENAI_API_KEY for the propose/answer stages):
cargo run -p cognigraph-construct --example blind_eval -- fixtures/semantic-neurons/blind/<name>
```

`blind_eval` reports baseline construction recall/restraint, live
gap-directed proposals (written to `proposed.neurons.json` for review),
an IF-ACCEPTED re-measure, and the answer-level scorecard. Review the
proposals before trusting the "if accepted" numbers — acceptance is a
human act, not the runner's.
