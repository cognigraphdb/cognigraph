# Docker image publication

The [CI workflow](../../.github/workflows/ci.yml) builds and tests both
editions. Its `publish_images` input defaults to `false`; ordinary pushes, tags,
PRs and build-only CI runs do not publish images. Publication is an explicit
manual run from `main` in `cognigraphdb/cognigraph`, after the shared CI gates.
The first publishing run succeeded on 2026-09-13: **v2.7.11** is available for
both editions. The [release receipt](docker-hub/release-2.7.11.md) records the
registry digests and independently executed checks of the published images.

## Destinations and scope

| Edition | Docker Hub image | Platforms |
|---|---|---|
| Community | `cognigraph/cognigraph:<version>` | `linux/amd64`, `linux/arm64` |
| Enterprise | `cognigraph/cognigraph-enterprise:<version>` | `linux/amd64`, `linux/arm64` |

Tags use the stable workspace version, such as `2.7.1`, without a `v` prefix.
From the first release published after 2.7.27, `<version>` is a
multi-architecture index and each member is also tagged `<version>-amd64`
and `<version>-arm64`; releases through v2.7.14 are `linux/amd64` only.
The mutable `latest` tag selects the current stable release in each repository
and is an index composed from the same per-architecture digests as its
version. Starting with v2.7.14, publication advances both aliases only after
both numbered images have verified registry digests. Pin an explicit version
or digest when a deployment must retain its selected release. No
minor-version alias is published. The
[multi-architecture decision](../decisions/decision_multi_architecture_images.md)
records the tag layout. Local Docker checks use the Docker host platform
unless `DOCKER_DEFAULT_PLATFORM` is set and qualify only that platform; CI
builds and checks both platforms natively.

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
   a release. Both repositories were created and verified public on 2026-09-13.
2. Store a dedicated Read & Write Docker Hub PAT as the GitHub Actions repository
   secret `DOCKERHUB_TOKEN`. The workflow fixes the username to `cognigraph` and
   does not require Delete permission. The initial token was configured on
   2026-09-11 and expires on **2026-12-10**; rotate it before the next publishing
   run after that date. Do not put tokens in repository files or command arguments.
3. Limit other writers to these release tags. The workflow rejects any existing
   version tag and serializes its own runs, but the remote existence check and
   upload are not an atomic registry operation. Both repositories enforce
   **Specific tags are immutable**. The required rule is
   `^[0-9]+\.[0-9]+\.[0-9]+(-(amd64|arm64))?$`, which protects every stable
   version and its per-architecture members while allowing `latest` to
   advance. The publisher rejects other settings before upload, so the owner
   must replace the earlier `^[0-9]+\.[0-9]+\.[0-9]+$` rule on both
   repositories before the first multi-architecture publication (pending as
   of 2026-09-22). The initial v2.7.11 setup used all-tag immutability; its
   sealed receipt retains that historical policy.

### Verified account setup — 2026-09-13

- [Community](https://hub.docker.com/r/cognigraph/cognigraph) and
  [Enterprise](https://hub.docker.com/r/cognigraph/cognigraph-enterprise) are
  public, categorized as **Databases & storage**, and have saved descriptions
  and edition-specific overviews. Their maintained overview text lives in
  [Community overview](docker-hub/community.md) and
  [Enterprise overview](docker-hub/enterprise.md). Update the availability
  paragraphs when publishing a new image. At the initial setup checkpoint both
  pages showed **Empty repository**; after v2.7.11 publication their saved
  overviews were updated to show the exact pull commands.
- The existing `github-actions-cognigraph-publish` PAT is Active, Read & Write,
  expires on 2026-12-10 and was unused at initial setup. `DOCKERHUB_TOKEN` is present in
  GitHub Actions, last updated on 2026-09-11. No token was created, disclosed or
  rotated during setup. The v2.7.11 publishing run subsequently authenticated
  successfully with this existing credential.
- Docker Scout analysis is enabled for Community and persisted after reload.
  Enterprise analysis is not enabled: the Personal plan's repository allowance
  is consumed by Community. No paid upgrade was made. Docker's current
  [Scout documentation](https://docs.docker.com/scout/) lists one included
  repository; recheck the account's allowance before changing this allocation.
- Initial anonymous API readback confirmed both repository identities, public visibility,
  immutable tags, categories, saved overviews and zero tags. The
  [setup receipt](docker-hub/setup-2026-09-13.json) records the readback and
  original overview hashes. The later [release receipt](docker-hub/release-2.7.11.json)
  records published tags and updated overview hashes; the setup receipt remains unchanged.

At initial account setup, main was v2.7.7 (`9218771`) and develop was v2.7.10
(`7238514`). Account setup itself changed no release branch, image tag or
deployment. The subsequent authorized v2.7.11 promotion followed the
[branch decision](../decisions/decision_develop_integration.md) and updated Railway.

The login action runs only after build/runtime checks and logs out in its post
step. Workflow permissions are limited to reading GitHub repository contents.
The PAT is scoped to this dedicated Docker account, so its account-wide Write
permission should not be reused as a general development credential.

## Approved first release — v2.7.11

The owner approved promoting the corrected v2.7.11 candidate through develop
to main, updating Railway Community, and publishing both Docker editions. All
three actions succeeded; [release acceptance](docker-hub/release-2.7.11.md)
records the exact main commit, CI runs, image digests and live checks.
[CG-74](../issues/CG-74.md) records the selector defect that blocked v2.7.10 and
the passing local and remote qualification of its replacement.
Remove unused test containers and images after their checks; preserve
persistent volumes and unrelated Docker resources.

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
and current base-image tags. It checks image metadata, platform, non-root
configuration, binary version and edition, packaged licenses, unauthenticated
rejection, authenticated insertion, edition-specific OpenAPI paths, and
HTTP/CLI CGQL reads after container restart. Each check uses an isolated
Docker volume, synthetic data and no model provider; it removes its own
containers and volumes. The check records the image IDs it ran in
`target/ci/images/<arch>.json`. CI runs the suite natively on an amd64 and an
arm64 runner; on a publishing run each leg then exports exactly the recorded
IDs with `docker save` (`python3 scripts/docker_images.py export`, refusing a
tag whose ID has changed) and uploads them as a one-day artifact.

The [v2.7.14 change record](../changelog/2026-09-13-v2-7-14.md) records the
maintained alias contract. Hosted QA remains separately opt-in.

Before an upload, the publishing helper (one `publish` job after both Docker
legs) requires a clean Actions checkout, the official repository and
manual-main trigger, an unchanged remote main head, public target repositories
and unused version tags. It loads both legs' tarballs, proves every loaded ID
equals its receipt and label metadata, reruns the runtime checks for the
images of its own host platform, then repeats the remote checks before any
upload. It tags the tested image IDs `<version>-amd64` and `<version>-arm64`
and pushes them without rebuilding; each digest is read back from Docker Hub.
Once all four match, it rechecks main and creates the `<version>` index of
each repository with `docker buildx imagetools create` from those digests,
reading back the index digest and its platform coverage. After both indexes
verify, the helper rechecks main and the previous aliases, then creates
`latest` from the same per-architecture digests and requires exact digest
equality with the version index. Main is checked again before each alias.
The Actions summary records each successful upload, registry digest, edition,
platform and source commit, including uploads preceding a later failure.

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

The repository uploads and alias updates are sequential, not atomic. No alias
advances if either numbered publication or its readback fails. If one succeeds and the
other fails, retain the successful digest and inspect Docker Hub before retrying.
An existing tag, including a per-architecture tag left by a partial run,
blocks automatic retries even when it appears to be the same candidate. Do not delete or overwrite it to force a green run. Review the partial
publication and either perform an explicitly authorized recovery of the missing
edition or prepare a newly versioned, fully verified candidate. The workflow
does not delete images, create Git tags/releases or roll back registry state.
If an alias update fails after both versions are published, retain the immutable
images and inspect each alias before an explicitly authorized repair. A partial
alias update can temporarily leave editions on different releases. Only the
serialized workflow should write these tags; checks cannot make independent
writers or cross-repository updates atomic.

The guard checks only the current version in these repositories. The normal
push/version policy still owns comparison with earlier published product
versions. Registry checks do not replace PR review, license review, full edition
conformance, image vulnerability scanning or deployment acceptance. Automated
image scanning is now part of the shared Docker suite under the
[container vulnerability policy](../decisions/decision_container_vulnerability_gate.md).
Image signing remains separate future work. Available image
fixes block the build at every severity; unfixed findings remain in its reports
and require review. A passing scan does not certify a zero-CVE image.

The implementation follows Docker's
[test-before-push guidance](https://docs.docker.com/build/ci/github-actions/test-before-push/)
and uses the documented
[Docker Hub API](https://docs.docker.com/reference/api/hub/latest/) for repository
and tag checks.
