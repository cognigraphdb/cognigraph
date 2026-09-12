# Continuous verification

The [workflow](../../.github/workflows/ci.yml) runs on PRs targeting `main`,
pushes to `main`, and manual dispatch. The same commands run locally:

```sh
python3 scripts/verify.py --suite ci
python3 scripts/verify.py --suite docker
```

The first suite includes frozen UI installation/checks/build, workflow lint,
Cargo and Bun advisory scans, script regressions, formatting, docs, issue and
edition boundaries, strict Clippy/tests in both editions, Helm rendering,
Native release acceptance and both-edition Chromium regressions. Rust builds,
Clippy and tests use the lockfile without modification.

The Docker suite builds both Linux images, tests their packaged server/CLI and
persistence, and exercises the rendered Helm backup in both editions. CI uses
Linux/amd64; local Docker defaults must target the same platform for publication
qualification. The chart probes use disposable Docker resources, not a cluster.

## Tools and diagnostics

Install Rust stable with Clippy/rustfmt, Bun, Playwright Chromium and its system
libraries, Docker, Helm 4.2.4, actionlint 1.7.12 and cargo-audit 0.22.2. CI pins
Action implementations to upstream commit SHAs. Rust stable, Bun latest and
Docker base-image latest tags retain the project's current-runtime policy.

Native acceptance runs through `scripts/native_ci.py` and the existing readiness
harness using a loopback embedding fixture and owned temporary stores. It
qualifies memory, resident/embedded, resident/sidecar and paged/sidecar modes
in both editions, plus invalid startup configuration. Every run gets a fresh
`target/ci/native/run-*` directory containing build/runtime logs, reports and
an overall summary, including on failure. The workflow uploads those results
and `ui/test-results/` for seven days even when earlier checks fail. Python
optimization is rejected or isolated so assertions cannot be silently removed.

Advisory scans fail on the tools' vulnerability errors and unavailable scans.
RustSec informational warnings remain visible; they are not suppressed or
reported as fixed. Current dependency follow-up is [CG-70](../issues/CG-70.md).
Weekly Dependabot updates cover Actions, Cargo and the UI Bun lockfile. Updates
arrive as PRs and do not authorize integration. Model APIs, research holdouts and
provider qualification are outside routine CI.

## GitHub protections

The public code repository uses a `main` ruleset requiring PRs, the GitHub Actions
`CI required` check on an up-to-date candidate, resolved review conversations,
and no branch deletion or force-push. The aggregate check requires both the
source/Native job and Docker/Helm job to succeed; failure, cancellation or skips
cannot qualify it. A second person's approval is not mandatory for this
owner-operated repository. No bypass actor is configured.

Action permissions stay read-only by default and cannot approve PRs. Action
references require full SHAs. Secret scanning and push protection are enabled
on the public code repository; Dependabot alerts and security-update PRs are
enabled. These settings are verified through GitHub's API, not inferred from YAML.

The separate website has its own `Website CI required` job and local
`scripts/verify.py`, covering Bun checks/audit and a non-root/read-only Docker
smoke across twelve SSR pages. GitHub currently refuses branch rulesets for
that private repository under its plan. Its visibility and billing are unchanged;
website CI does not itself enforce a protected merge until that capability is
available. See the website's own CI guide for its current verification state.

## Publication and deployment

Verification runs never log in to Docker Hub or publish images. Publication
requires `workflow_dispatch` with `publish_images: true` on the official `main`,
then independently passes the [publication preflight](docker-publishing.md).
PR code has no publication token. A manual run keeps its upload from being
cancelled by newer automated verification. Tests do not deploy an application.

The current authorization is to publish CI branches and open PRs. Merging those
PRs, publishing images, creating tags and deploying remain separate actions.
Default-branch automation and Dependabot configuration become active when the
workflow/configuration PRs are merged. Until then, PR checks validate their
candidate workflows and the existing main workflow retains its earlier trigger.
