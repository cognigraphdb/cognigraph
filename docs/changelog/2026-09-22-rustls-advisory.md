# v2.7.17 — Patch rustls for RUSTSEC-2026-0285 before develop integration

- Date: 2026-09-22
- Status: v2.7.17
- Kind: Dependency advisory

## Changes

The develop gate blocked the push of the v2.7.15 and v2.7.16 candidates on a
`cargo audit --deny unsound` error: RUSTSEC-2026-0285, "TLS 1.3 handshake
messages incorrectly accepted across encryption level boundaries", severity
5.3 (medium), in `rustls` 0.23.44. The advisory was published on 2026-09-14,
the same day the v2.7.15 candidate was qualified locally, so it was not
visible at qualification time.

`rustls` is a transitive dependency reached only through `reqwest` 0.13.5
(`hyper-rustls`, `tokio-rustls`, `rustls-platform-verifier`) in
`cognigraph-embeddings`, `cognigraph-construct` and `cognigraph-cli`. The
lockfile now selects `rustls` 0.23.45, the advisory's stated fix, through a
compatible `cargo update -p rustls`. No manifest requirement changed, no
exception record was added, and the pre-existing allowed warning for the
unmaintained `paste` crate (RUSTSEC-2024-0436) is unchanged.

The workspace version moves to 2.7.17 because newly changed content needs the
next version under the push policy.

## Validation

`python3 scripts/verify.py --suite advisories` passes after the update.
`cargo metadata --locked` accepts the refreshed lockfile. The remaining
develop-gate suites run in the pre-push hook for the authorized push. This
record does not claim a GitHub release, Docker Hub update or hosted
deployment; published images remain v2.7.14 until a release completes.
