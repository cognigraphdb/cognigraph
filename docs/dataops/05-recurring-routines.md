# 05 — Recurring routines

Data changes; the graph, the neurons, and the caches all have opinions
about that. This page is the "what do I re-run when X happens" table,
plus the small set of periodic checks that keep a space healthy.

## When source documents change

Document text drift is the failure mode Semantic Neurons exist for:
trigger phrases stop appearing, pathways die silently, recall collapses
while nothing errors. The routine:

1. **Re-chunk and re-load** the changed documents (guide 01). Keep chunk
   ids stable so `/api/construct/ingest` can atomically replace the targeted
   chunks and retract only their stale evidence occurrences.
2. **Re-ground and re-measure** — `POST /api/construct/ingest` then
   `POST /api/construct/evaluate` (guide 04; both deterministic, no API key).
   Compare recall to the previous run. The evaluate call is read-scope
   and takes a stored spec, so this pair belongs in a cron job.
3. **Triage possible dead pathways** — the library/example degradation report
   checks literal trigger presence. It is not exposed over HTTP/CLI and does not
   reproduce template expansion, sentence endpoint gates, or blocker effects,
   so it can miss or falsely report a production pathway. Use it to nominate
   checks, then compare exact derived facts and evaluation gates; a durable
   generation-to-generation causal drift report remains future work.
4. **Repair the misses** (propose → review → accept) and **re-measure**:
   recall must close without opening violations. At volume, `cognigraph
   neuron review SPACE` can apply a pre-existing legacy review policy — the
   judge may auto-accept only eligible `relation_hint` proposals and queues
   aliases, blockers, rank hints, malformed verdicts, and every other case with
   triage attached (guide 04, §5; slice big queues with `limit` on the review
   call, and page them with `neuron pending SPACE --limit N`);
   `cognigraph neuron audit SPACE` is the sampled human spot-check that
   belongs in this routine too.
5. **Authorize the exact candidate** — for an M25 target, an authenticated
   PolicyAuthor submits the externally signed typed candidate revision, a
   distinct PolicyApprover signs approve or reject, and the unchanged
   evaluation/promotion workflow must select that exact candidate digest.
   Check it with `cognigraph semantic-repair current SPACE CHANNEL`. The legacy
   policy's model list is not signed judge qualification, and a head change
   does not trigger construction automatically.
6. **Apply explicitly** — use `/api/construct/governed-ingest` for a bounded
   supplied-chunk reconciliation. For one complete verified M22/M23 corpus,
   run `semantic-repair generation build`, inspect its exact impact and
   projection, then submit a separate externally signed Promoter deployment
   intent. M26 switches the target space atomically on Native; build is inert,
   synchronous, and not a durable/background job. Promotion does not trigger
   either operation.
7. **Graduation pass** — a revision can drift text back under a naive
   base rule, making an old neuron redundant:

   ```sh
   cognigraph neuron graduation acme_supply
   ```

   Review `covered_by_base` flags; retire deliberately, never
   automatically.

The mechanism is exercised end-to-end in
[../self-healing-experiment.md](../research/semantic-neurons/self-healing.md) — recall
collapsed 6/6 → 1/6 under a synthetic revision and was restored to 6/6
with restraint intact. The revision is synthetic and acceptance in that
readout was simulated; it demonstrates the invoked mechanism, not automatic or
independently validated repair of real-world drift.

## When the ontology changes

A new entity or relation rule is a *vocabulary* change: bump `version` in the
candidate's base space, rebuild and re-measure the exact typed candidate, then
submit a new signed M25 revision and independent review. Generic updates to
`space_types` are denied. After the existing promotion head selects the new
candidate, invoke governed ingest for the intended chunks and check graduation
in any legacy authoring workspace, or build/inspect/sign/deploy the complete
M26 generation from the verified corpus — new base rules routinely make old
hints redundant. Neither M25 selection nor M26 build mutates the live graph by
itself.

## Cache and index maintenance

- Server document, relationship, construction, neuron-transition, batch,
  read-write CGQL, write-capable Lua, and other managed mutation routes clear
  search results conservatively. Public opaque backend-query routes are
  disabled. Invalidation is dependency-safe rather than
  limited to the collection named in the cache key, because
  semantic/hybrid/graph results can embed documents, edges, and neuron rankings
  from other collections.
- A committed M26 deployment invalidates cached search results after its atomic
  target-space replacement. The retained inactive generation has no query or
  cache surface.
- M25-managed semantic-control and derived collections are read-only through
  generic mutation surfaces. Parsed CGQL and typed document reads may inspect
  them; opaque backend-native query text cannot reference them because its
  mutation behavior cannot be classified safely.
- Direct storage writes outside the server cannot participate in that barrier.
  Clear the result cache explicitly after those paths (and after a bulk load
  that bypasses the managed routes):

  ```sh
  cognigraph cache clear
  ```

- Vector/BM25 indexes are generation-stamped and rebuild lazily after
  data changes — no reindex command exists because none is needed.
- The persistent embedding cache never goes stale (embeddings are
  deterministic per text+model). The one event that should make you
  clear it is **changing the embedding model** — mixed-model vectors in
  one collection produce quietly wrong similarities:

  ```sh
  cognigraph cache clear
  ```

  and re-embed the affected collections (guide 01, §4).

## Periodic health checks

Weekly-ish, or after every significant load:

```cgql
// unembedded chunks (should be zero)
FOR c IN chunks_raw FILTER c.embedding == null RETURN c._key
```

```cgql
// facts whose evidence chunk no longer exists (should be zero after routed ingestion;
// catches legacy, imported, or out-of-band writes)
FOR f IN facts
  LET hit = (FOR c IN chunks
    FILTER c.space_id == f.space_id AND c.chunk_id == f.evidence_chunk_id
    RETURN 1)
  FILTER LENGTH(hit) == 0
  RETURN { fact: CONCAT(f._from, " --", f.relation_type, "--> ", f._to) }
```

```cgql
// proposed neurons waiting on review (should trend to zero)
FOR n IN neurons
  FILTER n.status == "proposed"
  COLLECT space = n.space_type WITH COUNT INTO waiting
  RETURN { space: space, waiting: waiting }
```

Plus, from the shell:

```sh
curl -s $COGNIGRAPH_URL/health && curl -s $COGNIGRAPH_URL/metrics | head
cognigraph cache stats
```

## Snapshots as a routine, not an emergency

```sh
cognigraph export --out "backup-$(date +%F).json"
```

Take one before every bulk load, ontology change, or neuron acceptance
batch. This preserves supported Native backend state; it is not a complete
disaster-recovery story. M21-M24 recovery also needs a verified artifact-CAS
bundle plus external configuration, trust anchors, and secrets. ArangoDB uses
operator-native database backup rather than CogniGraph snapshot export.
Composition and restore drills belong in [../operations.md](../operations/README.md).
Native M26 import preflight validates the stored-plus-incoming immutable
revision/review, generation, and deployment authority union; rebuilds derived
promotion/deployment heads; and reapplies the selected retained target
projection atomically. It still does not carry external CAS bytes or build an
unmaterialized promotion automatically. Preserve external signing keys and
trust configuration separately.

## The one-page cheat sheet

| Event | Do |
|---|---|
| New governed space from scratch | author exact candidate → measure → PolicyAuthor signature → independent PolicyApprover review → unchanged promotion → explicit governed ingest or M26 build/inspect/signed deploy (04) |
| New documents | chunk → load → embed → verify (01) |
| Docs revised | reconcile → degradation report → repair → re-measure → sign/review/promote exact candidate → explicit governed ingest or M26 build/inspect/signed deploy → graduation (04, 05) |
| Search looks wrong | retrieval checklist (03) |
| Ontology extended | candidate version bump → eval → new signed revision/review → promotion → explicit governed ingest or M26 build/inspect/signed deploy → graduation (04) |
| Embedding model changed | cache clear → re-embed everything (05) |
| Before anything scary | `cognigraph export` |
