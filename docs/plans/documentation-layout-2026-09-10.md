# Documentation organization proposal

- Date: 2026-09-10
- Status: Implemented; see the [migration report](documentation-migration-2026-09-10.md)
- Scope: Documentation structure, README reduction, changelog records and preparation for a separate product docs repository
- Follow-up: Review root/UI AGENTS.md, hooks, skills and related workflows after the documentation migration

The approved proposal below preserves the planning inventory and migration intent.
Its pre-migration observations are historical; the linked report records execution.

## Ownership

The code repository owns how CogniGraph works, how to run it, what was decided,
what was measured and what remains to implement. The sibling
`/Users/skitsanos/FTP/Products/CogniGraph/docs` directory owns product strategy,
positioning, commercial material, papers and publishing assets. The user
explicitly confirmed that papers and publishing material belong there.

This is an ownership split, not a ban on technical content in a paper. A paper
can explain the implementation and cite its measurements; the source contract,
reproducible experiment and captured evidence stay versioned with the code.
At planning time the sibling directory existed and was empty. Do not initialize or
publish its future Git repository as part of this planning step.

## Current inventory

Before this proposal, the non-ignored Markdown inventory contained 250 files: 187 under `docs`, 44
under `fixtures`, six project skills, six UI documents, two crate documents
and five root documents. Within `docs`, the organized issue and decision trees
contain 74 and 56 Markdown files respectively. There are 21 loose root files.

| Current document | Lines | Main problem |
|---|---:|---|
| Root README | 742 | Product pitch, setup, API examples, configuration and detailed milestone history all compete for attention. |
| Changelog | 2,380 | Ninety top-level Unreleased records, six release sections and additional dated history share one file. |
| Implementation plan | 1,696 | Current priorities are buried among completed milestones and repeated validation reports. |
| Operations | 1,777 | Several independently useful runbooks occupy one long page. |
| Architecture | 836 | Structural explanation, endpoint inventory, configuration and release observations overlap other references. |

The three existing draft replacements are substantive work: `README.draft.md`
(369 lines), `docs/architecture.draft.md` (552), and
`docs/product-overview.draft.md` (401). Reconcile their content; do not overwrite
them or assume the published copies are authoritative on every paragraph.

This is also a consistency problem. For example, the README still says Luna
uses reasoning `none`, while the shared provider sends `low`. The H2 roadmap
still directs readers toward tickets that have since closed. Reducing duplicated
configuration and status lists should prevent these contradictions recurring.

## Proposed engineering layout

```text
code/
  README.md                         # Short repository entry point
  AGENTS.md                         # Workflow review follows migration
  docs/
    README.md                       # Navigation and document ownership
    implementation-plan.md          # Current status and next work only
    architecture/                   # Components, storage, governance, design notes
    reference/                      # CGQL and other implementation contracts
    operations/                     # Running, configuration, auth, jobs, recovery
    dataops/                        # Keep existing task-oriented guide
    examples/                       # Keep runnable requests beside examples
    research/                       # Experiment guides, measurements, ideas
    plans/
      archive/                      # Completed plans and delivery history
    decisions/                      # Keep paths, identities and status index
    issues/                         # Keep registry and evidence structure
    changelog/                      # Indexed, individual Markdown records
  fixtures/                         # Preserve datasets and sealed experiments
  crates/                           # Keep crate-local documentation
  ui/                               # Keep UI documentation with its component
```

Each substantial folder has a short README describing its scope and linking its
documents. Use purpose-based names, not tool names such as `superpowers`. Retain
`docs/implementation-plan.md` as the current engineering plan: existing agent
instructions already use it, and it is a useful stable entry point. Move its
completed history out rather than introducing another competing status page.

## Proposed product/business layout

```text
CogniGraph/docs/                    # Future separate repository
  README.md                        # Product/business navigation
  product/
    overview.md
    roadmap.md                     # Product priorities and outcomes
  sales/                           # Decks, one-pagers, notes, assets, exporters
  research/
    semantic-neurons/               # Papers, origin notes, positioning dossier
      publish/                     # HTML/PDF build bundle, fonts, bibliography
  archive/                         # Superseded editorial drafts when needed
```

Move the Semantic Neurons manuscript and publishing bundle together, preserving
their relative layout. Its evidence dossier stays alongside the research it
supports; the product overview links to it. Avoid creating a second independent
copy of its numerical claims under `product/`.

Product roadmap material explains outcomes and priorities. Rust tasks, backend
contracts, acceptance criteria and completed milestone logs remain in the code
repository. The current H2 roadmap is mixed and must be split by content.

## Move and reconciliation map

Paths in the destination column are proposed, not completed moves.

| Current source | Destination / treatment |
|---|---|
| `README.md` + `README.draft.md` | One minimized root README; route useful detail to its owning guide and retain draft provenance until reconciliation is verified. |
| `docs/product-overview.md` + draft | Reconciled sibling `docs/product/overview.md`; preserve any still-unmerged draft material in the sibling archive. |
| `docs/semantic-neurons/` | Sibling `docs/research/semantic-neurons/`, including papers, historical notes, positioning dossier and `publish/`. |
| `docs/sales-pitch/` | Sibling `docs/sales/`, including HTML, PDF, image assets, deck notes and helper scripts. |
| `docs/architecture.md` + draft | `docs/architecture/README.md` and focused component pages; reconcile drafts before removing them. |
| `docs/native-storage-model.md` | `docs/architecture/native-storage.md`. |
| `docs/vector-sidecar-design.md` | `docs/architecture/design-notes/vector-sidecar.md`. |
| `docs/redb-primary-design.md` | `docs/architecture/design-notes/redb-primary.md`; retain its later concurrency amendment. |
| `docs/cgql-mutations-design.md` | `docs/architecture/design-notes/cgql-mutations.md`. |
| `docs/semantic-neurons-port-design.md` | `docs/architecture/design-notes/semantic-neurons-port.md`. |
| `docs/cgql-v1.md` | `docs/reference/cgql.md`; a version-neutral filename matches the document's delivered v2 coverage. |
| `docs/operations.md` | `docs/operations/README.md` plus running, configuration, authentication/tenancy, jobs, governance and recovery guides. |
| `docs/reference-repair.md` | `docs/operations/reference-repair.md`. |
| `docs/benchmarks.md` | `docs/research/benchmarks/native-backend.md`; preserve dated measurements and their conditions. |
| `docs/dailymed-pilot.md` | `docs/research/dailymed/pilot.md`, with dated result records separated if useful. |
| `docs/dailymed-clinical-reference.md` | `docs/research/dailymed/clinical-reference.md`. |
| `docs/webnlg-pilot.md` | `docs/research/webnlg/pilot.md`. |
| `docs/self-healing-experiment.md` | `docs/research/semantic-neurons/self-healing.md`. |
| `docs/generalization-assessment.md` | `docs/research/semantic-neurons/generalization.md`. |
| `docs/superpowers/graphmae.md`, `ewc.md` | `docs/research/ideas/`; explicitly unadopted research ideas. |
| `docs/superpowers/plans/` | `docs/plans/archive/`, preserving original filenames and dated intent. |
| `docs/superpowers/README.md` | Integrate its historical-status explanation into the ideas/archive indexes; preserve any unique source content. |
| `docs/quick-wins-2026-07.md` | `docs/plans/archive/quick-wins-2026-07.md`. |
| `docs/roadmap-2026-h2.md` | Product priorities to sibling `product/roadmap.md`; dated delivery/research history to code `docs/plans/archive/roadmap-2026-h2.md`; current engineering next steps to the implementation plan. |
| `docs/implementation-plan.md` | Keep path, extract completed milestones into `docs/plans/archive/`, and retain a concise current-status and next-work summary. |
| `design-qa.md` | `ui/audit/design-qa.md`, following applicable UI instructions at migration time. |
| `docs/issues/`, `docs/decisions/` | Keep structure and identities; update maintained navigation links where destinations move. |
| `docs/dataops/`, `docs/examples/` | Keep structure; link detailed contracts rather than duplicate them. |
| `fixtures/`, crate READMEs, existing UI docs | Keep component placement; sealed packages and historical evidence stay unchanged. |

An old design note with a current amendment is not automatically obsolete.
Preserve its status and make its relationship to the current architecture clear
before classifying any content as archival.

## Changelog record convention

Use `docs/changelog/README.md` as an index, with one Markdown file per logical
change record. A record can cover both CG-39 and CG-40 because their combined
policy change and measurement form one coherent delivery.

```text
docs/changelog/
  README.md
  2026-09-09-directed-extraction-contracts.md
  2026-09-09-luna-document-baseline.md
  2026-09-09-luna-low-runtime.md
  2026-07-07-v2.5.0.md
  ...
```

New records contain a title, date, release association or `Unreleased`, change
type, optional issue links, a short explanation of resulting behavior, relevant
compatibility notes and a link to validation evidence. They summarize the change;
the issue owns the defect and its resolution, and the decision owns rationale.

Split the existing ninety Unreleased records at their actual logical boundaries.
Recover dates from their explicit evidence or Git history; do not stamp all of
them with the migration date. Preserve the six historical release sections as
release records and the remaining dated sections as historical records. Keep an
explicit undated historical record where a source date cannot be established.
Do not infer that a record shipped merely because it mentions a current version.

The index groups Unreleased changes and released/history records without copying
their bodies. At release time, associate selected records with the release and
generate its summary from those records. A small root `CHANGELOG.md` navigation
pointer can remain during the transition; it must not retain a parallel log.

## Root README budget and content

Target roughly 60–100 lines, organized around a first successful local run:

1. Name and one paragraph describing the Rust service.
2. A few capability bullets, with key supported-scope distinctions.
3. Prerequisites and one Native local quick start with a health check.
4. A compact map of the server, crates, UI, fixtures and documentation.
5. Links to setup/operations, architecture, CGQL, DataOps, examples, issues,
   decisions, changelog and product material.
6. License.

Move the full environment table to the configuration reference, HTTP examples
to the existing examples folder, and governance details to architecture and
operations. The root README should not carry per-milestone narratives, benchmark
scoreboards or sales arguments. The architecture and product drafts provide
useful source material for their own destinations; even the 369-line README
draft is larger than the intended entry point.

## Links, provenance and workflow dependencies

Keep local relative links within each repository. Cross-repository evidence
references should identify the code revision and path; once repository URLs are
established, use durable revision-specific URLs for published evidence. Until
then, document the sibling-checkout convention in the two indexes. Do not invent
a future remote URL or require the business checkout to build/test the code.

Do not globally replace paths inside frozen experiment packages, captured
requests/responses, evidence JSON, source manifests or historical source quotes.
Their hashes and original checkpoint meaning remain intact. Update maintained
Markdown navigation separately. A scan found one fixture Markdown reference to
the operations page; retain a small compatibility pointer there if needed.
Keep compatibility pages only for demonstrated consumers, not a duplicate tree.

The paper builder locates its Markdown source relative to its own file, so
moving the complete bundle preserves that relationship. Sales export/deploy
helpers instead contain `docs/sales-pitch` paths and need mechanical relocation
updates. Verify local builds without invoking deployment.

The release skill explicitly writes `CHANGELOG.md`; the Native development skill
references `docs/architecture.md`. AGENTS.md and the CGQL skill reference the
implementation plan, whose path is retained. The decision-index and server-size
checks depend on the existing decisions/issues paths, also retained. Make only
necessary path/record-format corrections during migration; reserve broader
AGENTS, hook, skill and policy redesign for the user's subsequent review.

## Migration sequence and completion checks

1. Establish a separate checkpoint for the completed CG-39/CG-40 work before
   mixing in bulk moves. It is still uncommitted; do not stage user drafts into
   that checkpoint by accident. This proposal does not create a commit.
2. Record source-to-destination mappings and hashes, reconcile the three drafts,
   then transfer business/publishing bundles with verified copies before source
   removal. Record original revision plus working-file digest for uncommitted
   material. Preserve old Git history; repository-history extraction can be a
   separate future decision.
3. Group engineering files, split the long current guides from historical logs,
   minimize the README and create the changelog records/index. Keep only one
   maintained owner for each contract, status and measurement.
4. Update links and necessary tooling paths; validate the new layout. Then
   perform the separate AGENTS/UI AGENTS, hooks and skills review requested by
   the user, incorporating their additional ideas.

Completion means no unaccounted-for source content or assets; reconciled drafts;
valid maintained links/anchors; unchanged frozen package hashes; all decisions
still indexed; all issue identities intact; README setup exercised against the
real server; relocated publication/export assets checked locally; and the
changelog workflow producing a valid record without recreating the old monolith.
Historical claims retain their dates and caveats. No new model calls or holdout
execution are necessary for documentation organization.

Only this proposal and its implementation-plan link are written in the planning
step. Other documentation, assets, source code, agent rules and workflow scripts
have not been moved or rewritten by this step.
