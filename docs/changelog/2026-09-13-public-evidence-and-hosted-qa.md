# Separate private captures and make hosted QA on demand

- Date: 2026-09-13
- Status: v2.7.12
- Kind: Repository and operations

## Behavior

[CG-75](../issues/CG-75.md) moves raw research packages, provider traces and UI
captures into a private evidence archive. The public [catalog](../evidence/README.md)
identifies originals by path, size and SHA-256; public reading copies retain dated
results and limitations. Small public test inputs, source attribution, the Native
and subquery HTTP harnesses and modularity policy remain available to CI.

[CG-77](../issues/CG-77.md) adds a tested distribution check to shared CI and
updates the agent/QA instructions. DailyMed examples write local scratch under
ignored `data/dailymed/`; required research inputs must be prepared or supplied
explicitly. No provider calls or holdout runs are part of this change.

[CG-76](../issues/CG-76.md) implements the owner's clarified deployment model:
applications use their own CogniGraph instances, and the shared Railway database
is on-demand hosted QA. The database is stopped, its volume retained and its
automatic source trigger removed. The unused permanent Access/tunnel setup is
removed; the independently deployed website is unchanged.

## Evidence and publication boundary

Original captures and differing published v2.7.11 bytes are preserved privately.
Current-tree removal does not erase historical Git objects or previous public
copies. Source-history changes require separate reviewed publication scope;
existing immutable image digests and source labels remain historical identities.

The [acceptance summary](../verification/public-boundary-2026-09-13.md) records
verified archive digests, full CI without a private sibling, 15 browser journeys,
514 Native checks, both Docker editions and live Helm backup checks. The changed
research example also ran on fabricated inputs without provider calls. That checkpoint preceded the
[v2.7.12 source release](2026-09-13-v2-7-12.md); image publication and historical
Git removal remain separate operations.
