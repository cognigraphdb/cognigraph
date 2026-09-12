# Docker image publication

The [CI workflow](../../.github/workflows/ci.yml) builds and tests both
editions. Its `publish_images` input defaults to `false`; ordinary pushes, tags,
PRs and build-only CI runs do not publish images. Publication is an explicit
manual run from `main` in `cognigraphdb/cognigraph`, after the shared CI gates.
The first publishing run has not been executed. Continue to use the
[source-build quick start](../../README.md#five-minute-start) until images exist.

## Destinations and scope

| Edition | Docker Hub image | Platform |
|---|---|---|
| Community | `cognigraph/cognigraph:<version>` | `linux/amd64` |
| Enterprise | `cognigraph/cognigraph-enterprise:<version>` | `linux/amd64` |

Tags use the stable workspace version, such as `2.7.1`, without a `v` prefix.
No `latest`, minor-version alias or ARM build is published.
This avoids silently changing a deployment's selected version. Local Docker
checks use the host platform unless `DOCKER_DEFAULT_PLATFORM` is set; ARM host
success alone is not amd64 acceptance. CI explicitly builds and checks amd64.

Both images include the server, CLI and license files under
`/usr/share/licenses/cognigraph/`. OCI labels identify the source repository,
workspace version and commit; `io.cognigraph.edition` identifies the build.
Community and Enterprise retain their respective [license terms](../../LICENSING.md).
A publicly downloadable Enterprise image does not grant unrestricted production
use or change the commercial license's evaluation terms.

## One-time account setup

Use the personal Docker Hub account **`cognigraph`**. The GitHub organization
is separately named **`cognigraphdb`**; it is not the image namespace.

1. Create public Docker Hub repositories named `cognigraph` and
   `cognigraph-enterprise` under the `cognigraph` account. The workflow refuses
   missing, private or mismatched repositories rather than creating them during
   a release. On 2026-09-11 neither repository was publicly available.
2. Store a dedicated Read & Write Docker Hub PAT as the GitHub Actions repository
   secret `DOCKERHUB_TOKEN`. The workflow fixes the username to `cognigraph` and
   does not require Delete permission. The initial token was configured on
   2026-09-11 and expires on **2026-12-10**; rotate it before the next publishing
   run after that date. Do not put tokens in repository files or command arguments.
3. Limit other writers to these release tags. The workflow rejects any existing
   version tag and serializes its own runs, but the remote existence check and
   upload are not an atomic registry operation. Account-level tag immutability,
   when available, adds enforcement against concurrent external publishers.

The login action runs only after build/runtime checks and logs out in its post
step. Workflow permissions are limited to reading GitHub repository contents.
The PAT is scoped to this dedicated Docker account, so its account-wide Write
permission should not be reused as a general development credential.

## Verify and publish

Prepare and push the candidate through the [push workflow](push.md), including
the required version bump, incoming-PR review and local checks. Publishing
images is a separate authorized action. The version must agree with local
Cargo.lock packages and a versioned changelog record.

```sh
# Local equivalents; Docker also runs packaged-server HTTP probes.
python3 scripts/verify.py --suite ci
python3 scripts/verify.py --suite docker

# Remote build-only verification (the default).
gh workflow run ci.yml --repo cognigraphdb/cognigraph --ref main

# Only after image publication is authorized and the account setup is complete.
gh workflow run ci.yml --repo cognigraphdb/cognigraph --ref main -f publish_images=true
```

The Docker suite builds both editions using the committed dependency lockfile
and current base-image tags. It checks image metadata, non-root configuration,
binary version and edition, packaged licenses, unauthenticated rejection,
authenticated insertion, edition-specific OpenAPI paths, and HTTP/CLI CGQL reads
after container restart. Each check uses an isolated Docker volume,
synthetic data and no model provider; it removes its own containers and volumes.

Before an upload, the publishing helper requires a clean Actions checkout,
the official repository and manual-main trigger, an unchanged remote main head,
public target repositories and unused version tags. It checks both images again,
then repeats the remote checks before any upload. It tags the tested image IDs
and pushes them without rebuilding. The Actions summary records each accepted
image reference, registry digest, edition and source commit.

Inspect the completed run and its logs. Build success alone is not publication
success, and local checks do not establish a remote CI result. For deployment,
prefer the recorded digest. For Helm, explicitly set `edition`,
`image.repository` to the selected repository above and `image.tag` to the exact
version. The chart's local-build defaults do not select these new repositories.

## Failures and retries

An unavailable registry or GitHub check fails the run; a network/authentication
error is never treated as an unused tag. If main advances during verification,
dispatch a new run for the current candidate. No image is uploaded until both
editions pass the runtime checks.

The two repository uploads are sequential, not atomic. If one succeeds and the
other fails, retain the successful digest and inspect Docker Hub before retrying.
An existing tag blocks automatic retries even when it appears to be the same
candidate. Do not delete or overwrite it to force a green run. Review the partial
publication and either perform an explicitly authorized recovery of the missing
edition or prepare a newly versioned, fully verified candidate. The workflow
does not delete images, create Git tags/releases or roll back registry state.

The guard checks only the current version in these repositories. The normal
push/version policy still owns comparison with earlier published product
versions. Registry checks do not replace PR review, license review, full edition
conformance, image vulnerability scanning or deployment acceptance. Automated
image scanning/signing and native ARM publication are separate future work.

The implementation follows Docker's
[test-before-push guidance](https://docs.docker.com/build/ci/github-actions/test-before-push/)
and uses the documented
[Docker Hub API](https://docs.docker.com/reference/api/hub/latest/) for repository
and tag checks.
