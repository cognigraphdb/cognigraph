# Native-only product and website alignment

Date: 2026-09-12. Completed locally after the
[CG-68 readiness checkpoint](../issues/native-readiness-2026-09-12.md).
Deployment is on hold at the owner's request. This follow-up does not alter
the checkpoint's sealed captures, hashes or measured outcomes.

## Current contract and copy

The sibling product-docs checkout now describes Native as the only storage
engine in both editions. Its product overview and architecture list twelve
Rust crates. Presenter notes, two editable sales decks and the active
Semantic Neurons research index use the same storage boundary. The research
positioning dossier receives a dated amendment; its measurements are unchanged.

The sibling website's `/source`, `/docs` and `/from-aql` pages now follow the
same contract. Its crate map matches the twelve members in `Cargo.toml` and
contains no adapter entry. Website positioning and language-brief guidance
point authors to the current migration contract.

External AQL comparisons remain useful migration teaching. The
[AQL-to-CGQL guide](../reference/aql-to-cgql.md) owns that contract;
[direct dump import](arangodump-import-design.md) remains deferred optional
work. No separate A9 source was found in these checkouts, so no independent
A9 module is claimed as updated.

Historical papers, archived drafts, benchmark captures and the website's sealed
query results retain their original context. Existing owner edits to product
copy, read-replica scope, graphics and website QA history are preserved.

## Local verification

- Website `bun install --frozen-lockfile`: no dependency changes.
- Website `bun run check` on Bun 1.4.2: TypeScript, Biome and 37 tests passed
  with 1,109 assertions.
- Production SSR over loopback HTTP: GET and HEAD returned 200 for all three
  affected pages. Rendered output contains no retired adapter claims.
- Chromium 153.0.8010.12: all three pages checked at 1440, 390 and 320 pixels.
  The twelve crate links exactly match the workspace; no horizontal page
  overflow or browser errors occurred. The AQL page's CGQL copy action returned
  the exact displayed query at every width. Screenshots were visually reviewed.
- Three changed deck slides were rendered and visually reviewed at 1920×1080.
  Text stayed within each slide, with no horizontal overflow. This covers the
  edited slides, not a new audit of every sales claim or a PDF export.

Website evidence stays in its owning checkout's ignored local directory
`design/qa/native-only-2026-09-12/`, indexed from `design-qa.md`. Sales evidence
stays in the separate product-docs checkout.

Documentation validation passed: `scripts/check-docs.py` with and without
`--include-product` (440/419 documents, no errors),
`scripts/check-decision-index.py` (57 decisions) and
`scripts/issue.py check` (68 issues). No Rust code or build inputs changed in this alignment;
the Rust, Docker and Helm results remain those of the CG-68 checkpoint.

## Delivery boundary

No commit, push, tag, version increment, image publication or live deployment
was performed for this alignment. Website changes are a local candidate and
must be coordinated with code publication so its GitHub references describe
the published workspace. Use the [push workflow](../operations/push.md) when
publication is authorized. The [first-deployment guide](../operations/first-deployment.md)
applies only when deployment resumes.
