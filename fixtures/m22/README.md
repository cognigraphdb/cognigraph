# M22 reproducible-derivation authoring guide

M22 has no new signing key or HTTP mutation type. It changes the exact bytes
covered by the existing M20 `corpus` and `graph` attestations and introduces a
new `PromotionContext` generation. This directory documents those authoring
shapes; it contains no private key, secret seed, credential, usable signature,
or complete attestable package.

Use the server's exact checked plan from
[`docs/examples/m22-derivation-plan.json`](../../docs/examples/m22-derivation-plan.json).
Do not edit or recompute individual plan fields. A context-v5 evaluation accepts
only that complete plan object.

## Required M20 manifests

The corpus attestation must use artifact format
`cognigraph.prepared-chunk-corpus.v1` with exactly one manifest entry:

| Logical path | Media type | Executable |
|---|---|---:|
| `corpus.json` | `application/json` | `false` |

The graph attestation must use artifact format
`cognigraph.reproducible-evaluation-graph.v1` with exactly two entries in this
sorted order:

| Logical path | Media type | Executable |
|---|---|---:|
| `candidate.json` | `application/json` | `false` |
| `graph.json` | `application/json` | `false` |

Oracle, scorer, and verifier retain their M21 formats and entrypoints. Every
manifest entry's `byte_length` and `blob_digest` cover the exact raw bytes
staged in the tenant-incarnation local CAS.

## Canonical byte requirement

All three M22 JSON files must already be encoded as CogniGraph's integer-only
NFC canonical JSON: object keys NFC-normalized and sorted, strings NFC-
normalized, array order retained, no insignificant whitespace, and no floating-
point values. The formatted snippets below illustrate fields only; copying them
with indentation will fail exact canonical-byte validation.

Use `cognigraph_governance::canonical_json_bytes` in an external authoring tool,
write those bytes without a trailing newline, and compute SHA-256 over exactly
that byte sequence. The `candidate.json` blob digest is also
`promotion_context.candidate.candidate_digest`.

## Prepared corpus shape

```json
{
  "schema_version": 1,
  "space_type": "REPLACE_WITH_SPACE_TYPE",
  "corpus_revision_id": "REPLACE_WITH_CORPUS_REVISION",
  "preprocessing_digest": "sha256:REPLACE_WITH_64_LOWERCASE_HEX",
  "chunks": [
    {
      "id": "chunk-0001",
      "title": "Example title",
      "text": "Prepared NFC chunk text."
    }
  ]
}
```

Sort chunks by unique raw `id`. IDs must also remain unique after the existing
construction storage-key transform. The corpus is not a raw-document bundle:
decoding, normalization policy, segmentation, overlap, and id assignment have
already happened.

## Construction candidate shape

```json
{
  "schema_version": 1,
  "kind": "REPLACE_WITH_CONTEXT_CANDIDATE_KIND",
  "id": "REPLACE_WITH_CONTEXT_CANDIDATE_ID",
  "revision": "REPLACE_WITH_CONTEXT_CANDIDATE_REVISION",
  "base_space_type": {
    "id": "REPLACE_WITH_SPACE_TYPE",
    "name": "Example",
    "version": 1,
    "description": "Example governed construction space",
    "entities": [
      {
        "name": "CompoundX",
        "type": "compound",
        "aliases": ["compound x"]
      },
      {
        "name": "Meridian",
        "type": "organization",
        "aliases": ["meridian"]
      }
    ],
    "relation_rules": [
      {
        "source": "Meridian",
        "relation": "OWNS",
        "target": "CompoundX",
        "when_any": ["{source} owns {target}"],
        "require_in_sentence": ["source", "target"]
      }
    ]
  },
  "accepted_neurons": []
}
```

The displayed entity order is illustrative; authoring tools must sort every
required array according to the closed server contract. Accepted neurons may
be only `alias`, `relation_hint`, or `relation_blocker`, sorted by unique `id`.
Resolve the base space plus accepted neurons with the same pinned semantics and
put the canonical digest of `{schema_version, space_type, vetoes}` in
`promotion_context.effective_configuration.construction_config_digest`.

## Claimed evaluation graph shape

```json
{
  "schema_version": 1,
  "space_type": "REPLACE_WITH_SPACE_TYPE",
  "graph_revision_id": "REPLACE_WITH_GRAPH_REVISION",
  "corpus_manifest_digest": "sha256:REPLACE_WITH_CORPUS_MANIFEST_DIGEST",
  "corpus_semantic_digest": "sha256:REPLACE_WITH_CANONICAL_CORPUS_DIGEST",
  "candidate_digest": "sha256:REPLACE_WITH_CANONICAL_CANDIDATE_DIGEST",
  "construction_config_digest": "sha256:REPLACE_WITH_RESOLVED_CONFIG_DIGEST",
  "derivation_plan_digest": "sha256:22e20e4d05fe665976c5e3201756fd22cc372f6ae635623ffb87506ea91032de",
  "facts_digest": "sha256:REPLACE_WITH_CANONICAL_FACT_ARRAY_DIGEST",
  "facts": [
    {
      "source": "Meridian",
      "relation": "OWNS",
      "target": "CompoundX",
      "evidence_chunk_id": "chunk-0001"
    }
  ]
}
```

Sort and deduplicate facts by `(source, relation, target, evidence_chunk_id)`.
The server independently derives these rows, reconstructs this complete object,
canonicalizes it, and requires the result to equal the staged `graph.json`
bytes and manifest digest. A graph that preserves the distinct fact score but
changes evidence rows still fails.

## Authoring sequence

1. Produce the immutable prepared chunks and assign the corpus revision and
   preprocessing digest outside CogniGraph.
2. Produce and externally review the strict construction candidate.
3. Resolve its effective configuration and replay the pinned grounder in an
   authoring tool to obtain the exact fact rows.
4. Build canonical `corpus.json`, `candidate.json`, and `graph.json`; compute
   every exact-byte and semantic digest.
5. Build and sign the M20 corpus and graph attestations, alongside the unchanged
   oracle/scorer/verifier attestations. Stage all manifest blobs in the correct
   tenant-incarnation CAS scope.
6. Resolve the five active artifact bindings and freeze them, the exact plan,
   and the matching candidate/configuration/revision identities in a fresh
   context-v5 target.
7. Submit the evaluation. Never submit a derivation receipt: CogniGraph creates
   it only after exact replay, comparison, scoring, and final authority checks.

These shapes are not proof of raw-document preprocessing, semantic truth,
complete persistent graph reconstruction, independent execution attestation,
deployment, or HA.
