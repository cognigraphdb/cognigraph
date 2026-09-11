# License split: FSL community core, commercial enterprise components

- Date: 2026-09-10
- Status: Unreleased
- Kind: Licensing

## Change

Replace the workspace's default `license = "MIT"` with a two-tier model
recorded in the [licensing decision](../decisions/decision_licensing.md).
Community crates (`core`, `query`, `native`, `arango`, `lua`, `cache`,
`embeddings`, `auth`, `cli`, `server`, `ui/`) are licensed under the
Functional Source License 1.1 with an Apache 2.0 future grant (`LICENSE`).
Enterprise crates (`governance`, `construct`, `artifacts`) and the
governance, Semantic Neurons, multi-tenancy and multi-node capabilities are
under the CogniGraph Enterprise License (`LICENSE-COMMERCIAL`, draft for
legal review). `LICENSING.md` is the plain-language map; `CLA.md` and
`CONTRIBUTING.md` add the sign-off requirement.

Each crate manifest now declares its own license instead of inheriting from
the workspace; Enterprise crates carry a `LICENSE` pointer file. The README's
license section points at `LICENSING.md`.

The build does not yet separate the tiers: the server links all crates and
tenancy lives inside it. [CG-45](../issues/CG-45.md) tracks the `enterprise`
feature gate. `third_party/` is empty; no vendored obligations exist.

## Validation

`Cargo.toml` and every crate manifest parse as TOML with a `license` or
`license-file` field. Documentation links, the decision index and the
changelog index pass their checks. No Rust code changed; no build was run for
this record.

Pre-publication review found conflicting tier assignments for the proposed read
replicas. The map and design now mark that assignment pending confirmation;
replication is not implemented. CG-45 remains the build-separation task.
