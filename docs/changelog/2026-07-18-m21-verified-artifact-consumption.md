# M21 verified artifact consumption

- Date: 2026-07-18
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:1040-1073` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **M21 verified artifact consumption.** Added an opt-in, read-only local content-addressed
  source scoped by tenant incarnation that streams and SHA-256-verifies every
  blob in the five active M20 manifests without dereferencing signed locations,
  accepting uploads, or fetching over the network. `PromotionContext` v4 pins
  one loader/ABI plan and uses the verified singleton `graph.json` and
  `oracle.json` as the actual distinct-fact evaluation inputs; corpus bytes are
  hash-read as provenance and do not reconstruct the graph. Scorer and verifier
  blobs must match the normal non-symlink executable-path digest pinned once
  through `current_exe()` at server startup and the frozen reproducibility
  digest, but staged code is never loaded or executed and this does not prove
  the process's mapped code.
  At most 10,000 entries and 4,096 unique digest+length pairs across the five
  manifests are admitted, and repeated verification is reused when its
  retained-byte requirement is already satisfied. The complete asynchronous
  consume/parse/score operation has a fixed 300-second deadline and polls
  durable cancellation, tenant suspension, and shutdown state once per second.
  A canonical receipt finalized after scoring binds the
  exact job attempt/input/payload/EvalSpec, five consumed slots, and canonical
  evaluation result; it is durably stored with successful job-schema-v2 results
  and bound through evidence/decision v4, head v2, and the promoter's signed
  `cognigraph.promotion-intent.v2` consumption-authority digest. The receipt's
  unkeyed hash does not independently authenticate server authorship. M18-M20
  records preserve their historical meanings and normal M21
  adoption requires a fresh target. This remains a singleton, selection-only
  contract: it adds no external signature over the receipt, remote attestation,
  graph derivation proof, deployment, consumer switching, quorum, or HA.
  Strict wire shapes, deadline-first timeout handling, full incoming Native
  snapshot preflight (including unreferenced schema-v2 jobs and divergent
  hot/archive duplicates), and candidate-plus-baseline Artifact Attestor duty
  separation close fail-open recovery and authority edges. Adversarial
  regressions, the exact workspace Rust gates, and release-binary authenticated
  HTTP lifecycles passed on 2026-07-18. Persistent Native completed in 5.09s;
  live ArangoDB Enterprise 3.12.9-1 completed in 103.09s and removed exactly 31
  probe records while restoring the exact baseline across 14 scoped collections.
