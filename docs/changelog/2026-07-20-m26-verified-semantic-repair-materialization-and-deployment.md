# M26 verified Semantic Repair materialization and deployment

- Date: 2026-07-20
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:849-897` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **M26 verified Semantic Repair materialization and deployment.** Added an
  explicit synchronous generation build for a current M22/M23 promotion whose
  exact candidate also resolves through an approved M25 revision. On the
  atomic Native backend, the builder hash-reads the selected canonical prepared
  corpus from the tenant-incarnation local CAS, reruns the pinned grounder,
  requires exact semantic-fact equality with both original and replay
  derivation receipts, and retains one immutable complete entity/chunk/mention/
  fact occurrence projection. Each generation binds exact candidate-versus-
  baseline added, removed, and unchanged facts plus the complete projection
  digest. Building remains inert. Deployment is a separate externally signed
  `cognigraph.semantic-repair-deployment-intent.v1` Promoter act bound to the
  generation, impact, current promotion authority, M25 revision/review, and
  deployed-head compare-and-set. One Native transaction replaces the target
  space's chunks, mentions, and facts, inserts compatible missing shared
  entities, and commits the immutable deployment decision and derived head;
  other spaces are preserved and entities are never deleted. One generation is
  served per space, even when promotion uses multiple channels. Rollback first
  requires the ordinary signed promotion rollback, then a fresh signed M26
  decision naming the retained one-step-prior generation. HTTP and matching
  `semantic-repair generation`/`deployment` CLI surfaces build, list, inspect,
  deploy, and resolve current state without accepting private key material.
  Builds are capped at 1,000 chunks, 10,000 candidate and 10,000 baseline
  semantic facts, 50,000 total projection rows, and 16 MiB canonical generation
  records; retention is capped at 64
  generations per tenant incarnation, 16 per space, 256 MiB aggregate, and
  10,000 deployment decisions per space. M26 requires an atomic-batch backend:
  maintenance-mode ArangoDB still starts normally and reports M26 disabled,
  while M26 mutations and recovery of present M26 authority fail before CAS
  reads, M26 collection creation or authority writes, and graph mutation. This
  is explicit verified construction and signed deployment, not automatic
  healing, a drift scheduler, a durable materialization job, LLM
  qualification, physical
  per-generation routing, generation GC, distributed coordination, or HA.
  The exact Rust gates and the 81.04-second focused lifecycle passed, covering
  collision-safe concurrent replay, exact `+1/-1/1 unchanged` impact, stale
  target-row removal, exact A/B collection replacement, entity insertion and
  preservation, promotion-first rollback, isolated signed-intent and duty
  checks, tamper/recovery, snapshot conflict, other-space preservation, and
  tenant-incarnation isolation. An authenticated release-binary persistent-
  Native probe imported the full authority snapshot, replayed generation build
  and deployment, materialized the exact active `DISTRIBUTES` and `SUPPLIES`
  facts, remained healthy across explicit recovery and restart, and listed
  both retained generations through the CLI. A configured live ArangoDB
  Enterprise 3.12.9-1 probe started successfully, reported M26 disabled,
  returned HTTP 503 for authenticated generation build in 304 ms, wrote no M26
  authority or graph rows, and was cleaned back to its exact 13-collection
  baseline. The full evidence matrix and release-binary digests are recorded
  in the M26 decision.
