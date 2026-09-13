# First Railway Community acceptance — 2026-09-12

> Public reading copy: environment-specific addresses and identifiers are omitted.
> The original is preserved in the private evidence catalog (artifact `a9cf8b9e8c99589fd1b5`).
> Dated acceptance describes that run; the shared service now follows on-demand QA.

[CG-71](CG-71.md) is resolved. One authenticated Native Community v2.7.7 writer
and the bundled React console are live at
[database.example.com](https://database.example.com).
Docker Hub publication was explicitly deferred. The separately hosted website
remained unchanged and returned HTTP 200 after the database recovery drill.

## Qualified source and delivery

| Item | Recorded identity |
|---|---|
| Source candidate | `c072a43b594ba67ea17a22b144fc80f12482f3f0`, v2.7.7 |
| Integration | [PR #8](https://github.com/cognigraphdb/cognigraph/pull/8), merged 2026-09-12 18:40:41 UTC |
| Protected main / deployed commit | `92187713786fb9256fa732da5a197586ac56315b`; identical tree to the source candidate |
| PR CI | [34710554925](https://github.com/cognigraphdb/cognigraph/actions/runs/34710554925), success |
| Main CI | [34711926177](https://github.com/cognigraphdb/cognigraph/actions/runs/34711926177), success |
| Railway service | `database`, production, `platform-id-in-private-record` |
| Final deployment | `platform-id-in-private-record`, SUCCESS |
| Final running instance | `platform-id-in-private-record` |
| Railway image manifest | `sha256:7f61af7154a67eaa27380b4628a825b358108bb05462ce35ab1c9a6ec858f651` |
| Active restored volume | `platform-id-in-private-record`, `/data`, EU West |
| Active volume instance | `platform-id-in-private-record` |

Local shared CI and the installed pre-push hook passed the final source tree:
strict Rust formatting/Clippy/tests in both editions, 72 Python tests, 161 UI
units, nine browser cases, 514 Native release checks, twelve startup rejections,
documentation/issue/decision/modularity/edition checks and advisory gates.
Both Linux/amd64 Docker editions passed API/CLI/auth/restart, packaged-console,
root-volume privilege/symlink/shutdown and Helm backup checks. GitHub separately
passed its source and packaged-edition jobs. See the [CI summaries](../evidence/engineering-cg-71-2026-09-12.md#artifact-5ca53d19bef4430e98ba)
and [local image identities](../evidence/engineering-cg-71-2026-09-12.md#artifact-cc78918990713b11c248).

The incoming PR gate reviewed #2–#7 and deferred their exact heads in
[the disposition registry](../operations/pr-dispositions.json). #3 has duplicate
CodeMirror type failures; #4 mismatches React/react-dom versions; #5 requires
quick-xml source migration. #2, #6 and #7 remain separate dependency qualification
work. None entered this candidate. Protected main requires a PR and strict
`CI required`; Railway's source trigger has Wait for CI enabled. No image-publish
step, release tag or package publication ran.

## Hosted execution

The API and console share one HTTPS origin. `PORT` and `COGNIGRAPH_PORT` are both
3000; the explicit start command is `/usr/local/bin/cognigraph-entrypoint`.
Authentication is enabled, embedding providers are disabled, CGQL source rows
are capped at 100000 and time at 5000 ms, and CGQL mutations remain disabled.
The service has one replica, no overlapping writers, no sleep, a 30-second
drain and `/health/database` readiness. [Railway operations](../operations/railway.md)
owns the configuration and credential handover.

| Check | Executed result |
|---|---|
| HTTPS | Valid TLS; HTTP redirects to HTTPS; health reports Community 2.7.7 and database connected |
| Authentication | Anonymous documents/export return 401; real Admin login succeeds |
| Browser | Login/logout, create document, direct route/reload, keyboard Query navigation and CGQL readback pass |
| API persistence | Create/update/delete and nested JSON round-trip pass; deleted probe returns 404 |
| Desktop geometry | Chromium at 1280 × 800 and 1067 × 667; no root horizontal overflow or clipped hostname |
| Process | PID 1, UID/GID 10001, zero effective capabilities, no-new-privileges enabled |
| Storage | `/data/native` owned 10001:10001 mode 0700; redb mode 0600 |
| Restart | Real hosted shutdown/reopen preserves the full application snapshot |
| Final clean state | No user collections, one Admin, zero tokens/tenants; exact clean export survives another restart |

See the [browser audit](../evidence/ui-2026-09-12-railway-community.md#artifact-e8012f0d05cc4486141b),
[process readback](../evidence/engineering-cg-71-2026-09-12.md#artifact-720f5a91f9c56fb940e6),
[first restart](../evidence/engineering-cg-71-2026-09-12.md#artifact-06cb5ec6222531c92463) and
[final runtime](../evidence/engineering-cg-71-2026-09-12.md#artifact-f8945775b8f9916eb65b).
[Final access/configuration readback](../evidence/engineering-cg-71-2026-09-12.md#artifact-3c0c6ff2bad360bf2c7b)
also confirms anonymous denial and the unchanged website deployment. The first restart
shut down at 19:30:03.804718 UTC, reopened Native at 19:30:04.454308 and listened
at 19:30:04.457321. These timestamps describe this small synthetic store, not an RTO.

## Recovery actually executed

**Application snapshot:** exported the hosted store with two synthetic documents
and the bootstrap Admin. A fresh local Community container restored the entire
snapshot through authenticated import after removing its newly generated
bootstrap Admin using a temporary restore Admin. After source-Admin login and
temporary-account removal, the full parsed export matched exactly, including
internal collections. It still matched after restart. The disposable container
and volume were removed. [Application evidence](../evidence/engineering-cg-71-2026-09-12.md#artifact-bd223ccf7bff77cee845)
and [the corrected procedure](../operations/recovery.md#exact-authenticated-restore)
retain that account-handling requirement.

**Railway volume snapshot:** stopped the original writer, created manual backup
`platform-id-in-private-record` at 19:33:52.638 UTC, restarted, and added
an `after_backup` marker. After stopping again, restored that backup to a new
volume and reviewed the exact database-only mount replacement. Applying it
deployed the same main commit. The complete export matched the pre-backup
snapshot and the later marker returned 404. [Platform evidence](../evidence/engineering-cg-71-2026-09-12.md#artifact-6f0eda56ce3e2c141219)
records the identities and successful comparison.

The source application snapshot SHA-256 was
`38cd51199e1dfab9c4caaa1e252246c53fed7a08bca235ecef9a485199152702`.
The active restored volume inherited the tested backup as
`platform-id-in-private-record`, plus daily and weekly schedules
(six and 27 days retention). Only the owned synthetic collection was then
dropped. A cold clean-baseline backup, `platform-id-in-private-record`, was
created at 19:42:34.321 UTC and catalog readback confirmed it. The service was
restarted and its complete clean export matched SHA-256
`3fc01db99b0db309cccf8311e9f322ed14544ad25619094ef618912ff4d655fc`
for the saved clean snapshot.
This final baseline was created and read back; the preceding synthetic backup
is the one whose platform restore was executed.

The original unmounted, probe-only volume `platform-id-in-private-record`
was retired after acceptance. Railway acknowledged deletion and reports
`isPendingDeletion: true`, scheduled for 2026-09-14 19:51:11 UTC. It remains in
the platform inventory during that retention window; it is not mounted to a
service. The active volume is READY and not pending deletion, with no staged
environment changes. See [backup/config readback](../evidence/engineering-cg-71-2026-09-12.md#artifact-f2ce2dc6af426a32e52e)
and [cleanup readback](../evidence/engineering-cg-71-2026-09-12.md#artifact-0fb782288076d5230047).

Snapshots contain password hashes and are intentionally not tracked. Credentials
remain only in Railway's protected variables and transient test processes. The
operator receives the username and protected-variable location, not credentials
in chat or source. Screenshots contain only synthetic data.

## Failed attempts and corrected assumptions

- v2.7.6 deployment `platform-id-in-private-record` failed at build time:
  Railway rejected Docker `VOLUME`. v2.7.7 removed it and explicitly tested mounts.
- Initial v2.7.7 deployment `platform-id-in-private-record` built and
  listened on 3000 but failed platform healthchecks. Explicitly aligning Railway
  `PORT` and the Rust listener resolved the target-port mismatch.
- Two subsequent attempts encountered Native's file lock during failed-instance
  cleanup. The exact holder was not established. A read-only diagnostic found
  the volume lock available; restored normal startup then succeeded. No file
  lock was bypassed, and no data corruption is claimed or observed. The diagnostic
  deployment did not serve HTTP and is not counted as successful acceptance.
- A naive additive restore retained the target bootstrap Admin. The temporary
  Admin procedure above corrected the exact-copy check and passed on fresh storage.
- Railway denied `workflowStatus` read access while allowing backup/restore.
  Acceptance therefore used catalog/staged-volume inspection and restored data.
  A mount commit with `skipDeploys: true` was rejected; applying the reviewed
  mount replacement with deployment enabled succeeded.

## Evidence boundary

This qualifies one small, authenticated Community installation and its tested
recovery procedures. It does not establish HA, replicas, off-site backup,
point-in-time recovery, large-volume restore performance, Enterprise hosting,
mobile/Windows rendering or complete console/backend parity. Scheduled backups
are configured; future successful scheduled runs are not inferred. Provider tests
remain credential-gated/disabled, and no model calls or sealed holdout ran.

This post-deployment record is a local follow-up to the published v2.7.7 source;
its documentation commit is not part of the live deployment or the CI runs above.
