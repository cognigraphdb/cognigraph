# M22 reproducible corpus-to-graph derivation

- Date: 2026-07-18
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:1008-1039` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **M22 reproducible corpus-to-graph derivation.** Added a new, non-retroactive
  context-v5 authority generation that consumes a canonical singleton
  `cognigraph.prepared-chunk-corpus.v1` `corpus.json` and an exact canonical
  `cognigraph.reproducible-evaluation-graph.v1` package containing
  `candidate.json` and `graph.json`. Consumption-plan v2 pins the loader and
  prepared-corpus grounder identities, semantics, ABI, and closed limits. The
  server binds prepared chunks to the corpus revision and preprocessing
  digest, resolves the strict base ontology plus sorted accepted alias,
  relation-hint, and relation-blocker neurons, and requires the resulting
  effective-configuration digest to match the frozen context. It then replays
  the existing negation-aware, sentence-gated, direction-faithful and
  veto-aware grounder without reading the live graph. The claimed graph is
  accepted only when its sorted evidence-bearing fact rows, complete canonical
  envelope bytes, and SHA-256 content address exactly match the independently
  reconstructed artifact before scoring. Receipt v2 adds a nested derivation
  binding with exact fact rows and oracle bytes, so recovery reconstructs the
  signed graph/oracle addresses and recomputes the score. Candidate, entity,
  rule, veto, and clause-negation lookup is indexed; template substitution
  treats inserted surfaces literally, and sentence gates cache one decision
  per sentence.
  The receipt is carried through job v3, evidence/decision v5, head v3, and the
  promoter's signed `cognigraph.promotion-intent.v3` derivation-authority
  digest; restart recovery and Native snapshot-union preflight validate that
  chain. The receipt remains a server-produced unkeyed record rather than an
  independent attestation. This milestone starts from prepared chunks and
  reproduces only the canonical evaluation-fact projection: it does not replay
  raw-document preprocessing, reconstruct a complete persistent graph, execute
  staged code, deploy the selected head, or add remote attestation, quorum, or
  HA. Final workspace gates passed. Authenticated release-binary lifecycles
  passed on persistent Native in 5.24s and live ArangoDB Enterprise 3.12.9-1 in
  118.63s; the Arango probe removed 34 records, restored 0, and matched the
  exact pre-test baseline.
