# v2.7.16 — Construct lifecycle documentation and Semantic Neurons tickets

- Date: 2026-09-22
- Status: v2.7.16
- Kind: Documentation and issue registry

## Changes

The crate-level documentation of `cognigraph-construct` described the neuron
lifecycle as `proposed → accepted → graduated` and linked a design note path
that no longer exists. The comment now matches `NeuronStatus`
(`proposed`, `accepted`, `rejected`, `retired`), names graduation as the
leave-one-out redundancy report it is, and links
`docs/architecture/design-notes/semantic-neurons-port.md`.
[CG-91](../issues/CG-91.md) records the finding and its verification.

Three findings from a source review of Semantic Neurons, Side Views and the
graph-augmented search route are recorded as open tickets so that design
directions the paper licenses are tracked rather than remembered:

- [CG-88](../issues/CG-88.md): side views as a proposal-queue gap detector, a
  design direction stated in the Semantic Neurons paper (rev2, §8.2) with no
  implementation.
- [CG-89](../issues/CG-89.md): `POST /api/search/graph-augmented` defaults to
  `document_relations`, so neuron-built `facts` edges and accepted rank hints
  do not affect a default request.
- [CG-90](../issues/CG-90.md): gate refusals from `/construct/directed` and
  `/construct/propose` exist only in the HTTP response and are not durably
  recorded.

No runtime behavior changes. The workspace version moves to 2.7.16 because
every push carries a version and a change record.

## Validation

`cargo fmt --all -- --check` passes. `cargo doc -p cognigraph-construct
--no-deps --locked` builds; the two pre-existing rustdoc intra-doc-link
warnings in `draft.rs` and `webnlg/mining.rs` are unchanged.
`python3 scripts/issue.py check` and `python3 scripts/check-docs.py` report
no errors. The lockfile was refreshed for workspace members only and passes
`cargo metadata --locked`. Nothing was pushed, released or deployed.
