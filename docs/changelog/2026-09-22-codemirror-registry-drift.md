# v2.7.19 — Take CodeMirror patch releases published during CI

- Date: 2026-09-22
- Status: v2.7.19
- Kind: Dependency refresh

## Changes

The GitHub CI run for the v2.7.18 candidate failed its dependency freshness
gate on two npm releases that appeared after the local gate had passed:
`@codemirror/state` 6.7.5 → 6.7.6 and `@codemirror/view` 6.43.12 → 6.43.13.
This is the registry-drift case the
[develop gate guide](../operations/develop-gate.md) describes: a green local
check does not survive a registry release, and the final recheck is
mandatory.

The two console ranges and the Bun lockfile take the new releases. No Rust
dependency changed; the Cargo lockfile changes only in the workspace version
fields. [CG-92](../issues/CG-92.md) records the addendum. The workspace
version moves to 2.7.19 because newly changed content needs the next version.

## Validation

The UI check, unit tests and build, the freshness gate and the browser suite
run locally on the candidate; the pre-push hook repeats the full develop gate.
This is not a release; published images remain v2.7.14.
