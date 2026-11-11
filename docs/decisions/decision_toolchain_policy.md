# Decision: Rust stays on floating stable, with a weekly toolchain canary

Status: accepted and implemented 2026-11-11 for [CG-151](../issues/CG-151.md).

## Context

CI installs the `stable` Rust toolchain on every run, and the
[CI guide](../operations/ci.md) asks for `rustup update stable` before a
candidate is qualified. That is the project's current-runtime policy, shared
with Bun and the Docker base images. Its cost showed on 2026-11-11: rustc
1.99 deprecated `fetch_update` on atomics, so
`cargo clippy --features enterprise -- -D warnings` failed on a revision that
had passed every gate on 2026-09-23. Nothing ran in between, so the required
gate was red for an unknown time and would have been discovered only by the
next push.

## Decision

Owner decision, 2026-11-11.

1. **Floating stable stays.** CI and local qualification keep using the
   current stable release. No `rust-toolchain.toml` is added.
2. **Weekly canary.** `.github/workflows/toolchain.yml` runs every Tuesday at
   04:23 UTC on the default branch, and on manual dispatch. It installs
   stable and runs `cargo fmt --all -- --check` and both Clippy gates with
   `-D warnings`, the checks a compiler release can break without a code
   change. It prints the toolchain version it tested.
3. **Not a required check.** The canary never runs on pull requests or
   pushes and does not gate a merge. A red run is a finding: it is recorded
   as a ticket and fixed before the next outgoing candidate. A new lint is
   fixed in code; `#[allow]` needs its own recorded reason.
4. **No declared `rust-version`.** The supported compiler is the current
   stable release. No older compiler is built or tested, so a minimum version
   in `Cargo.toml` would be an unverified claim. Code may use an API as soon
   as it is stable.
5. **Rejected: pinning the toolchain.** A pinned version makes local and CI
   runs agree and turns each compiler upgrade into a reviewed change. It also
   adds a bump routine that nothing automates here today, and it departs from
   the current-runtime policy for the one tool where upstream releases are
   routine and backward compatible.
6. **Rejected: leaving the policy as it was.** The gate can then turn red
   between pushes with nobody looking, which is what CG-151 recorded.

## Consequences

- The canary reuses the Action revisions reviewed for the main workflow. A
  policy test in `scripts/tests/test_ci_policy.py` fails if they diverge, if
  the workflow gains a pull-request or push trigger, or if a gate is removed.
- The canary covers formatting and lints only. Tests, the UI, Native, Helm,
  advisory and dependency suites still run in the main workflow on pull
  requests and pushes.
- GitHub disables scheduled workflows in a repository with no activity for 60
  days; the canary then needs re-enabling in the Actions settings.
- The first remote run is unverified until this change reaches the default
  branch.
- Revisit if canary failures become frequent enough that a pinned toolchain
  with scheduled bumps would cost less.
