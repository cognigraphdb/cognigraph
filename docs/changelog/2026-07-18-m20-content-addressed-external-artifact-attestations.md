# M20 content-addressed external artifact attestations

- Date: 2026-07-18
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:1074-1101` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **M20 content-addressed external artifact attestations.** Added a purpose-isolated
  `artifact-attestor` role/scope and root-certified key purpose, plus immutable
  signed exact-byte manifest claims for corpus, graph, oracle, scorer, and
  verifier inputs. Canonical manifests bind sorted portable logical paths,
  media types, byte lengths, executable flags, and SHA-256 blob digests; signed
  kind-specific subjects tie those bytes to exact revision, configuration,
  EvalSpec, case-manifest, semantics-identity, and executable identities.
  `PromotionContext` v3 requires all five active attestations alongside M19
  policy authority. Evidence v3 copies candidate/baseline sets and the
  promoter's signed intent binds their aggregate artifact-authority digest.
  Original/replay sets must match; candidate/baseline corpus, oracle, scorer,
  and verifier bindings remain equal, while graph may differ only under the
  existing `graph_revision` policy allowance. Artifact attestors are distinct
  from policy author, approver, and promoter. Protected storage, a 64 MiB
  aggregate canonical-manifest budget, compact 50-record list pages, full
  detail/show and resolve API/CLI surfaces, recovery validation, promotion
  status, and Native stored-plus-incoming snapshot preflight cover the new
  authority. V1/v2 records retain their meaning, generations cannot mix per
  target, and normal M20 adoption uses a fresh v3 target. The server validates
  signed claims and bindings but never fetches locations or stores the external
  bytes; evaluation still reads the live graph, so M20 does not prove
  consumption, availability, custody, semantic truth, or freshness. Native
  snapshots contain records rather than artifact bytes; ArangoDB still has no
  CogniGraph application snapshot. Adversarial tests, the exact workspace Rust
  gates, the stored full-v3 evaluation/evidence/promotion lifecycle, and
  release-binary persistent-Native/live-Arango HTTP lifecycles passed on
  2026-07-18; the M20 decision records restart, revocation, request-bound, and
  exact-cleanup evidence.
