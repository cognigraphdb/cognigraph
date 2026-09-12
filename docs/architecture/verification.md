# Verification

## Test Coverage

| Crate | Scope |
|---|---|
| cognigraph-core | Types, serde, error mapping, shared backend conformance suite |
| cognigraph-query | Parser/validation/planner/executor units plus the file-driven corpus (`tests/corpus/*.cgql`) |
| cognigraph-native | Backend contract, persistence, BM25, CGQL corpus equivalence, mutations, scan-count planner contracts |
| cognigraph-auth | Role/scope matrix, hashing, user/token lifecycle |
| cognigraph-cache | Weight curve, rank decay, similarity, invalidation |
| cognigraph-lua | Sandbox, instruction limits, RBAC-gated write mode |
| cognigraph-construct | Grounding gates, neurons, directed construction, WebNLG scorer, preparation replay |
| cognigraph-governance / artifacts | Canonicalization, signature verification, CAS tamper cases |
| cognigraph-server | Error mapping, rate limiter, metrics, OpenAPI drift, jobs, promotion/governance lifecycles |

Run `python3 scripts/verify.py --suite ci` for both editions, the dependency
boundary, documentation and browser regressions. No repository-wide count is
frozen here. Two construction live-loop cases return early unless explicitly
enabled; two embedding-provider cases are ignored in ordinary Rust runs.
Neither is executed provider coverage. See the [UI testing guide](../operations/ui-testing.md).

Native conformance covers memory, resident/embedded, resident/sidecar and
paged/sidecar stores, plus persistent reopen. The modes share CGQL code and
are not independent language implementations. Feature verification also requires
release-binary HTTP/Lua and restart checks; image qualification uses
`python3 scripts/verify.py --suite docker` and both Helm live checks. The
[Native-only acceptance report](../issues/native-readiness-2026-09-12.md) records
the qualified revision and exclusions.
