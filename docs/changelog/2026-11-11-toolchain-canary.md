# v2.7.41 — Enterprise Clippy gate and toolchain canary

- Date: 2026-11-11
- Status: v2.7.41
- Kind: Maintenance and CI

## Changes

[CG-151](../issues/CG-151.md) is resolved.

- `release_cancel_requested` in the job manager used `AtomicU64::fetch_update`,
  which rustc 1.99 deprecates. Under `-D warnings` that failed the Enterprise
  Clippy gate. It now uses the infallible `update` with the same saturating
  decrement. A new test pins the behaviour: the `cancel_requested` gauge
  drops only when a job leaves that state and never wraps below zero.
- The owner kept Rust on floating stable and added a weekly canary. The new
  `.github/workflows/toolchain.yml` runs formatting and both Clippy gates on
  the default branch every Tuesday and on manual dispatch. It is not a
  required check. The [decision record](../decisions/decision_toolchain_policy.md)
  states the policy, including that no `rust-version` is declared, and the
  [CI guide](../operations/ci.md) says how to use the canary.
- Three policy tests keep the canary read-only, scheduled, limited to the
  Action revisions reviewed for the main workflow, and running the gates that
  block a push.

The incoming-work check that follows this change found no open pull requests
but 30 outdated direct dependencies and 11 advisories. They are recorded as
[CG-192](../issues/CG-192.md) and are not fixed here.

No API, storage or runtime contract changes.

## Validation

On rustc 1.99.1: `cargo fmt --all -- --check`, both Clippy gates with
`-D warnings`, `cargo test --locked --all` (809 passed, 2 ignored) and
`cargo test --locked --all --features enterprise` (1,036 passed, 2 ignored)
pass. `python3 -m unittest discover -s scripts/tests`,
`python3 scripts/check-server-modularity.py`, `actionlint`,
`python3 scripts/issue.py check`, `python3 scripts/check-docs.py`,
`python3 scripts/check-decision-index.py` and
`python3 scripts/check-public-distribution.py` report no errors.

The dependency and advisory suites were run and fail, as CG-192 records. Not
run: the UI, Native, Helm and Docker suites. The
gauge was not exercised over HTTP against a running Enterprise server, and
the scheduled workflow has not run on GitHub. Nothing was pushed, released or
deployed.
