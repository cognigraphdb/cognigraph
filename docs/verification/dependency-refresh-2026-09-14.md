# Dependency refresh verification — 2026-09-14

Local acceptance of [CG-83](../issues/CG-83.md), candidate **v2.7.15**, including
the preceding CG-80 container fix and CG-82 qualification gates. The source is
uncommitted on base `c413e784291306eec137957b9bc1293b08804759`; private input
hashes identify the tested changes. This does not claim a GitHub push, merge,
Docker Hub update or hosted deployment. Public images remain v2.7.14.

Raw reports, source/archive checksums, test-development failures and passing
captures live in `cognigraph-evidence/runs/2026-09-14-cg-83-dependency-refresh/`.
Its `manifest.json` seals 1,082 artifacts with SHA-256
`b3eda82180d583fdf0f649b3f8577d4beb4542e1619d4a2c242102db1e76ea25`.
All listed file sizes and hashes were verified after sealing. This digest
identifies private evidence; it is not a signature or public reproducibility claim.

The final combined `python3 scripts/verify.py --suite ci` run and the full
`--suite docker` run pass. The checks below describe their executed scope and
the additional compatibility probes.

## Dependency and advisory boundary

The read-only freshness gate reports **58 current direct dependency identities**,
zero Cargo/Bun compatible resolution drift and no exceptions. The dated before/after
direct versions are in [CG-83](../issues/CG-83.md); the private package contains
complete lockfile deltas and fresh registry observations.

Cargo audit passes with zero vulnerability/unsoundness findings. Its sole warning
is the existing optional `paste` maintenance advisory. Updating the macro helper
to 0.2.3 removes one dependency path, while tokenizers 0.23.2 still uses paste
directly. The [reviewed maintenance decision](../decisions/decision_dependency_refresh_2026_09_14.md)
retains the existing deadline and optional-feature boundary. Bun audit passes.

Both Linux/amd64 images pass Trivy 0.74.0 with **zero fixable findings** and no
critical findings in that scan. Each still has 223 package/CVE pairs (103 unique
IDs): 51 high, 81 medium, 90 low and one unknown, all without an available fix
in the scanner's database. [CG-81](../issues/CG-81.md) retains that work. This is
not zero-CVE certification or an exploitability assessment; Cargo/Bun scans
provide separate application-dependency coverage.

## Compatibility and runtime checks

- Two new authentication regressions verify an independently generated Argon2
  0.5.3 PHC hash, wrong/corrupt-password rejection, unchanged stored legacy hash,
  retained default parameters and independent salts for new accounts.
- The [upgrade harness](../../scripts/check-dependency-upgrade.py) runs actual
  v2.7.14 and v2.7.15 **Enterprise** release binaries against disposable resident/
  embedded, resident/sidecar and paged/sidecar stores. Old users, exact JSON and
  text search survive reopening; new accounts/writes survive restart and a
  subsequent process-crash recovery. It does not test power loss, interrupted
  commits or downgrade compatibility.
- Both editions pass the Native release protocol: **514 HTTP checks and 12
  startup rejections** across all four memory/storage configurations. Existing
  persistence, index reopening and stored-document cache regressions pass.
- The Tantivy 0.26.2 archive checksum and all 373 retained release files pass the
  integrity gate after reversing only the reviewed lru manifest correction.
- Optional ONNX strict Clippy and compilation/tests pass: 20 unit tests and the
  deterministic Unicode/BPE/padding regression. The explicitly enabled local
  MiniLM test performs real inference, checks 384 finite dimensions, repeatability,
  distinct outputs and single-versus-batch agreement. The existing model/tokenizer
  files are hashed privately. No model was downloaded; this is integration
  compatibility evidence, not embedding-quality or provider benchmarking.

## Acceptance suites

- Formatting and strict Clippy pass in both editions. Rust tests pass:
  **727 Community and 918 Enterprise**, with two hosted-provider tests intentionally
  ignored in each. The optional ONNX inference test is ignored by default and
  was also executed explicitly with the local model as recorded above.
- All **112 Python regression tests** pass, including gate failures and vendor
  integrity negative controls. Documentation, decision/issue indexes, public
  distribution, edition boundaries, modularity, Actionlint and Helm checks pass.
- UI Biome/TypeScript checks pass without the new selector-order warnings; all
  **161 unit tests** and the production build pass. The CSS repair only reorders
  existing rules/selectors and preserves declarations.
- **17 Chromium cases** pass against real Community/Enterprise servers serving
  production assets: login/errors, small viewports, document CRUD/reload,
  access boundaries, CodeMirror and the added graph traversal/resize/remount
  case. Manual local browser inspection also verifies the graph's visual/JSON
  switch, both vertices and relationship. This does not qualify every browser
  or every console workflow.
- Both Docker builds pass advisory scanning, auth/API/CLI/license checks,
  restart/CGQL persistence, console assets/deep links, root/symlink rejection,
  UID/capability/volume checks, and both rendered Helm private-backup/denial probes.

## Cleanup and remaining release work

The test harnesses remove their stores and servers. The manual preview was
closed and stopped. Both temporary Docker image tags were removed after their
last consumer; before/after inventories preserve all pre-existing container,
image and volume identities. The copied baseline executable was removed after
the upgrade check; its hash and reproducible harness remain.

Commit, protected-branch integration, remote CI and advancing Docker Hub's
`latest` tags remain release work. Recheck incoming PRs, registry/advisory state
and the exact candidate under the [push policy](../operations/push.md). Railway
was not started or tested.
