# Public verification inputs

The public acceptance harnesses run against real local binaries and disposable
Native stores. They do not need the private research archive.

- [Native runtime harness](harnesses/native-runtime-http.py) is called by
  [Native readiness](../../scripts/check-native-readiness.py) and the CI runner.
- [Subquery contract harness](harnesses/subquery-contract-http.py) executes the
  public CGQL corpus through the HTTP surface.
- [Modularity exceptions](../../scripts/policies/server-modularity-exceptions.json)
  are maintained policy inputs for the server size guard.

Raw historical execution captures are identified in the
[evidence catalog](../evidence/README.md). Write new captures to explicit external
or ignored scratch output paths and publish reviewed summaries under the
[evidence policy](../operations/evidence-policy.md).

## Acceptance records

- [Public evidence boundary and hosted QA](public-boundary-2026-09-13.md)
