# Engineering documentation

Start with the [implementation plan](implementation-plan.md) for current status
and next work. The [issue registry](issues/README.md) owns defect resolution;
[decisions](decisions/README.md) own rationale and contract amendments.

| Area | Read it for |
|---|---|
| [Architecture](architecture/README.md) | Components, storage, cache and governance boundaries |
| [Reference](reference/README.md) | CGQL and the HTTP contract |
| [Operations](operations/README.md) | Running, configuration, authentication, jobs and recovery |
| [DataOps](dataops/README.md) | Preparing, ingesting, querying and reviewing data |
| [Examples](examples/README.md) | HTTP, Lua and request templates |
| [Research](research/README.md) | Reproducible experiments, dated measurements and unadopted ideas |
| [Plans](plans/README.md) | Current plans and archived delivery history |
| [Changelog](changelog/README.md) | Individual change and release records |

## Ownership and checkout layout

The code repository owns executable contracts, decisions, issues, operational
guides and reproducible evidence. Product strategy, positioning, sales, papers
and publishing assets live in the [sibling product docs](../../docs/README.md).
Local cross-repository links assume sibling directories named `code/` and `docs/`
under the product root. Both repositories are published under the [cognigraphdb](https://github.com/cognigraphdb) GitHub organization; the code repository is `cognigraphdb/cognigraph`.
It is optional for builds and code checks; `--include-product` enables validation
of both checkouts when present. Published citations should use a pinned code
revision at that URL.

## Maintenance

Put each contract or measurement in one owning guide and link to it. Preserve
dated claims and sealed fixture bytes. Archived source paths describe their
original revision; they are not current navigation. Record a new logical change
under `changelog/` and update its index. Validate navigation and the record index:

```sh
python3 scripts/check-docs.py
python3 scripts/check-decision-index.py
```

Commands in engineering guides run from the code repository root unless stated
otherwise. The [migration report](plans/documentation-migration-2026-09-10.md)
records the layout change and source provenance.
