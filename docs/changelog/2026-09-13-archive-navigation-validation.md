# Validate archived navigation and retain original provenance

- Date: 2026-09-13
- Status: v2.7.12
- Kind: Documentation and CI

## Behavior

[CG-78](../issues/CG-78.md) fixes six remaining archive links to fixture
directories and the CI YAML file. The implementation history and roadmap are
explicitly identified as amended reading copies, with originals preserved
privately and verified against the unchanged migration hashes.

The existing docs checker now covers archived pages. Its single preserved-original
exception pins the exact source hash and fails if that source changes or disappears.
Directory, YAML, JSON and Markdown-anchor targets use the same validation path.
The shared CI runner already invokes this check; a regression protects that gate.
The ignored legacy issue-evidence bytecode remnant is removed.

## Verification

The revised checker reproduced all six missing targets before repair. Isolated
Git-checkout regressions cover archive navigation, exact-original exceptions,
invalid policies and optional product access. All eight new regressions and the
complete 88-test script suite pass, along with public/product docs, decision,
issue and distribution checks. Both original plans match their source hashes;
normalized reading-copy comparisons preserve the historical body text.
No database, Railway, provider or
model execution is required for this documentation and validator change.
See the [amendment record](../plans/archive-navigation-2026-09-13.md).
