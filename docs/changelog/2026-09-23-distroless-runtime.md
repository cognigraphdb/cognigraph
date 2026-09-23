# v2.7.34 — Distroless runtime and in-server root start-up

- Date: 2026-09-23
- Status: v2.7.34
- Kind: Container security and packaging

## Changes

Both edition images move from `debian:stable-slim` with `curl` and
util-linux to `gcr.io/distroless/cc-debian13:nonroot`
([CG-81](../issues/CG-81.md),
[decision record](../decisions/decision_distroless_runtime.md),
[review](../verification/container-security-2026-09-23.md)).

- Trivy on the new images: 22 binary-package/CVE pairs, none high, none
  fixable, down from 229 pairs with 51 high. Every remaining pair has a
  recorded disposition based on the libraries the binaries load and the
  glibc functions they import.
- The Railway root start-up that `deploy/container-entrypoint.sh` performed
  now runs inside `cognigraph-server` before any thread, only when the image
  marker `COGNIGRAPH_CONTAINER_INIT=1` is set: the same contract and
  messages, then a drop to 10001:10001 with no groups, all capability sets
  empty and `no_new_privs`, verified before serving. The shell script is
  removed; `/usr/local/bin/cognigraph-entrypoint` remains as an alias for
  Railway's start command.
- The HEALTHCHECK runs the bundled `cognigraph health` (liveness and database
  readiness) instead of `curl`.
- Container checks use a digest-pinned busybox helper sharing the PID
  namespace or volume, and `docker cp` for licenses; they now also check GID,
  groups, all four capability sets and the HEALTHCHECK. The image identity
  check requires user `10001:10001` and the server entrypoint.
- A Lua sandbox test proves no script reaches LuaJIT's `io`, `os` or
  `package` libraries by any route, which the glibc dispositions rely on.

The workspace version moves to 2.7.34. No image is published by this change.

## Validation

Unit tests cover the start-up decision, one-time provisioning, symlink and
ownership refusals without repair, and thread counting. The full Docker suite
passed in both editions on the distroless images, including real root start-up
refusals, `/data/native` provisioning, PID 1 identity and capabilities,
restart, clean shutdown and both live Helm backups.
