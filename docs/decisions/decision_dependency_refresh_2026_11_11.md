# Dependency refresh, Tantivy 0.27 and the Unicode pin — 2026-11-11

- Date: 2026-11-11
- Status: Active
- Scope: [CG-192](../issues/CG-192.md), with the owner's decisions of 2026-11-11

## Decision

Refresh all Cargo and Bun dependencies under the
[develop qualification policy](decision_develop_qualification.md). The owner
approved three breaking upgrades and one dated pin.

1. **Breaking upgrades.** `lru` 0.18 to 0.19 (the query cache), `serde-saphyr`
   1.3 to 2.1.1 (the server build script and the OpenAPI drift tests) and
   Tantivy 0.26.2 to 0.27.0. None needed a source change. Tantivy 0.27.0
   requires `lru ^0.18.2`, so two `lru` release lines resolve side by side;
   both carry the soundness fix that [CG-70](../issues/CG-70.md) was about.
2. **The vendored Tantivy snapshot is retired.** It existed only to correct
   Tantivy's `lru` requirement. Tantivy 0.27.0 publishes the corrected
   requirement, which is the removal condition the vendor guide set. The
   snapshot, its provenance record, the Cargo patch declaration and workspace
   exclusion, the Docker copy, the packaged Tantivy licence file, the
   integrity check and its test are removed. Tantivy is now an ordinary
   registry dependency like every other one, so its licence is no longer
   copied into the image separately. `vendor/README.md` stays as a short note
   so existing links resolve and a future patch has stated conditions.
3. **Text indexes are rebuilt, not migrated.** The text index is a derivative
   of the store. Stores written by the previous binary are reopened with the
   candidate and searched; the ticket records the result.
4. **`unicode-normalization` is pinned at 0.1.25.** Release 0.1.26 moves to
   Unicode 18.0.0. The M23 preparation contract freezes Unicode 17.0.0 for
   normalization and whitespace, a compile-time guard enforces it, and Rust's
   own character tables are still Unicode 17. The six manifests require
   `=0.1.25`, and `docs/operations/dependency-exceptions.json` carries an
   exception that expires on 2027-02-11. [CG-194](../issues/CG-194.md) tracks
   the migration.
5. **Biome 2.6 does not format Markdown here.** The new release formats
   Markdown by default. The console and the TypeScript client exclude `*.md`
   from Biome, which keeps the behaviour of the previous release.
6. **Rejected: keeping the vendored snapshot with an exception.** It would
   leave 375 third-party files and a custom integrity gate in place after the
   reason for them ended.
7. **Rejected: moving to Unicode 18 now.** Normalization and whitespace
   classification would use different Unicode versions, and existing
   prepared-corpus digests would stop reproducing.

## Evidence and outcome

[CG-192](../issues/CG-192.md) owns the executed checks and their limits. This
amends the version-specific parts of the
[CG-70 dependency decision](decision_dependency_advisories.md) and of the
[2026-09-14 refresh decision](decision_dependency_refresh_2026_09_14.md); the
advisory gate and the maintenance-review requirements stay in force.
