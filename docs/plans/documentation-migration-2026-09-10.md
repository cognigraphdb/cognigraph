# Documentation migration

- Date: 2026-09-10
- Status: Implemented; validation recorded below
- Approved plan: [Documentation layout](documentation-layout-2026-09-10.md)
- Source checkpoint: `dfc115803cd42fdf1f9b7026333a1bfddc139873`
- Provenance: [Source mappings, archives, changelog ranges and frozen hashes](documentation-migration-2026-09-10.json)

## Result

The [engineering index](../README.md) now groups architecture, reference,
operations, research, plans, decisions, issues and changelog. The repository
README is 71 lines. The stable [implementation plan](../implementation-plan.md)
contains current status and next work; its completed history is archived.
The issue and decision registries retain their identities and organization.

The optional sibling [product docs](../../../docs/README.md) owns product
overview/roadmap, sales, positioning, manuscripts and publishing assets.
Forty-four existing research/publishing/sales files were copied, hash-verified
and removed from their old locations. The future repository is not initialized;
its local deployment output and secret-path ignore rules moved with it.
The code's Git history remains available at the source checkpoint.

## Reconciliation

| Source | Maintained owner | Treatment |
|---|---|---|
| Root README and draft | [Root README](../../README.md), operations, reference, examples | Keep one Native quick start and component map; configuration uses the newer runbook values, including Luna-low. API examples remain with DataOps/examples; detailed governance lives in its runbook. Both editorial sources are preserved in the sibling archive. |
| Architecture and draft | [Architecture](../architecture/README.md), [HTTP reference](../reference/http-api.md) | Preserve detailed current backend/cache/Lua contracts and route tables; adopt the draft's compact governance and verification explanations. Correct directed-construction scope and current module paths. Preserve both exact originals in the engineering archive. |
| Product overview and draft | [Product overview](../../../docs/product/overview.md) | Adopt the draft's memory-layer, governance and use-case structure, qualify unsupported immutability/read-only/local-provider claims, and link the evidence dossier instead of duplicating its scoreboard. External Chunker claims stay owned by that project. Preserve both exact originals. |
| Operations | [Operations](../operations/README.md) | Split running, configuration, authentication, CLI, jobs, recovery and governance. M22–M26 each have a focused procedure. Preserve the M24 fixture's old operations anchor with one small compatibility pointer. |
| H2 roadmap and implementation plan | [Current plan](../implementation-plan.md), [product roadmap](../../../docs/product/roadmap.md), [archive](archive/README.md) | Separate product outcomes from engineering status. Preserve exact dated histories, including the historical ticket counts and original next-step proposals. |
| Research bookmarks and agent plans | [Research ideas](../research/ideas/README.md), [plan archive](archive/README.md) | Keep unadopted ideas explicitly unadopted; retain each dated plan's original intent and the old folder's status explanation. |
| Changelog | [Individual records](../changelog/README.md) | Preserve all 90 Unreleased entries and all six release/nine historical sections as 105 records. Recover dates from headings or Git blame, retaining date-source metadata. Add this migration as a separate new record. |

The manifest pins eight exact editorial/history archives. Their original links
and reviewed source paths describe the source checkpoint, so they are exempt
from current-navigation checks. Frozen fixture/evidence content is also exempt
from rewriting. Maintained links point to their new owners. Three source links
made stale by CG-26 now show both the historical module name and its current
location without changing the original review finding.

## Workflow adjustments

`scripts/check-docs.py` checks maintained local Markdown destinations/anchors,
changelog metadata and generated-index coverage. Its default mode treats the
sibling product checkout as optional; `--include-product` validates both trees.
The existing decision-index check uses the same optional-checkout boundary.
Both checks are included in the existing manually triggered CI workflow.

The CGQL/Native skills now reference the new paths. The release skill assigns
individual records to a release and updates their generated index, keeping the
root changelog as a pointer. Existing release confirmation gates remain intact.
No broader AGENTS, hook or skill policy was changed.

Sales helpers resolve their own directory and work from another working
directory. Local verification found that the old ten-slide default omitted the
eleventh slide. The exporter now discovers the count from the HTML, accepts an
explicit positive override, prefers the available headless shell, and uses an
isolated browser profile. Deployment was not invoked.

## Verification

- All 1,092 tracked fixture/evidence files match their original SHA-256 hashes.
  All 30 moved non-Markdown, non-shell assets/build sources match their source
  hashes. Eight archived editorial/history files are exact copies.
- All 105 migrated changelog bodies match the old source after ignoring only
  rewritten link destinations. Source line ranges and hashes are recorded.
- The decision inventory retains 55 rows: 24 Active, 21 Amended, nine Research,
  one Historical. Issue identities remain CG-1–CG-40: 39 Resolved and CG-25
  Closed without change. No remaining fixture-to-doc path is missing.
- `cargo build --release -p cognigraph-server` passed. The README's Native,
  loopback, auth-disabled, embeddings-disabled configuration ran with a new
  persistent database. Health, database health, jobs health, OpenAPI and the
  collection catalog all returned HTTP 200; SIGINT exited successfully.
- The relocated paper builder ran in an isolated copy from an unrelated working
  directory. HTML was byte-identical; all 23 PDF pages had identical extracted
  text. The title and measurement pages rendered correctly on visual inspection.
- The corrected sales exporter produced all 11 pages from `/tmp`; first and
  last slides rendered correctly. Shell syntax checks passed. Existing PDFs,
  HTML assets and fonts remain byte-identical in the destination.

Final link/index, optional-checkout and validation-probe results are stored in
the [verification record](documentation-migration-2026-09-10-verification.json).
No Rust source changed, so the previous Rust suite/Clippy result is not presented
as a new full-suite run. No model calls, holdout inference, existing database
reset, remote push or publication occurred.

## Next

Review root/UI AGENTS.md, hooks, project skills and related workflows with the
user's additional ideas. This is a separate policy review; the documentation
migration does not choose those policy changes in advance.
