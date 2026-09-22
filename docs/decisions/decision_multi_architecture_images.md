# Decision: native arm64 builds are qualified per platform and published as one index

Status: accepted and implemented 2026-09-22 for [CG-87](../issues/CG-87.md).
The first multi-architecture publication is pending an authorized release
from `main` and the Docker Hub tag-rule update in consequence 1.

## Context

Published images were `linux/amd64` only, so Apple Silicon and other ARM
hosts ran the database under emulation, which is too slow for a
database-backed development loop. No macOS artifact existed and no CI job
exercised an arm64 build of either edition. The Dockerfile itself is
architecture-neutral (`rust:1-slim` and `debian:stable-slim` are
multi-architecture) and the Docker suite already passed on an arm64 Docker
host locally. The publisher pushes only image IDs that passed runtime
checks, and Docker Hub enforces immutable `x.y.z` tags.

## Decision

1. **Per-architecture tags plus an index, test-before-push kept.** Each
   published version consists of `<version>-amd64` and `<version>-arm64`
   images and a multi-architecture index under `<version>`; `latest` is an
   index composed from the same per-architecture digests and must equal
   the version index digest. Untagged push-by-digest before testing was
   rejected because untested layers would reach the registry. The Docker
   Hub immutable-tag rule on both repositories becomes
   `^[0-9]+\.[0-9]+\.[0-9]+(-(amd64|arm64))?$`; the publisher refuses any
   other setting, so the owner updates both repositories before the first
   multi-architecture publication.
2. **Native runners, no emulation.** The CI Docker job is a matrix over
   `ubuntu-24.04` (`linux/amd64`) and `ubuntu-24.04-arm` (`linux/arm64`).
   Each leg builds both editions natively and runs the unchanged Docker
   suite: vulnerability gate, packaged-server checks, container startup and
   Helm live backups. `DOCKER_DEFAULT_PLATFORM` is the leg's platform and
   the image check refuses a mismatched platform. Cross-compilation and
   QEMU builds were rejected: LuaJIT's C build and a release Rust build
   under emulation are slow and would qualify a different toolchain path.
3. **Receipts link tested identities to uploads.** The image check writes
   `target/ci/images/<arch>.json` naming the platform, version, revision
   and the image IDs it ran. On a publishing run each leg exports exactly
   those IDs with `docker save`, refusing a tag whose ID no longer matches,
   and hands the tarballs to one publish job as a one-day artifact. The
   publish job loads every platform, proves each loaded ID equals its
   receipt and label metadata, reruns the runtime checks for the images of
   its own host platform, and only then uploads. No image reaches the
   registry from the matrix legs.
4. **Upload order.** Per-architecture tags are pushed and read back from
   Docker Hub first; indexes are created with `docker buildx imagetools
   create` from the verified digests and read back, including their
   platform coverage; aliases are created last from the same sources and
   must reproduce the version index digest exactly. Every upload appends
   to the Actions summary, including uploads before a later failure. The
   existing remote-head and `latest`-unchanged rechecks remain between
   phases.
5. **Apple Silicon by source build in CI.** There are no GitHub releases to
   attach a binary to, so a `macos-15` job builds both editions with the
   release lockfile and runs `scripts/check-platform-smoke.py`: startup,
   `/health/database` readiness, edition and version identity,
   unauthenticated rejection, an authenticated write, restart persistence
   and a CLI read. The same script runs on any developer machine. The job
   is required for `CI required`; a macOS binary attachment is future work
   if releases are introduced.
6. **Scope left out.** No `linux/arm/v7`, Windows or macOS container images;
   no signing; no change to Railway, which keeps deploying the amd64 member
   of the index.

## Consequences

- Developers on Apple Silicon pull native images with the same tags and
  digests recorded at publication; `--platform linux/amd64` is no longer
  needed from the first multi-architecture release onward. Earlier tags
  stay amd64-only.
- Publishing runs take one more job and about four image tarballs of
  artifact traffic; verification runs are unchanged in cost apart from the
  parallel arm64 and macOS legs.
- Local Docker checks qualify the host platform only. A passing local suite
  on an arm64 Mac is arm64 acceptance, not amd64 acceptance; CI provides
  both.
- Revisit when a release process with attached binaries exists, when
  Docker Hub offers digest-only uploads without tags in the immutable
  policy, or if arm64 runner availability changes.
