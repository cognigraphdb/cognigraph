# v2.7.27 — Native arm64 builds and multi-architecture publication

- Date: 2026-09-22
- Status: v2.7.27
- Kind: CI, packaging and distribution

## Changes

Both editions are now built, checked and published for `linux/arm64` as
well as `linux/amd64`, and verified from source on Apple Silicon
([CG-87](../issues/CG-87.md), [decision record](../decisions/decision_multi_architecture_images.md)):

- The CI Docker job is a matrix over `ubuntu-24.04` and `ubuntu-24.04-arm`.
  Each leg builds both images natively and runs the unchanged Docker suite
  (vulnerability gate, packaged-server checks, container startup, Helm live
  backups) with `DOCKER_DEFAULT_PLATFORM` set to its platform.
- `scripts/docker_images.py check` records the image IDs its runtime checks
  covered in `target/ci/images/<arch>.json`; new `export` saves exactly those
  IDs with `docker save` and refuses a rebuilt tag; new `load` proves loaded
  IDs equal their receipts. On a publishing run each leg uploads its export
  as a one-day artifact and a single `publish` job loads both, reruns the
  host-platform checks, pushes `<version>-amd64` and `<version>-arm64`,
  composes the `<version>` and `latest` indexes with
  `docker buildx imagetools create`, reads back digests and platform
  coverage, and requires the alias digest to equal the version index.
  The required Docker Hub immutable-tag rule becomes
  `^[0-9]+\.[0-9]+\.[0-9]+(-(amd64|arm64))?$` (owner action on both
  repositories before the first multi-architecture publication).
- A required `macos-15` job runs the new `scripts/check-platform-smoke.py`:
  release builds of both editions with the lockfile, then startup,
  `/health/database` readiness, edition and version identity,
  unauthenticated rejection, an authenticated write, restart persistence and
  a CLI read against the bare binaries. The script runs on any developer
  machine and writes `target/ci/platform/<os>-<arch>.json`.
- `CI required` needs the shared gates, both Docker legs and the macOS job.
  Image vulnerability reports are uploaded per architecture.
- README, running, CI, publishing and Docker Hub overview docs state the
  supported platforms and drop the emulation caveat for releases after
  2.7.27; tags through v2.7.14 stay `linux/amd64` only.

The workspace version moves to 2.7.27. Not a release; no image is published
by this change.

## Validation

Script regressions cover the immutable-tag rule, platform requirement,
receipts, export refusal after a rebuild, load identity checks, index
composition and platform coverage, the upload order (per-architecture tags,
then indexes, then aliases), alias equality and partial-failure receipts;
workflow policy tests cover the matrix legs, artifact hand-off, the gated
publish job, the macOS job and the aggregate check. The platform smoke passed
on Apple Silicon macOS for both editions. Manifest creation and alias digest
equality were exercised against a local registry with real amd64 and arm64
images. The Docker suite, export and load ran natively on an arm64 Docker
host. Remote runner results and the first multi-architecture publication
require the next authorized push and release.
