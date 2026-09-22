# v2.7.21 — Graph-augmented search warns when accepted rank hints are inert

- Date: 2026-09-22
- Status: v2.7.21
- Kind: HTTP contract

## Changes

`POST /api/search/graph-augmented` traverses `document_relations` by
default, while governed construction writes its facts into the managed
`facts` edge collection. Accepted `relation_rank_hint` neurons therefore had
no observable effect on a default request, silently.

Enterprise responses now include `warnings` with code `inert_rank_hints`
(plus `message`, `accepted_rank_hints` and `edge_collection`) when at least
one accepted rank hint exists and `edge_collection` is not exactly `facts`.
The check reuses the neurons read the ranking already performs. Fresh and
cache-assisted responses carry it; strong cache hits omit it because that
path performs no backend read. Community never emits it. Unreadable or
malformed neuron documents neither warn nor fail. The default edge
collection is unchanged in both editions; the
[decision record](../decisions/decision_graph_augmented_edge_collection.md)
states why. [CG-89](../issues/CG-89.md) is resolved.

The route's inline tests moved to `graph_augmented_tests.rs`, and the new
warning logic lives in `graph_warnings.rs` with its own `_tests.rs`, keeping
each module inside the line budget. No other behavior changes; the workspace
version moves to 2.7.21 because every push carries a version and a change
record.

## Validation

Unit tests cover exact collection identity, lifecycle states and neuron
kinds; route tests cover fresh, cache-assisted and strong-hit paths,
`facts`, unreadable collections, malformed documents, a zero facts limit and
a missing edge collection, on both editions. The live editions harness
asserts the warning on Enterprise and its absence on Community. The full
develop gate runs in the pre-push hook and pull-request CI. Not a release.
