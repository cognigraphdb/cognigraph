# Engineering documentation

Start with the [implementation plan](implementation-plan.md) for current status
and next work. The [issue registry](issues/README.md) owns defect resolution;
[decisions](decisions/README.md) own rationale and contract amendments.

Native is the only runtime storage backend. The [Native-only batch](plans/native-only-2026-09-12.md)
is complete through CG-65, CG-67 and CG-68. [CG-71's live acceptance](issues/railway-community-2026-09-12.md)
qualifies a dated Railway Community deployment with the console. The shared
database now follows the [on-demand QA lifecycle](decisions/decision_hosted_qa_lifecycle.md)
and stays stopped between explicitly requested hosted checks; ordinary builds
and CI do not test Railway. Applications use their own instances. Use the
[Railway operating guide](operations/railway.md) for that installation and the
[first-deployment guide](operations/first-deployment.md) for another environment.
[v2.7.11 release acceptance](operations/docker-hub/release-2.7.11.md) records
the first published Docker images in both editions and the Railway update,
including hosted phone-login checks. [CG-74](issues/CG-74.md) is resolved. CG-64/CG-66
optional importer work remains deferred.

| Area | Read it for |
|---|---|
| [Architecture](architecture/README.md) | Components, storage, cache and governance boundaries |
| [Reference](reference/README.md) | CGQL and the HTTP contract |
| [Evidence catalog](evidence/README.md) | Private artifact identities and public reading copies |
| [Operations](operations/README.md) | Running, configuration, authentication, jobs and recovery |
| [DataOps](dataops/README.md) | Preparing, ingesting, querying and reviewing data |
| [Examples](examples/README.md) | HTTP, Lua and request templates |
| [Research](research/README.md) | Reproducible experiments, dated measurements and unadopted ideas |
| [Plans](plans/README.md) | Current plans and archived delivery history |
| [Changelog](changelog/README.md) | Individual change and release records |
| [Console review](evidence/ui-2026-09-11-full-review.md#artifact-2a08bd45b55e9bd0cb9c) | Current UI defects, executed journeys and backend coverage gaps |

## Ownership and checkout layout

The code repository owns executable contracts, decisions, issues, operational
guides, public verification inputs and reviewed research summaries. Raw captures
and operational inventories live in the optional private `../evidence/` checkout.
Product strategy, positioning, sales, papers
and publishing assets live in the [sibling product docs](../../docs/README.md).
Local cross-repository links assume sibling directories named `code/` and `docs/`
under the product root. Both repositories are published under the [cognigraphdb](https://github.com/cognigraphdb) GitHub organization; the code repository is `cognigraphdb/cognigraph`.
It is optional for builds and code checks; `--include-product` enables validation
of both checkouts when present. Published citations should use a pinned code
revision at that URL.

## Maintenance

Put each contract or measurement in one owning guide and link to it. Preserve
dated claims and sealed fixture bytes. Quoted historical paths describe their
source revision; archived reading-copy links use validated current navigation.
Exact originals use the [hash-bound preservation policy](plans/archive-navigation-2026-09-13.md).
Record a new logical change
under `changelog/` and update its index. Validate navigation and the record index:

```sh
python3 scripts/check-docs.py
python3 scripts/check-decision-index.py
```

Commands in engineering guides run from the code repository root unless stated
otherwise. The [migration report](plans/documentation-migration-2026-09-10.md)
records the layout change and source provenance.
