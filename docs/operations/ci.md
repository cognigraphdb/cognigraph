# Continuous verification

The [workflow](../../.github/workflows/ci.yml) runs on PRs targeting `main` or `develop`,
pushes to either branch, and manual dispatch. The same commands run locally:

```sh
python3 scripts/verify.py --suite ci
python3 scripts/verify.py --suite docker
```

The first suite includes frozen UI installation/checks/build, workflow lint,
Cargo and Bun advisory scans, script regressions, formatting, docs, issue and
edition boundaries, strict Clippy/tests in both editions, Helm rendering,
Native release acceptance and both-edition Chromium regressions. Rust builds,
Clippy and tests use the lockfile without modification. The bounded Tantivy
snapshot also passes `scripts/check-vendored.py`, which checks published source
bytes and permits only the reviewed dependency-manifest patch.

The Docker suite builds both Linux images, tests their packaged server/CLI and
persistence, and exercises the rendered Helm backup in both editions. CI uses
Linux/amd64; local Docker defaults must target the same platform for publication
qualification. The chart probes use disposable Docker resources, not a cluster.
Their backup target is an owned Docker volume, so Linux ownership is exercised
on Docker Desktop too. A bounded provisioner initializes that temporary volume;
the backup writer and inspectors run without root or capabilities. Inspection
requires mode `0600`, reads as backup UID 10001 and verifies denial for UID 10002.
The harness removes its volume afterward and does not relax snapshot permissions
to make it readable by the host runner.

These suites run locally or inside the CI runner. The container startup check
emulates Railway's volume mount locally; it does not contact or start Railway.
Hosted QA runs only on the user's explicit request, independently of builds,
pushes and releases. After local testing, finish the
[Docker cleanup procedure](docker-cleanup.md), retaining only identified reusable
Evidence data volumes from the test session.

## Tools and diagnostics

Install Rust stable with Clippy/rustfmt, Bun, Playwright Chromium and its system
libraries, Docker, Helm 4.2.4, actionlint 1.7.12 and cargo-audit 0.22.2. CI pins
Action implementations to upstream commit SHAs. Rust stable, Bun latest and
Docker base-image latest tags retain the project's current-runtime policy.

Run `rustup update stable` before qualifying an outgoing candidate, then confirm
`rustc --version` and `cargo clippy --version`. A locally installed toolchain
named stable can lag behind the fresh stable installed by GitHub; CG-69's first
PR run exposed a new Clippy lint after local validation on an older release.

Native acceptance runs through `scripts/native_ci.py` and the existing readiness
harness using a loopback embedding fixture and owned temporary stores. It
qualifies memory, resident/embedded, resident/sidecar and paged/sidecar modes
in both editions, plus invalid startup configuration. Every run gets a fresh
`target/ci/native/run-*` directory containing build/runtime logs, reports and
an overall summary, including on failure. The workflow uploads those results
and `ui/test-results/` for seven days even when earlier checks fail. Python
optimization is rejected or isolated so assertions cannot be silently removed.

Advisory scans fail on the tools' vulnerability errors and unavailable scans;
`cargo audit --deny unsound` also rejects RustSec unsoundness advisories.
Maintenance warnings remain visible. [CG-70](../issues/CG-70.md) and the
[dependency decision](../decisions/decision_dependency_advisories.md) document
the bounded Tantivy patch and the optional ONNX paste maintenance exception,
including its review triggers. That exception does not suppress an advisory.
The [Renovate configuration](../../.github/renovate.json) covers Actions, Cargo
and the UI Bun lockfile, targeting only `develop`. Its weekly window is Monday
00:00–03:59 Europe/Bucharest; advisory updates use Renovate's security scheduling.
React and CodeMirror updates are grouped, with no automerge. Install the official
Renovate GitHub App for this repository only. Dependabot automatic security-update
PRs are disabled separately from vulnerability alerts, which remain enabled.
Updates arrive as PRs and do not authorize integration. Model APIs, research holdouts and
provider qualification are outside routine CI.

## GitHub protections

The public code repository uses rulesets for `main` and `develop` requiring PRs, the GitHub Actions
`CI required` check on an up-to-date candidate, resolved review conversations,
and no branch deletion or force-push. The aggregate check requires both the
source/Native job and Docker/Helm job to succeed; failure, cancellation or skips
cannot qualify it. A second person's approval is not mandatory for this
owner-operated repository. No bypass actor is configured.

Action permissions stay read-only by default and cannot approve PRs. Action
references require full SHAs. Secret scanning and push protection are enabled
on the public code repository; Dependabot vulnerability alerts stay enabled
while Renovate owns update PRs. These settings are verified through GitHub's API, not inferred from YAML.

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

`develop` is the default integration branch. `main` retains production history
and Railway's source branch. Promote an explicitly authorized release through a
verified develop-to-main PR. The [branch decision](../decisions/decision_develop_integration.md)
owns this boundary. [CG-73](../issues/CG-73.md) records transition status and remote
receipts; changing the default branch does not itself deploy either application.

## Public distribution gate

The shared CI runner also executes `python3 scripts/check-public-distribution.py`.
It enforces the [evidence policy](evidence-policy.md) without a private checkout.
Private archive verification is a separate explicit local check using
`--private-root ../evidence`; it never replaces ordinary public tests.
