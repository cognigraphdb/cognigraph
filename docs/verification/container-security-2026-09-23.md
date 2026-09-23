# Container vulnerability review — 2026-09-23

Local verification of [CG-81](../issues/CG-81.md) under the
[distroless runtime decision](../decisions/decision_distroless_runtime.md).
Scanner: Trivy 0.74.0 through the shared gate
(`scripts/check-image-vulnerabilities.py`), local Linux/arm64 builds of both
editions; CI repeats the scan natively on amd64 and arm64. Counts are
binary-package/CVE pairs; both editions are identical.

## Before and after

| Runtime | OS packages | Pairs | High | Medium | Low | Unknown | Fixable |
|---|---:|---:|---:|---:|---:|---:|---:|
| `debian:stable-slim` + `curl`, `util-linux` (v2.7.33) | 103 | 229 | 51 | 72 | 100 | 6 | 0 |
| same without `curl` (measured, not shipped) | 82 | 154 | 43 | 49 | 56 | 6 | 0 |
| `gcr.io/distroless/cc-debian13:nonroot` (v2.7.34) | 14 | 22 | 0 | 13 | 7 | 2 | 0 |

All twelve high-severity Trivy IDs listed in CG-81 are gone because their
packages are no longer in the image: util-linux (CVE-2026-76642, -78408,
-78409, -78410), curl (CVE-2026-12064, -8286, -8458, -8927), ncurses
(CVE-2025-69720), systemd (CVE-2026-16742), acl (CVE-2026-54369) and perl
(CVE-2026-9538). Removing `curl` alone would have left 43 high pairs, all in
packages Debian marks Essential, which cannot be removed from that base.

## What the binaries load

Both `cognigraph-server` and `cognigraph` link only `libc.so.6`, `libm.so.6`
and `libgcc_s.so.1` (ELF `DT_NEEDED`, read from the built images). The
distroless base also ships `zlib1g`, `libssl3t64`, `libzstd1`, `libstdc++6`
and `libgomp1`; neither binary links them, and neither names them for
`dlopen`. Of the glibc functions named in the remaining advisories, the
binaries import none of `strfmon`, `wordexp`, `glob`, `regcomp`/`regexec`,
`tsearch`/`tdelete`, `iconv*` or the `res_*` resolver API. The server imports
`fopen64`, `popen`, `system` and `dlopen` through the embedded LuaJIT's `io`,
`os` and `package` libraries; the Lua sandbox removes all three, and a new
test proves that no route (`_G`, `getfenv`, `rawget`, `package.loadlib`,
`io.popen`, `debug.getregistry`) reaches them. Both binaries import
`getaddrinfo`.

## Dispositions of the remaining 22 pairs

| IDs | Package | Disposition |
|---|---|---|
| CVE-2026-85091, CVE-2026-27171 | zlib1g | **Not loaded.** Present in the base; neither binary links zlib or names it for `dlopen`. Scout rated CVE-2026-85091 high on the old image; Trivy now lists it as medium. |
| CVE-2026-19499 (`strfmon`), CVE-2026-6368, CVE-2026-6791 (`wordexp`, tilde expansion), CVE-2010-4756 (`glob`), CVE-2018-20796, CVE-2019-9192 (regex), CVE-2026-19542 (`tdelete`), CVE-2026-77117, CVE-2026-80489 (iconv JISX0213) | libc6 | **Not reachable.** The vulnerable functions are not imported by either binary. |
| CVE-2026-18374 (`fopen` mode string) | libc6 | **Not reachable by input.** `fopen64` is imported only for LuaJIT's `io` library, which scripts cannot reach; Rust file access uses `open(2)` with fixed flags. |
| CVE-2026-89092 (nscd) | libc6 | **Not applicable.** No nscd daemon or socket exists in the image. |
| CVE-2026-5435 (TSIG), CVE-2026-8674 (long search domain), CVE-2026-6238 (crafted resolver input) | libc6 | **Residual, bounded.** `getaddrinfo` is used for outbound provider URLs and the CLI's `--url`. TSIG processing belongs to the `res_*` API, which is not imported. The search-domain path is configured by the container runtime's `resolv.conf`, not by requests. A malicious DNS server answering the host's resolver remains a precondition outside CogniGraph's control; it is recorded, not dismissed. |
| CVE-2019-1010022, -1010023, -1010024, -1010025, CVE-2026-86805, CVE-2026-95818 | libc6 (`ld.so`, `ldd`) | **Not reachable remotely.** They need a local attacker who can execute or load chosen ELF files. The image has no shell, no `ldd`, no package manager and runs only its two binaries as UID 10001 with no capabilities and `no_new_privs`. |

No finding has a Debian fix at this checkpoint, so none blocks the gate. No
ignore file, VEX statement or suppression was added; every pair stays in the
reports, and a future fix becomes blocking automatically.

## Validation

- Both images built on the pulled distroless base; the vulnerability gate,
  packaged-server checks (auth, edition API, CLI, licenses, restart, CGQL),
  container start-up checks and both live Helm backups passed.
- The root start-up path ran for real: a root start outside the Railway
  contract and a symlinked `/data/native` are refused; the provisioned
  directory is `10001:10001:700`; PID 1 has UID and GID 10001 in every slot,
  no supplementary groups, zero `CapInh`, `CapPrm`, `CapEff` and `CapAmb`,
  and `NoNewPrivs: 1`; shutdown is clean; the image HEALTHCHECK reports
  healthy through the bundled CLI.
- Railway's configured start command `/usr/local/bin/cognigraph-entrypoint`
  remains valid as an alias of the server.
- Not executed: a Railway deployment of this image, and Docker Scout (no
  Docker Hub login in this run). Exploitability was not tested; this is a
  reachability review, not a proof of absence.
