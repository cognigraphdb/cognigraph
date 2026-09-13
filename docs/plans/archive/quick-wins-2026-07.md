# Quick wins — post-v2.1.0 (planned 2026-07-06)

Small, high-leverage items from the honest assessment after v2.1.0.
Selection rule: each item either **protects the governance pitch**,
**completes the HTTP story**, or **tells us the truth before a customer
does** — and lands in roughly a day or less. Deliberately NOT here:
secondary indexes, ANN vector index, HA, multi-tenancy, SDKs — those are
phases, not wins, and stay on [roadmap-2026-h2.md](roadmap-2026-h2.md)'s
strategic list.

Working conventions apply: gates per commit, decision records for
contested calls, benchmarks-first for anything performance-shaped.

---

## Governance protection (the pitch is "reviewed" — prove who reviewed)

- [x] **QW1. Reviewer attribution on neuron transitions.** Landed
  2026-07-06: `proposed_by`/`proposed_at` on create; `reviewed_by`/
  `reviewed_at` + optional `review_note` on every accept/reject/retire
  (the middleware already inserted the authenticated `User` into request
  extensions — the handlers just never read it). Auth-disabled mode
  records `"anonymous"` honestly. Attribution appears in `GET /api/neurons`
  (raw docs) and on graduation candidates (`review` field, joined from
  the stored docs since the `Neuron` struct doesn't carry it). CLI:
  `cognigraph neuron accept KEY --note "..."`. Tests pin both auth
  modes + note persistence across later transitions; live-verified
  through real middleware (`reviewed_by: "admin"` end to end). Bonus
  catch: guide 04's authoring example omitted the required `evidence`
  field — fixed.

## Complete the loop over HTTP (ingest landed; measure and repair didn't)

- [x] **QW2. `POST /api/construct/evaluate`** — landed 2026-07-06: eval spec
  inline or stored (`eval_specs` collection keyed by space id) → recall,
  restraint, missing and violating facts against the live graph.
  Deterministic, no LLM; POST for the body but mounted read-scope via a
  separate merged router (viewers may measure; only editors ingest).
  Guide 04 §3 leads with the route; guide 05's revision routine is now a
  cron-able ingest+evaluate pair.
- [x] **QW3. `POST /api/construct/propose`** — landed 2026-07-06: gaps
  explicit or measured server-side from the (stored or inline) eval
  spec; BM25 evidence; verbatim-self-checked proposals stored as
  `proposed` with QW1 authorship — id conflicts and validation failures
  become visible skips, never partial drops. `CompletionProvider` wired
  into `AppState` (OpenAI preferred, Gemini fallback,
  `COGNIGRAPH_COMPLETION_MODEL`). Acceptance met literally: the full
  measure→miss→repair→prove cycle ran live with nothing but curl
  (recall 1/2 → propose → accept with note → re-ingest → 2/2, 0
  violations).

## Review ergonomics (the human is the bottleneck — spend on their time)

- [x] **QW4. CLI review workflow.** Landed 2026-07-06: `neuron pending
  [SPACE]` (digested queue, oldest first, with facts/triggers/authors)
  and `neuron show KEY` — the neuron plus every chunk its triggers
  match (full text, via a CGQL query over the space's chunks) AND any
  trigger matching no chunk (a dead trigger to question before
  accepting; for blockers, matches show what WOULD be suppressed).
  Read-only, zero server changes; live-verified.
- [x] **QW5. Trigger span provenance.** Landed 2026-07-06: fact edges
  carry `trigger_start`/`trigger_end` — the byte range of the AFFIRMED
  trigger occurrence in the chunk's ORIGINAL text (casefold offsets are
  mapped back through folding expansions; when a negated occurrence
  precedes the licensing one, the span points at the licensing one —
  both test-pinned, non-ASCII included). Additive field; grounding cost
  unchanged (bench re-run in-band). Live-verified: span sliced the
  original text to the exact affirmed phrase past a negated first
  occurrence.

## Truth-telling (before a customer does it for us)

- [x] **QW6. Hostile-scale eval.** Landed 2026-07-06:
  The corporate research kit, now excluded from the public snapshot,
  combined pharmaceutical vendor-research source chunks
  with deterministic adversarial near-misses (1,120 chunks total; 243
  source-derived, 877 trap chunks). Published result:
  baseline construction recall **37/37**, restraint violations **9/13
  distinct** after D4 counting semantics (historical per-mention total:
  14/18). After the authored fixture applied D1/D2 gates/templates to
  the risky rules, construction held recall at **37/37** and restraint
  closed to **0/13**; answer-level recall still failed, but answer
  restraint passed. QW6 exposed semantic restraint
  (company/direction leakage), not a proven trigger-matching performance
  problem.
- [ ] **QW7. (conditional) Aho-Corasick trigger/veto matching.** Only if
  QW6's corpus makes grounding cost visible (recorded trigger from the
  blocker decision stands: 64 vetoes ≈ 17 µs/chunk today, fine).
  QW6 did **not** justify this yet: it exposed quality/restraint failures,
  not matching cost. Benchmarks-first: no landing without a before/after
  row.

## Paper cuts (minutes each, do alongside)

- [x] **QW8a.** Landed 2026-07-06: TEMPLATE README now carries the
  measured meta-question note (phrase their ground truth as assertable
  facts; the answer stage under-asserts on absence-reasoning questions —
  they were most of the residual recall loss on the blind kits).
- [x] **QW8b.** Landed 2026-07-06: guide 03 ships the measured-good
  answer prompt (exhaustive selection + verbatim copying, 47%→78% on the
  blind kits) with the two properties to preserve when adapting it —
  including the verbatim rule that makes fabrication symbolically
  checkable downstream.

---

## Suggested order

QW1 → QW2 → QW3 (one arc: the governance record and the HTTP-complete
loop), QW4 + QW8 alongside, QW5 next, QW6 whenever the adversarial
author (you) has a corpus ready — it can run in parallel with everything
else. QW7 only if QW6 demands it.
