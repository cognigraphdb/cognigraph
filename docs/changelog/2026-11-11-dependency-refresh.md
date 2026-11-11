# v2.7.43 — Dependency refresh, Tantivy 0.27 and advisory closure

- Date: 2026-11-11
- Status: v2.7.43
- Kind: Dependencies and maintenance

## Changes

[CG-192](../issues/CG-192.md) is resolved under the
[refresh decision](../decisions/decision_dependency_refresh_2026_11_11.md).

- **Rust dependencies.** A full `cargo update` moved 130 crates, including
  `tokio` 1.54.1, `redb` 4.5.0, `icu` 2.4.0, `mlua` 0.12.2, `rustix` 1.1.6
  and the TLS stack (`rustls` 0.23.46, `rustls-webpki` 0.103.16,
  `webpki-root-certs` 1.0.12).
- **Breaking upgrades,** approved by the owner: `lru` 0.19, `serde-saphyr`
  2.1.1 and Tantivy 0.27.0. None needed a source change.
- **Vendored Tantivy retired.** Tantivy 0.27.0 publishes the corrected `lru`
  requirement, so the snapshot under `vendor/`, its provenance record, the
  Cargo patch declaration, the Docker copy, the separately packaged Tantivy
  licence file and `scripts/check-vendored.py` with its tests are removed.
  `.gitattributes`, which only described the snapshot, is removed too.
- **`unicode-normalization` pinned at 0.1.25.** Release 0.1.26 moves to
  Unicode 18.0.0 and fails the M23 compile-time guard. The pin has a dated
  exception until 2027-02-11, and [CG-194](../issues/CG-194.md) tracks the
  migration.
- **Optional ONNX provider.** `ort` 2.0.0-rc.15 creates sessions from an
  explicit environment; `onnx.rs` is adapted.
- **Console and TypeScript client.** React 19.3.1, React Router 8.5.1, Ant
  Design 6.7.0, the CodeMirror packages, Playwright 1.65.0, Biome 2.6.6 and
  the type packages. Biome 2.6 formats Markdown by default; both projects
  exclude `*.md` to keep the previous behaviour. One lint finding in the
  console's CGQL validation helper is fixed.

Advisories closed by the refresh: RUSTSEC-2026-0346 to 0349, 0382, 0383,
0385, 0386, 0391, 0395 to 0406, 0410 and 0486, and one yanked crate.

## Validation

On v2.7.43 with rustc 1.99.1, `python3 scripts/verify.py --suite dependencies`,
`--suite advisories` and `--suite ci` pass. The CI suite covers both Clippy
gates, 809 Community and 1,036 Enterprise tests, 178 script tests, the
console checks, tests and build, Helm rendering, Native acceptance (340 checks
per edition), the TypeScript client and the browser suite for both editions.

Stores written by the previous binary reopen under the candidate in three
storage modes for both editions, with text search over the old data, through
`scripts/check-dependency-upgrade.py`. The optional ONNX feature passes Clippy
and its offline tests; its inference test needs a local model and was not
run.

The Docker suite result is recorded in [CG-192](../issues/CG-192.md). Not
run: model providers, research holdouts, Railway or any publication. Nothing
was pushed, released or deployed.
