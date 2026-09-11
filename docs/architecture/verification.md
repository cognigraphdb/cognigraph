# Verification

## Test Coverage

| Crate | Scope |
|---|---|
| cognigraph-core | Types, serde, error mapping, shared backend conformance suite |
| cognigraph-query | Parser/validation/planner/executor units plus the file-driven corpus (`tests/corpus/*.cgql`) |
| cognigraph-native | Backend contract, persistence, BM25, CGQL corpus equivalence, mutations, scan-count planner contracts |
| cognigraph-auth | Role/scope matrix, hashing, user/token lifecycle |
| cognigraph-arango | Client units; integration + conformance suite env-gated on `ARANGO_PASSWORD` |
| cognigraph-cache | Weight curve, rank decay, similarity, invalidation |
| cognigraph-lua | Sandbox, instruction limits, RBAC-gated write mode |
| cognigraph-construct | Grounding gates, neurons, directed construction, WebNLG scorer, preparation replay |
| cognigraph-governance / artifacts | Canonicalization, signature verification, CAS tamper cases |
| cognigraph-server | Error mapping, rate limiter, metrics, OpenAPI drift, jobs, promotion/governance lifecycles |

Run `cargo test --all` for the authoritative result; no repository-wide count
is frozen here. Live-provider and ArangoDB tests skip when credentials are
absent. Feature verification additionally requires a release-binary live run
(see `AGENTS.md`).
