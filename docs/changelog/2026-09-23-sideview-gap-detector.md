# v2.7.37 — Side views as a proposal source

- Date: 2026-09-23
- Status: v2.7.37
- Kind: Feature (Enterprise, Semantic Neurons)

## Changes

`POST /api/construct/propose` can take its gaps from stored side views
([CG-88](../issues/CG-88.md),
[decision record](../decisions/decision_sideview_gap_detector.md)):

- New optional `side_views` request object: a source `collection`, optional
  `documents`, `min_support` (default 1), `max_candidates` (default 20, at
  most 100) and `dry_run`. It cannot be combined with `gaps` or `eval`, and
  unknown fields are refused.
- A co-mentioned entity pair becomes a candidate only when exactly one
  relation rule joins it and no fact edge of the space connects it. Every
  other pair is reported with a reason: `no_matching_rule`,
  `ambiguous_relation`, `endpoints_connected`, `below_min_support` or
  `over_max_candidates`.
- A dry run returns the detection report without a completion provider and
  writes nothing. Otherwise candidates go through the existing proposal path,
  and stored proposals add `proposed_from: "side_views"` and
  `side_view_source`. `proposed_by` still names the caller.
- Every propose response now includes `source` (`gaps`, `evaluation` or
  `side_views`).
- The detector lives in `cognigraph-construct` as
  `sideviews::gaps::detect_sideview_gaps` and is read-only.

The live measurement on a public kit is tracked as
[CG-97](../issues/CG-97.md). The workspace version moves to 2.7.37.

## Validation

Ten detector tests cover rule direction, ambiguity in relation and direction,
unmatched pairs, aliases and case, repeated mentions, support aggregation and
ordering, duplicate rules and view ids, exact rule endpoint names,
connections in either direction and in other spaces, a missing `facts`
collection and the support threshold. Eight route tests cover the dry run
without a provider, stored provenance, unchanged explicit-gap responses,
document narrowing, the support and candidate bounds, a store without side
views, selection validation and the provider requirement. One of them
compares `facts`, `entities`, `space_types`, `chunks`, `mentions` and
`side_views` before and after a proposal run. The full local gate passes.
