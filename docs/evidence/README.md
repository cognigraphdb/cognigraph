# Evidence catalog

Public engineering records retain decisions, methods, dated results and known
limitations. Raw research runs, provider exchanges, labelled evaluation
corpora, UI captures and environment inventories live in the private
`cognigraphdb/cognigraph-evidence` repository. Its access is independent
of the public code repository.

[catalog.json](catalog.json) maps each artifact to its original path, private
archive path, byte length and SHA-256. Package pages provide stable links.
Authorized reviewers resolve the archive path in the private repository;
others can read public conclusions but cannot independently replay those
private runs from this checkout alone. A digest proves byte identity when
compared with the artifact; it is not a signature, timestamp or validation
of the measurement.

Original bytes, source attribution and licences are preserved privately.
Public reading copies have amended navigation and an explicit provenance
notice; they do not replace sealed originals. Historical paths in quoted
commands describe the original experiment layout, not current public files.
No model or holdout runs were performed for this migration.

Current-tree removal does not remove old Git objects or previously made
public copies. Historical publication requires a separate reviewed operation.

Later [archive navigation amendments](../plans/archive-navigation-2026-09-13.md)
preserve their original plan bytes in a separate dated private package, with
public path/hash references. They do not rewrite this migration's sealed archive.

## Packages

- [engineering-arango](engineering-arango.md)
- [engineering-cg-70-2026-09-12](engineering-cg-70-2026-09-12.md)
- [engineering-cg-71-2026-09-12](engineering-cg-71-2026-09-12.md)
- [engineering-historical-checks](engineering-historical-checks.md)
- [engineering-luna-low-runtime-live-2026-09-09](engineering-luna-low-runtime-live-2026-09-09.md)
- [engineering-native-readiness-2026-09-12](engineering-native-readiness-2026-09-12.md)
- [engineering-native-runtime-2026-09-12](engineering-native-runtime-2026-09-12.md)
- [operations](operations.md)
- [research-blind](research-blind.md)
- [research-cross-provider-baseline-2026-09-09](research-cross-provider-baseline-2026-09-09.md)
- [research-cuad-2026-07-22](research-cuad-2026-07-22.md)
- [research-dailymed](research-dailymed.md)
- [research-draft-eval-results-2026-07-06.md](research-draft-eval-results-2026-07-06.md.md)
- [research-glm-luna-semeval-2026-09-09](research-glm-luna-semeval-2026-09-09.md)
- [research-judge-injection](research-judge-injection.md)
- [research-luna-baseline-2026-09-09](research-luna-baseline-2026-09-09.md)
- [research-luna-directed-v2-2026-09-09](research-luna-directed-v2-2026-09-09.md)
- [research-luna-documents-2026-09-09](research-luna-documents-2026-09-09.md)
- [research-model-matrix-gpt54-results-2026-07-07.md](research-model-matrix-gpt54-results-2026-07-07.md.md)
- [research-real-label-eval-admin-2026-07-21.json](research-real-label-eval-admin-2026-07-21.json.md)
- [research-real-label-eval-clinical-2026-07-21.json](research-real-label-eval-clinical-2026-07-21.json.md)
- [research-real-label-head-to-head-2026-07-21.log](research-real-label-head-to-head-2026-07-21.log.md)
- [research-sideviews-model-comparison-2026-09-08](research-sideviews-model-comparison-2026-09-08.md)
- [research-terra-luna-low-semeval-2026-09-09](research-terra-luna-low-semeval-2026-09-09.md)
- [research-vector-control-results-2026-07-07.md](research-vector-control-results-2026-07-07.md.md)
- [ui-2026-09-10-collections](ui-2026-09-10-collections.md)
- [ui-2026-09-10-logo](ui-2026-09-10-logo.md)
- [ui-2026-09-11-capabilities](ui-2026-09-11-capabilities.md)
- [ui-2026-09-11-collection-search-scope](ui-2026-09-11-collection-search-scope.md)
- [ui-2026-09-11-data-preservation](ui-2026-09-11-data-preservation.md)
- [ui-2026-09-11-execution-guards](ui-2026-09-11-execution-guards.md)
- [ui-2026-09-11-full-review](ui-2026-09-11-full-review.md)
- [ui-2026-09-11-logo-refresh](ui-2026-09-11-logo-refresh.md)
- [ui-2026-09-11-production-origin](ui-2026-09-11-production-origin.md)
- [ui-2026-09-11-provisioning](ui-2026-09-11-provisioning.md)
- [ui-2026-09-11-result-ownership](ui-2026-09-11-result-ownership.md)
- [ui-2026-09-11-review-paging](ui-2026-09-11-review-paging.md)
- [ui-2026-09-11-router-update](ui-2026-09-11-router-update.md)
- [ui-2026-09-11-sidebar-names](ui-2026-09-11-sidebar-names.md)
- [ui-2026-09-11-space-guidance](ui-2026-09-11-space-guidance.md)
- [ui-2026-09-11-table-keyboard](ui-2026-09-11-table-keyboard.md)
- [ui-2026-09-11-tenant-deletion](ui-2026-09-11-tenant-deletion.md)
- [ui-2026-09-11-ui-ci](ui-2026-09-11-ui-ci.md)
- [ui-2026-09-12-native-readiness](ui-2026-09-12-native-readiness.md)
- [ui-2026-09-12-native-runtime](ui-2026-09-12-native-runtime.md)
- [ui-2026-09-12-packaged-console](ui-2026-09-12-packaged-console.md)
- [ui-2026-09-12-railway-community](ui-2026-09-12-railway-community.md)
- [ui-2026-09-13-dependency-maintenance](ui-2026-09-13-dependency-maintenance.md)
- [ui-2026-09-13-login-card-selection](ui-2026-09-13-login-card-selection.md)
- [ui-2026-09-13-login-centering](ui-2026-09-13-login-centering.md)
- [ui-2026-09-13-login-ci-wait](ui-2026-09-13-login-ci-wait.md)
- [ui-2026-09-13-login-responsive](ui-2026-09-13-login-responsive.md)
- [ui-2026-09-13-railway-v2.7.11](ui-2026-09-13-railway-v2.7.11.md)
- [ui-antd-baseline](ui-antd-baseline.md)
- [ui-antd-final](ui-antd-final.md)
- [ui-design](ui-design.md)
- [ui-graph-users-regression](ui-graph-users-regression.md)
- [ui-overview](ui-overview.md)
