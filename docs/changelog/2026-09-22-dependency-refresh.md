# v2.7.18 — Dependency refresh for the develop freshness gate

- Date: 2026-09-22
- Status: v2.7.18
- Kind: Dependency refresh

## Changes

[CG-92](../issues/CG-92.md) refreshes the application dependencies that the
develop freshness gate found newer than the v2.7.15 qualification baseline:
pest and pest_derive 2.9.2, redb 4.3.0, rustix 1.1.5 and serde-saphyr 1.3.0
on the Rust side; Biome 2.5.14, CodeMirror state 6.7.5 and view 6.43.12,
antd 6.6.5 and react-router 8.4.0 in the console. Compatible transitive
resolutions were refreshed with them. No freshness exception was added, and
the vendored Tantivy 0.26.2 patch is unchanged.

redb 4.3.0 declares no file-format change. A store last written on
2026-08-30 was reopened by the candidate, written to, restarted and read back
with its evidence spans intact; details are in the ticket.

This record covers the third push-blocking finding on 2026-09-22, after the
[v2.7.17 rustls patch](2026-09-22-rustls-advisory.md). The workspace version
moves to 2.7.18 because newly changed content needs the next version.

## Validation

Dependencies, advisories, full CI and Docker suites pass locally on the
candidate; both editions' Clippy and tests, Native acceptance, browser suites,
image vulnerability gate, hardened container startup and live Helm checks are
included. See the
[verification record](../verification/dependency-refresh-2026-09-22.md).
This is not a release: no tag, GitHub release, Docker Hub update or hosted
deployment is claimed, and published images remain v2.7.14.
