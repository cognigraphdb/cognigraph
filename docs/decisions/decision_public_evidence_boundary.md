# Public engineering records and private evidence

- Date: 2026-09-13
- Status: Active
- Related: [CG-75](../issues/CG-75.md), [CG-77](../issues/CG-77.md)

## Decision

Keep the code repository public, including the existing Enterprise source and
license split. Architecture, reference contracts, decisions, changelogs, research
conclusions, plans and generic operating guidance remain public. Preserve dated
negative results and the limits of each measured claim.

Raw research captures, labelled benchmark packages, provider prompt/response
traces, UI audits, real deployment inventories and internal remediation records
belong in the separate private `cognigraphdb/cognigraph-evidence` repository.
Original bytes and third-party notices remain intact there. Public references
use stable paths, byte lengths and SHA-256 digests in the
[evidence catalog](../evidence/README.md).

Keep ordinary builds and CI independent of private evidence. Retain the small
test inputs they need and move executable CI helpers into maintained public
locations. Do not turn a missing private artifact into a passing skipped test.

## Consequences

Public readers retain the engineering rationale and bounded results; replay of
private research runs requires access to the private artifacts. Hashes establish
byte identity, not scientific validity, timestamps or signatures. Public reading
copies identify their sealed originals and any navigation amendments.

This reduces duplicate capture distribution and the current checkout's size.
It does not make implementation prompts secret while their source remains public,
change third-party licences, or withdraw previously distributed Git objects.
Historical removal and publication require their own reviewed scope; immutable
image digests and their real source identities stay unchanged.

The [evidence policy](../operations/evidence-policy.md) owns the writer locations,
publication checks and review boundaries. The [batch plan](../plans/public-evidence-boundary-2026-09-13.md)
owns implementation and validation status.

## Archived navigation amendment — 2026-09-13

[CG-78](../issues/CG-78.md) qualifies navigation changes in archived reading
copies. Preserve the original bytes and historical migration manifests; identify
the amended copies and their originals explicitly. Check local links in archived
reading copies through `check-docs.py`, including directories, YAML files and
anchors. Only named originals whose bytes match the reviewed hash may be exempt.
The [amendment record](../plans/archive-navigation-2026-09-13.md) owns the paths,
checks and distinction between preserved originals and maintained navigation.
