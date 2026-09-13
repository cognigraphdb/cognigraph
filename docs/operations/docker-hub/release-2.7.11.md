# v2.7.11 — first Docker images and Railway promotion

> Public reading copy: environment-specific addresses and identifiers are omitted.
> The original is preserved in the private evidence catalog (artifact `3ea4e46e673552a8b22e`).
> Dated acceptance describes that run; the shared service now follows on-demand QA.

Accepted on 2026-09-13. Both public Linux/amd64 images are published, and the
authenticated Railway Community instance runs v2.7.11 with its existing data.
The [machine-readable receipt](release-2.7.11.json) records source identity,
CI, registry readback, runtime checks, overview hashes and cleanup.

## Source and qualification

[PR #13](https://github.com/cognigraphdb/cognigraph/pull/13) resolves
[CG-74](../../issues/CG-74.md), the transient login-card selector race that
blocked v2.7.10. [PR #12](https://github.com/cognigraphdb/cognigraph/pull/12)
promotes the corrected v2.7.11 candidate from develop to main.
Main is `4a960357bd8902b15a1a1782dd0961c4932543f5`; its source tree
`1804d51c8fbe6274bf5d02c19d34ea3d829f8481` equals the locally qualified
correction commit `31f50f20e103adb16b2653b37cb034704918060a`.

Full local CI, both Linux/amd64 images and Helm backup checks passed, including
the installed pre-push hook on the committed candidate. The correction PR,
develop push, promotion PR, [main push](https://github.com/cognigraphdb/cognigraph/actions/runs/34755282374)
and [publishing run](https://github.com/cognigraphdb/cognigraph/actions/runs/34755296051)
all passed required CI. Coverage includes both strict Rust editions, 514 Native
release checks, twelve startup rejections, fifteen browser cases and both
packaged editions. Twenty repeated logout journeys additionally qualified the
selector repair. Historical failed v2.7.10 evidence remains unchanged.

## Published images

| Edition | Immutable tag | Registry digest |
|---|---|---|
| [Community](https://hub.docker.com/r/cognigraph/cognigraph) | `cognigraph/cognigraph:2.7.11` | `sha256:c0ccd1f5f1661190fab4185025bf9ee61cbc33acf601be836f01720735c30a6c` |
| [Enterprise](https://hub.docker.com/r/cognigraph/cognigraph-enterprise) | `cognigraph/cognigraph-enterprise:2.7.11` | `sha256:a28e782cd78f21f90b9a6f3518815e4dd8de4e6e1d30df6fe885f1f047a9b9a0` |

The existing publisher PAT successfully authenticated in GitHub Actions.
Both images passed packaged runtime checks before upload. Anonymous Docker Hub
readback matched each workflow-reported digest, public visibility, immutable-tag
settings and Linux/amd64 platform. Each digest was then independently pulled
locally and checked for matching OCI version, edition and source revision.
Both passed real container authentication, edition-specific API, CLI, packaged
license, restart and persisted CGQL checks. This is verified publication and
runtime evidence, not merely a successful image build.

Both saved public overviews now show the exact pull command. Community Scout
analysis remains enabled; Enterprise is outside the Personal plan's allocated
repository allowance. No paid upgrade, credential rotation, `latest` alias,
ARM image, Git tag, GitHub release or crates.io publication was performed.
No clean vulnerability-scan result is claimed by this runtime acceptance.

## Railway acceptance

Deployment `platform-id-in-private-record` reports SUCCESS for the exact
main commit above. The existing main source trigger waited for CI before
building and deploying. HTTPS health reports Community v2.7.11, readiness
reports a connected database, and anonymous API/export requests return 401.
Admin authentication and a synthetic create/read/patch/CGQL/delete journey pass.

An authenticated application export saved privately before promotion equals
the export after deployment. After removing the probe collection, another
export still equals that baseline exactly. No real documents, users, secrets
or database volumes were reset or replaced. One replica, zero overlapping
writers and `/data` persistence remain configured. SSH readback confirms PID 1
runs as UID/GID 10001 with zero effective capabilities and `NoNewPrivs: 1`;
the Native directory/file permissions remain 0700/0600.

The [hosted login audit](../../evidence/ui-2026-09-13-railway-v2.7.11.md#artifact-28071583ad3d71443c64)
passes narrow portrait, landscape, short-screen validation/scrolling and desktop
checks. This is Chromium viewport emulation, not a physical phone or software
keyboard test. The earlier [CG-71 recovery qualification](../../issues/railway-community-2026-09-12.md)
is preserved; this release did not repeat the platform restore drill.

## Cleanup and evidence boundary

The local qualification images `cognigraph:ci` and `cognigraph:ci-enterprise`
were removed after their checks. The two pulled published image references were
also removed after acceptance. Test runners removed their own containers and
anonymous probe volumes. Unrelated images, containers and persistent volumes
were preserved. Only main/develop branches and the primary worktree remain.

The existing CG-70 advisory maintenance exception is unchanged. No model-provider
or holdout execution is claimed. This report was prepared after the immutable
v2.7.11 candidate; publishing the report itself belongs to a later versioned
documentation change set.
