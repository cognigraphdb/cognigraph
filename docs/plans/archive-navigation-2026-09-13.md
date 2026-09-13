# Archived plan navigation amendment

- Date: 2026-09-13
- Issue: [CG-78](../issues/CG-78.md)
- Provenance: [Original paths, hashes and sizes](archive-navigation-2026-09-13.json)

## Reading copies and originals

The [implementation history](archive/implementation-history-through-2026-09-09.md)
and [roadmap](archive/roadmap-2026-h2.md) are now explicitly amended reading copies.
Their link targets use the current repository layout and public evidence pages.
Only navigation and the provenance notices changed; historical prose, dates,
measurements and scope remain unchanged.

Before amendment, both committed files matched the exact hashes recorded in the
[2026-09-10 migration manifest](documentation-migration-2026-09-10.json).
Their original bytes are retained separately in the private
`cognigraphdb/cognigraph-evidence` repository under
`archives/2026-09-13-archive-navigation/original/`, followed by the original
repository-relative path. The linked amendment JSON records each full private
path, SHA-256 and byte length. The original migration manifest is unchanged;
its hashes describe the original checkpoint, not these later reading copies.

The preserved [architecture original](archive/2026-09-10/architecture.original.md)
is unchanged and still matches its migration hash. Its three links refer to the
original source layout. Current navigation is in the
[architecture guide](../architecture/README.md).

## Verification contract

`python3 scripts/check-docs.py` checks archived reading copies, including directory
targets, YAML/JSON files and Markdown anchors. It is already part of the shared
CI and pre-push workflow alongside the separate public-distribution check.
There is no blanket archive-directory exemption.

The [link policy](../../scripts/policies/docs-links.json) names the one preserved
original exempt from link checking and pins its SHA-256 with a reason. A changed
or missing original, invalid policy, or newly added unlisted `.original.md` cannot
silently bypass the gate. Inline and fenced code examples remain source examples;
optional product-checkout links retain their existing explicit validation mode.

## Scope

The revised checker first reproduced all six outstanding archive links: five
fixture-directory references and the CI YAML reference. Repairs point to the
actual repository targets. Tests cover those target types, anchors, exact
original preservation, invalid exceptions and optional product access. The
ignored legacy issue-evidence bytecode remnant was removed.

No historical research results, runtime behavior, Railway state or published Git
refs are changed by this documentation fix. Source publication and any history
operation retain their separate review and verification requirements.
