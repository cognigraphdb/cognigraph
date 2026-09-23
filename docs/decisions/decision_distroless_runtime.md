# Decision: a distroless runtime with root start-up inside the server

Status: accepted and implemented 2026-09-23 for [CG-81](../issues/CG-81.md).
Amends the runtime part of the
[container vulnerability gate](decision_container_vulnerability_gate.md).

## Context

After CG-80 every image scan was clean of fixable findings, but 229
unfixable pairs (51 high) remained. Measured on 2026-09-23, 36 of the high
pairs came from util-linux and 8 from curl, the rest from ncurses, systemd,
acl and perl. The image needed `curl` only for its HEALTHCHECK and util-linux
only for `setpriv` in the Railway root start-up script; the other packages are
Essential in Debian and cannot be removed from `debian:stable-slim`. Dropping
curl alone would have left 43 high pairs.

## Decision

1. **Runtime base (owner choice).** `gcr.io/distroless/cc-debian13:nonroot`,
   pulled on every build like the former `debian:stable-slim`, so fixes arrive
   through the same `--pull` policy and the fixable-finding gate still blocks.
   It has glibc and CA certificates and no shell, package manager, curl,
   util-linux or Perl. The scan drops to 22 pairs, none high.
2. **Root start-up in Rust.** The image sets `COGNIGRAPH_CONTAINER_INIT=1`.
   Only with that marker and only as root, `cognigraph-server` enforces the
   former script's contract before starting any thread: exact Railway
   `/data` mount and Native path, no symlinks, create `/data/native` once as
   `10001:10001` mode 0700 and never repair an existing one. It then drops to
   UID/GID 10001 with no supplementary groups, empty effective, permitted,
   inheritable and ambient capability sets and `no_new_privs`, and verifies
   the result. Without the marker (bare binaries) or as non-root, nothing
   changes. Messages are unchanged, so operators see the same refusals.
3. **Paths.** The entrypoint is `/usr/local/bin/cognigraph-server`;
   `/usr/local/bin/cognigraph-entrypoint` remains as an alias so Railway's
   existing start-command override keeps working. The shell script is gone.
4. **Health.** The HEALTHCHECK runs the bundled `cognigraph health`, which
   checks `/health` and `/health/database`; a disconnected database now
   marks the container unhealthy.
5. **Verification without a shell.** Container checks read
   `/proc/1/status`, volume ownership and files through a digest-pinned
   busybox helper that shares the PID namespace or volume, and through
   `docker cp`. They now also assert GID, groups, all four capability sets
   and a healthy HEALTHCHECK.
6. **Dispositions, not suppressions.** Every remaining finding is reviewed in
   the [dated record](../verification/container-security-2026-09-23.md)
   against the libraries the binaries actually load and the glibc functions
   they import. No ignore or VEX file is added.

## Consequences

- Operators cannot `docker exec` a shell into the image; debugging uses the
  CLI or a helper container sharing the process or volume, as documented.
- The privilege-dropping code is now CogniGraph's to maintain; its behavior
  is covered by unit tests for the decision and provisioning logic and by
  real container checks on every Docker suite run.
- Revisit if the distroless project stops publishing Debian 13 images, if a
  future feature needs a runtime tool that only a full distribution provides,
  or when a static (musl or `static-debian13`) build becomes practical.
