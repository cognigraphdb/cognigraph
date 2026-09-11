# Local verification and push gates

The [root push policy](../../AGENTS.md#push-workflow) applies to every outgoing
product change set. CI and local checks use the same runner:

```sh
python3 scripts/verify.py --suite ci
python3 scripts/verify.py --suite docker
python3 scripts/verify.py --suite ui
python3 scripts/install-hooks.py
```

The installer uses Git configuration to select tracked `.githooks/`; it refuses
to replace a different hook path or existing active hooks. Install it separately
in each clone. The CI suite includes workflow regression tests, formatting, server
modularity, documentation/index and issue-history checks, edition dependency checks, strict Clippy and all workspace tests for both Community
and Enterprise feature sets.
The Docker suite builds the same Community and Enterprise images as CI and runs
isolated packaged-server HTTP, authentication, edition and restart/persistence checks.
CI image publication is a separate opt-in step described in
[Docker image publication](docker-publishing.md). The UI suite uses the frozen Bun
lockfile, Biome/TypeScript, tests and production build.

## What pre-push verifies

- The remote is a configured GitHub remote and every outgoing branch/tag resolves
  to the checked-out HEAD. The worktree, including nonignored untracked files, is
  clean. Existing branch updates must be fast-forward.
- The stable workspace version exceeds the remote target/default branch version
  for new changes, agrees with local packages in Cargo.lock, and has a versioned
  change record. Branch/tag refs for the same candidate share the version. Tags
  are annotated, named for the version and cannot replace a published tag.
- The current open PR list is fetched in full. A PR is accounted for when its head
  is an ancestor of the candidate. If a reviewed PR is superseded or explicitly
  deferred, record its exact head and rationale in `docs/operations/pr-dispositions.json`:

  ```json
  [{"number": 123, "head": "<full head SHA>", "disposition": "deferred",
    "reason": "Why it is excluded", "decision": "Reference to the user's deferral decision"}]
  ```

  Updating this file is not permission to invent a deferral. A changed PR head
  requires review again. Missing GitHub access blocks the gate.
- The CI suite runs on every push; a branch push to `main` also runs the Docker
  suite. Changes under `ui/` relative to the remote baseline add the UI suite.
  Relevant real HTTP/browser tests are still required by the feature workflow;
  command success alone cannot prove those acceptance criteria.
- After validation, the checkout, remote refs and open PR heads must remain
  unchanged. Resolve changes before retrying. A failed or unavailable required
  check blocks the push. Intentional credential-gated test skips remain visible.

Use `git push --dry-run origin main` to exercise the real installed gate without
publishing. It performs network reads and the full local checks. The hook does not
merge PRs, bump versions, create commits/tags or push refs itself.

This is a developer workflow guard, not a security boundary against someone
changing Git configuration. It does not install server-side branch protection,
guarantee remote CI success, or cover an unrelated product-documentation repository.
Deletion and prerelease version schemes need their own reviewed workflow.
Initial publication uses the explicit procedure below. Never bypass checks to
work around a failure.

## Claude edit protection

The Claude PreToolUse hook rejects malformed edit payloads, protected path
components and symlinks resolving to protected targets. `.env.example` remains
allowed only outside protected directories. The hook does not inspect file
contents or mediate arbitrary shell commands; repository instructions still
apply to every editing tool, including Codex.

The issue guard needs full Git history (CI uses `fetch-depth: 0`); shallow clones
must fetch the remaining history before verification. For Helm changes, also
run `python3 scripts/check-helm.py --live` and repeat with `--enterprise` after
building `cognigraph:ci` and `cognigraph:ci-enterprise`.
Its Helm/Bun rendering and Docker/HTTP checks do not contact a Kubernetes cluster.

## Owner-authorized data removal

Ordinary publication never rewrites existing branches or tags. For an explicitly
requested data-removal operation, `COGNIGRAPH_HISTORY_REMOVAL_MANIFEST` selects
the separate [manifest-bound gate](../../scripts/history_rewrite.py). The manifest
and scan receipt stay outside the repository and bind every old/new branch or
tag object, the checked main candidate and the complete remote-ref snapshot.
The receipt must cover the outgoing tips and report no remaining matches or
unexpected tree changes. The gate still runs CI and Docker, requires a patch
version increment, and blocks on incoming PRs or changed refs. Send the complete
transaction atomically with explicit force-with-lease values; do not use
`--no-verify`. Never use this mode for ordinary feature publication.
See the [data-removal boundary](repository-data-removal.md).

## Initial publication to a fresh repository

When the owner explicitly requests a fresh repository with no inherited history,
prepare a reviewed snapshot as a single root commit. Preserve pending work and
recovery backups outside the repository first. Do not publish old branches or
tags, merge old ancestry into the new main, or use a mirror push.

`COGNIGRAPH_INITIAL_PUBLICATION_MANIFEST` selects the
[initial-publication gate](../../scripts/initial_publication.py). Its value is an
absolute path to an external JSON manifest containing `operation` set to
`initial-publication`, the exact `candidate` commit, `repository` as `owner/name`,
the numeric GitHub `repository_id`, the reviewed boolean `private` visibility,
and `previous_version` from the previous published product. Review the destination
identity, distribution permissions and previous version directly on GitHub before
preparing this manifest; a reused repository name alone is not enough to identify
a new repository. A visibility change requires a new distribution review.

The gate accepts only creation of `refs/heads/main` at that root commit. It
requires an empty destination with no open PRs, a newer version, matching lockfile
and changelog, and passing CI, both Docker builds and UI checks. It rechecks the
candidate, manifest, repository identity and visibility, empty refs and incoming PRs immediately
before publication. Push only `HEAD:refs/heads/main`, with automatic tag following
disabled. Ordinary subsequent pushes use the normal gate without this variable.

A fresh repository does not inherit the archive's PRs, settings or history. The
archive and local recovery copies still retain their own history; this operation
does not delete them or certify erasure from GitHub storage. Historical hashes
and PR numbers in dated engineering records refer to the former repository and
are retained as provenance, not as navigable refs in the fresh repository.
