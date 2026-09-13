# Directed construction (D12): `POST /api/construct/directed`

- Date: 2026-07-22
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:423-443` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **Directed construction (D12): `POST /api/construct/directed`.** The classic
  pipeline extracts recurring corpus knowledge — rules are instance-anchored to
  entities a draft saw — which cannot express "does THIS document contain an
  exclusivity grant". The new mode takes a relation taxonomy (name,
  description, restraint vocabulary), has the completion model nominate
  per-document facts constrained to it, and lets deterministic gates decide:
  the relation must be in the taxonomy, the evidence must be a verbatim quote
  of a submitted chunk (whitespace-tolerant matching, offsets recovered into
  the original text), both endpoints must occur in the evidence sentence, and
  at least one vocabulary phrase must be present AND affirmed — negated
  vocabulary grounds nothing. Survivors flow through the same writer as rule
  grounding (`ingest_chunks_grounded_by`, the new pluggable seam): identical
  occurrence-v1 rows, per-chunk delete-and-rebuild reconciliation, semantics
  sidecar. A directed fact is distinguishable by its `reviewed_by` attribution
  (`directed:<model>@directed-policy-v1`), not by a parallel schema. A
  taxonomy relation without restraint vocabulary is refused — the model's
  say-so never stands alone. One request is exactly one completion call
  (32-chunk cap); larger corpora go as consecutive slices, idempotent per
  chunk. Tests cover each gate rejection, offset recovery, idempotent
  re-ingest, and the vocabulary requirement.
