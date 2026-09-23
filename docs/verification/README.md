# Public verification inputs

The public acceptance harnesses run against real local binaries and disposable
Native stores. They do not need the private research archive.

- [Native runtime harness](harnesses/native-runtime-http.py) is called by
  [Native readiness](../../scripts/check-native-readiness.py) and the CI runner.
- [Subquery contract harness](harnesses/subquery-contract-http.py) executes the
  public CGQL corpus through the HTTP surface.
- [ArangoDB dump import harness](harnesses/arangodump-import-http.py) reads
  imported stores back through a real server in every persistent storage mode.
- [Modularity exceptions](../../scripts/policies/server-modularity-exceptions.json)
  are maintained policy inputs for the server size guard.

Raw historical execution captures are identified in the
[evidence catalog](../evidence/README.md). Write new captures to explicit external
or ignored scratch output paths and publish reviewed summaries under the
[evidence policy](../operations/evidence-policy.md).

## Acceptance records

- [Container vulnerability review — distroless runtime](container-security-2026-09-23.md)
- [Public evidence boundary and hosted QA](public-boundary-2026-09-13.md)
