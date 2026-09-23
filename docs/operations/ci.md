# Continuous verification

The [workflow](../../.github/workflows/ci.yml) runs on PRs targeting `main` or `develop`,
pushes to either branch, and manual dispatch. The same commands run locally:

```sh
python3 scripts/verify.py --suite ci
python3 scripts/verify.py --suite docker
```

The first suite includes frozen UI installation/checks/build, workflow lint,
Cargo and Bun advisory scans, read-only dependency freshness checks, script regressions, formatting, docs, issue and
edition boundaries, strict Clippy/tests in both editions, Helm rendering,
Native release acceptance and both-edition Chromium regressions. Rust builds,
Clippy and tests use the lockfile without modification. The bounded Tantivy
snapshot also passes `scripts/check-vendored.py`, which checks published source
bytes and permits only the reviewed dependency-manifest patch.

The Docker suite builds both Linux images on the pulled distroless runtime,
scans both image IDs for vulnerabilities, tests their packaged server/CLI and
persistence, and exercises the rendered Helm backup in both editions. The
runtime has no shell, so process, file and volume checks run in a
digest-pinned busybox helper that shares the container's PID namespace or
volume; `docker cp` verifies packaged licenses. CI runs
it natively on `linux/amd64` (`ubuntu-24.04`) and `linux/arm64`
(`ubuntu-24.04-arm`) as a matrix; each leg sets `DOCKER_DEFAULT_PLATFORM`
and the image check refuses a mismatched platform. A local run qualifies the
Docker host's own platform only. A separate `macos-15` job builds both
editions from source on Apple Silicon and runs
`scripts/check-platform-smoke.py`, uploading `platform-smoke-results`.
`CI required` needs the shared gates, both Docker legs and the macOS job.
The chart probes use disposable Docker resources, not a cluster.
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
Action implementations to upstream commit SHAs. Install Trivy 0.74.0 as well
(`brew install trivy` on macOS; CI uses the pinned official installer). Rust stable, Bun latest and
Docker base-image latest tags retain the project's current-runtime policy.

Run `rustup update stable` before qualifying an outgoing candidate, then confirm
`rustc --version` and `cargo clippy --version`. A locally installed toolchain
named stable can lag behind the fresh stable installed by GitHub; CG-69's first
PR run exposed a new Clippy lint after local validation on an older release.

The `client` suite (`python3 scripts/verify.py --suite client`, part of the CI
suite) installs `clients/typescript` from its frozen Bun lockfile, runs Biome,
TypeScript and the unit tests, builds the package, builds the Community
server and runs the live tests: every client call against a disposable
server, the compiled package under Node and the documentation example. Its
lockfile is part of the dependency-freshness gate and `bun audit`.

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

## Container vulnerability gate

The [container vulnerability gate](../decisions/decision_container_vulnerability_gate.md)
runs `scripts/check-image-vulnerabilities.py` inside the Docker suite. It rejects
every finding with a fixed version at any severity. It needs local Docker access
and network access to Trivy's vulnerability database, without a Docker Hub login.
Scanner errors and missing inventory fail the gate. It scans the exact local IDs
and ignores local suppression/configuration overrides. Reports under
`target/ci/image-security/run-*/` include all findings, scanner/database metadata
and image IDs; CI uploads them as `image-security-results-<arch>` for seven
days, even if the scan fails. Unfixed findings remain visible and require review; a passing
gate is not a zero-CVE claim. [CG-81](../issues/CG-81.md) owns the remaining
2026-09-14 review. Cargo/Bun scans cover dependencies that an image scan may miss.

## GitHub protections

The [develop qualification gate](develop-gate.md) requires current direct
application dependencies and fresh compatible Cargo/Bun resolutions, including
optional Cargo feature paths and UI development dependencies. Registry or
resolver errors fail the check. The local push hook and workflow bracket source
qualification with incoming PR snapshots; the Docker job checks incoming work
again before it can pass or publish. These steps have read-only PR/check/status
permissions, and their token is scoped to the inspection steps rather than all
source tests. Full Git history is required in both jobs.

Dependency inventories and resolver logs under `target/ci/dependencies/`, and
`target/ci/incoming.json`, are included in acceptance diagnostics. Review/check
states are recorded separately from exact-head accounting. The owner still reviews
the work and authorizes merges. [CG-82](../issues/CG-82.md) tracks this local
workflow change. [CG-83](../issues/CG-83.md) qualifies the refreshed combined
candidate locally; remote activation still requires an authorized push and
passing remote CI.

The public code repository uses rulesets for `main` and `develop` requiring PRs, the GitHub Actions
`CI required` check on an up-to-date candidate, resolved review conversations,
and no branch deletion or force-push. The aggregate check requires the
source/Native job, both Docker/Helm legs and the Apple Silicon job to succeed;
failure, cancellation or skips cannot qualify it. A second person's approval is not mandatory for this
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
On such a run each Docker leg exports the image IDs its checks covered as a
one-day `tested-images-<arch>` artifact, and a single `publish` job loads
both, reruns the host-platform checks and composes the multi-architecture
tags. PR code has no publication token. A manual run keeps its upload from being
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
