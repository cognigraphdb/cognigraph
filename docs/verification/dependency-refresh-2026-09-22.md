# Dependency refresh verification — 2026-09-22

Local acceptance of [CG-92](../issues/CG-92.md), candidate **v2.7.18**, on top
of the v2.7.15 candidate, the v2.7.16 ticket batch and the v2.7.17 rustls
patch. The refreshed tree is based on commit `2458c5188b67c7e0c4625054dc9a6d52bfb1630d`
(v2.7.17). This does not claim a GitHub push, merge, Docker Hub update or hosted
deployment. Public images remain v2.7.14.

Raw reports, before/after manifests and lockfiles, suite logs, the reopen
scripts and logs, and the upgrade-harness report live in
`cognigraph-evidence/runs/2026-09-22-cg-92-dependency-refresh/`. Its
`manifest.json` seals 25 artifacts with SHA-256
`ed6b6721dcf09ca57caf851ba8abe15ca69609583f57e98381c50484b27624b9`.
This digest identifies private evidence; it is not a signature or public
reproducibility claim.

## Dependency and advisory boundary

The read-only freshness gate reports **58 current direct dependency identities**,
zero Cargo/Bun compatible resolution drift and no exceptions. The dated
before/after direct versions are in [CG-92](../issues/CG-92.md).

Cargo audit passes with zero vulnerability or unsoundness findings after the
[v2.7.17 rustls patch](../changelog/2026-09-22-rustls-advisory.md); its sole
warning remains the optional `paste` maintenance advisory under its existing
review decision. Bun audit passes for 217 packages.

Both Linux/amd64 images pass the image vulnerability gate with **zero fixable
findings**. Unfixed findings remain in the reports under [CG-81](../issues/CG-81.md);
this is not zero-CVE certification or an exploitability assessment.

## Compatibility and runtime checks

- redb 4.3.0's published changelog states no file format change and a
  backwards-compatible release. No new redb feature or experimental API is
  enabled.
- The [upgrade harness](../../scripts/check-dependency-upgrade.py) ran the
  actual v2.7.17 and v2.7.18 **Enterprise** release binaries against disposable
  resident-embedded, resident-sidecar and paged-sidecar stores: legacy and new
  login, stored JSON and text index, write, restart and process-crash reopen all
  pass in all three modes. It does not test power loss, interrupted commits or
  downgrade compatibility.
- A private developer store last written on 2026-08-30, under an earlier redb
  series, was copied to a scratch path and opened by the candidate: nine
  collections with their counts were read, a document was written, and after
  a restart the new collection and document were present while the 259
  pre-existing fact edges still returned their evidence spans through CGQL.
- Both editions pass the Native release protocol: **257 HTTP checks** each,
  with 5 (Community) and 7 (Enterprise) startup rejections, across the
  memory/storage configurations.
- The Tantivy 0.26.2 vendored archive passes the integrity gate unchanged.

## Suite results

- `--suite dependencies`: PASS, zero drift.
- `--suite advisories`: PASS.
- `--suite ci`: PASS. UI check, 161 unit tests and build; fmt, actionlint,
  vendored integrity, public-distribution check, audit, script regressions,
  server modularity, editions, docs, decision index and issue registry; strict
  Clippy and tests for both editions, **1,645 passed, 0 failed, 4 ignored**;
  Helm rendering; Native acceptance and the browser suite for both editions.
- `--suite docker`: PASS. Both edition images build with `--pull`; image
  vulnerability gate; image checks (auth, edition API, CLI, licenses, restart,
  CGQL); hardened container startup; live Helm checks for both editions.
  The suite ran with the candidate labelled 2.7.17; the committed 2.7.18 tree
  differs only in the version fields of `Cargo.toml` and `Cargo.lock`.

Registry drift: the GitHub run for the v2.7.18 head failed the freshness gate
on `@codemirror/state` 6.7.6 and `@codemirror/view` 6.43.13, published after
the local gate passed. The v2.7.19 commit takes both; the UI check, unit
tests, build, freshness gate and browser suite were rerun on that head, and
the pre-push hook repeated the full ci and docker suites. The Rust
lockfile differs from the qualified v2.7.18 tree only in workspace version
fields.

Owned CI images and the temporary previous-binary worktree were removed; no
test containers remained. Not run: external providers, model benchmarks,
research holdouts, Railway or any publication.
