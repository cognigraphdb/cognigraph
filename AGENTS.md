# Project Instructions

These instructions apply to the whole code repository. Read any applicable
nested instructions before editing a component, including [ui/AGENTS.md](ui/AGENTS.md).

## Communication

- Always answer in English.
- Keep explanations direct and focused on the current task.

## Repository Discipline

- Treat the worktree as shared. Do not revert or overwrite changes you did not make unless the user explicitly asks.
- Do not edit secrets or local environment files directly: `.env`, `.env.*`, `secrets/`, or `.git/`.
- `.env.example` may be edited when documenting configuration.
- Prefer `rg` for searching text and files.

## Documentation And Evidence

- Start with [the engineering index](docs/README.md) and
  [the implementation plan](docs/implementation-plan.md). Update the plan when
  project direction, feature status, or next work changes. Archived plans
  describe their historical checkpoints, not the current backlog.
- Use synthetic organizations in regression tests. Do not copy client-specific
  research or publishing assets into code fixtures; keep them outside this repository.
- Keep runtime contracts, operational guides, decisions, issues and reproducible
  evidence with the code. Product strategy, sales, positioning, papers and
  publishing assets belong in the optional sibling `../docs/` directory.
- Keep [issue identifiers and statuses](docs/issues/README.md) stable. Create a
  new issue only with `python3 scripts/issue.py new "Title" --priority P2 --area "..."`,
  which allocates the next identifier from the working tree, index, committed history and the
  registry counter, refuses to overwrite an existing file, and adds the registry
  row. Never write `docs/issues/CG-{number}.md` by hand or reuse an identifier:
  `python3 scripts/issue.py check` (part of the CI suite) fails when a committed
  issue's original title changes or a file and its registry row disagree. Retain
  resolution evidence.
- Use [decision records](docs/decisions/README.md) for rationale and contract
  amendments. Add a logical change record under [docs/changelog/](docs/changelog/README.md)
  and regenerate its index with `python3 scripts/check-docs.py --write-changelog-index`.
  The root `CHANGELOG.md` remains a navigation pointer.
- Link to the owning guide rather than duplicating configuration, model choices,
  measurements or status lists in entry points and agent instructions.
- Preserve sealed captures, manifests, source hashes and historical results.
  Put new measurements and protocol/scorer amendments in new versioned records
  or packages; do not rewrite frozen evidence to match newer code or paths.
- Follow each experiment's protocol and split boundaries. Holdout execution is
  a qualification step, not routine validation. Report the corpus, model, policy
  and scoring units that bound a measured claim.

## Query And Backend Direction

- Native is the only runtime storage backend. Follow the
  [storage decision](docs/decisions/decision_native_only.md) and
  [batch plan](docs/plans/native-only-2026-09-12.md). The pre-deployment cleanup
  is complete. [CG-68](docs/issues/CG-68.md) records local readiness and
  [CG-71](docs/issues/CG-71.md) records the first live Community deployment.
  Follow the [first-deployment guide](docs/operations/first-deployment.md)
  and requalify changed runtime/build inputs before deployment. CG-64/CG-66
  are optional deferred importer work.
  Preserve useful `GraphBackend` contracts, Native storage modes and explicit
  capabilities, including guarded and tenant-scoped wrappers.
- Public HTTP and Lua query text uses parsed CGQL. Backend language declarations
  and caller roles must never enable opaque query passthrough.
- Keep `graph.query()` as the Lua query entry point and preserve the separate
  authorization of HTTP read and mutation surfaces.
- Keep grammar, validation, planning and executor ownership in `cognigraph-query`;
  update Native, server and Lua integration where the requested behavior requires it.
- CGQL keywords are case-insensitive; identifiers remain case-sensitive.
  Multiple `FOR` clauses, joins and read subqueries are already implemented.
  Follow the [current specification](docs/reference/cgql.md) and its tested
  restrictions when extending them; update the contract and plan when scope changes.

## Rust Workflow

- Follow the existing crate layout and local patterns before adding new abstractions.
- Keep changes scoped to the requested behavior.
- Follow the [file modularity convention](docs/decisions/decision_file_modularity.md):
  target coherent source/test modules of 300–400 lines, with the documented soft
  cap and exceptions. Avoid mechanical splits that obscure a logical unit.
- For Rust changes, finish with:

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all
```

- If formatting fails because code needs rewriting, run `cargo fmt --all`, then rerun the validation commands.
- When changing the governed server modules covered by the size guard, also run
  `python3 scripts/check-server-modularity.py`. Its coverage and exception budgets
  are documented in the modularity decision; it is not a whole-workspace size check.

## Documentation Validation

The checks below cover ordinary editing. The [push workflow](#push-workflow)
requires the full applicable CI checks before publishing, including docs-only changes.

For documentation, navigation or agent-workflow changes, run from the code root:

```bash
python3 scripts/check-docs.py
python3 scripts/check-decision-index.py
```

- When changing cross-repository links or product docs, also run
  `python3 scripts/check-docs.py --include-product` with the sibling checkout present.
  The default code checks must work without that checkout.
- For documentation-only changes, use these checks and any relevant example,
  script or publishing validation. A changed runnable example needs a real run;
  a wording or navigation edit does not require the full Rust suite.

## Feature Verification

- Unit and route tests are necessary but not sufficient. After implementing a feature, live test it first — run the real binary and exercise the feature end to end (e.g. a release-build server over real HTTP with the relevant env/config) — and only then declare it done or suggest next steps and possible directions.
- Prefer disposable databases and isolated local configuration for verification.
  Preserve existing data unless the user has authorized the required reset or repair.
- Distinguish unit/route tests, executed live integration checks, and checks that
  skipped or returned early because credentials or services were unavailable.
  An early-returning integration test is not executed backend coverage.
- Report failures and untested boundaries honestly. Record verification in the
  relevant issue report, decision or changelog record; distinguish local results
  from remote CI, release and deployment evidence.

## Push Workflow

Before every authorized push, complete these steps in order:

Install the tracked hook once per checkout with `python3 scripts/install-hooks.py`.
The [push guide](docs/operations/push.md) explains its checks and limitations.
Run `python3 scripts/verify.py --suite ci` for the same checks as CI and
`python3 scripts/verify.py --suite docker` for its image build. Do not bypass a
failing hook with `--no-verify` or disable it to publish an unchecked candidate.

1. Fetch the relevant remote and inspect incoming commits and all open PRs,
   including their target branches, review state and checks. Review and process
   incoming work within the user's authorized scope. Record each PR's disposition
   (included, needs changes, superseded or explicitly deferred); resolve outstanding
   inclusion decisions before proceeding. Verify the combined code after integration.
   An unavailable PR listing is an incomplete gate, not evidence of no incoming work.
2. Bump the product version for every outgoing change set, including documentation
   and maintenance pushes. Use `[workspace.package].version` in `Cargo.toml`, keep
   inherited member versions/internal requirements consistent, refresh `Cargo.lock`
   and record the version in `docs/changelog/`. Use a patch increment for routine
   compatible fixes/docs; features and breaking changes require the appropriate
   semantic version increment. Honor any exact version already chosen by the user.
   The [pre-deployment Native-only decision](docs/decisions/decision_native_only.md)
   explicitly permits breaking cleanup without a mandatory major jump solely
   for Arango removal; the per-push version increment and verification still apply.
   Compare with the latest published product version, not only the last local tag.
   Branch and release-tag refs publishing the same candidate share one version;
   a retry of that unchanged candidate does not require another increment.
3. Inspect the current [CI workflows](.github/workflows/) and run every applicable
   check locally against the final integrated, versioned candidate. This gate
   applies to PR branches and even to docs-only pushes. The shared runners below
   are authoritative; [CI setup](docs/operations/ci.md) owns tool prerequisites. Currently:

   ```sh
   cargo fmt --all -- --check
   python3 -m unittest discover -s scripts/tests
   python3 scripts/check-server-modularity.py
   python3 scripts/check-docs.py
   python3 scripts/check-decision-index.py
   python3 scripts/issue.py check
   cargo clippy --locked --all-targets -- -D warnings
   cargo test --locked --all
   python3 scripts/check-editions.py
   cargo clippy --locked --all-targets --features enterprise -- -D warnings
   cargo test --locked --all --features enterprise
   python3 scripts/verify.py --suite ui
   python3 scripts/verify.py --suite ui-browser
   python3 scripts/verify.py --suite native
   python3 scripts/verify.py --suite helm
   python3 scripts/verify.py --suite advisories
   actionlint
   ```

   For every outgoing branch candidate, also run the CI Docker build and Helm backups:
   `python3 scripts/verify.py --suite docker` (Community and Enterprise). Follow current workflow conditions and toolchain
   requirements if they change. Run the relevant UI gates and real-binary/browser
   regression checks for affected behavior. A failed or unavailable required check
   blocks the push; report intentional CI service skips separately from executed
   live coverage. Local success does not establish a remote CI result.

   The shared CI suite includes both UI suites above on every run. Follow the
   [UI testing guide](docs/operations/ui-testing.md) for Bun/Chromium setup and
   the browser suite's disposable Community/Enterprise scope.
4. Re-check incoming PRs and the target remote head immediately before pushing.
   If new incoming work changes the candidate, process it and repeat the checks
   affected by that change. Report the final version, commit, PR dispositions and
   validation results; push only the authorized refs. An uncertain push result
   requires checking remote state before retrying.

For explicitly owner-authorized removal of private material from history, use
the separate manifest-bound procedure in the [push guide](docs/operations/push.md#owner-authorized-data-removal).
It retains candidate verification and remote-drift checks; ordinary pushes
remain subject to the fast-forward and immutable-tag rules.

This is standing authorization to prepare the required version bump as part of
an authorized push. Commit, remote merge, push, tagging and package publication
still follow the user's requested scope; this rule does not initiate them by itself.

## Shared Agent Workflows

Project workflow procedures live in `.claude/skills/`:

- [/check](.claude/skills/check/SKILL.md) runs the standard Rust validation workflow.
- [/review-changes](.claude/skills/review-changes/SKILL.md) reviews incoming merged work, not local uncommitted changes.
- [/rust-release-workflow](.claude/skills/release-workflow/SKILL.md) handles release preparation and must respect its confirmation gates.
- [/cgql-dev](.claude/skills/cgql-dev/SKILL.md) guides CGQL grammar, validation, planner, executor, and documentation work.
- [/native-backend-dev](.claude/skills/native-backend-dev/SKILL.md) guides Native storage, traversal, vector search, and query routing work.
- [/backend-contract](.claude/skills/backend-contract/SKILL.md) checks shared `GraphBackend` semantics across backend implementations.

The [cognigraph-ui-qa](.agents/skills/cognigraph-ui-qa/SKILL.md) skill lives in
`.agents/skills/` for Codex discovery. Its
[Claude entry point](.claude/skills/cognigraph-ui-qa/SKILL.md) links the same
procedure. Use it for UI implementation or an explicit UI/UX audit, with only
the flow checks relevant to the request. Maintain one canonical procedure.

When the user requests a matching workflow, read its linked `SKILL.md` and follow
the applicable procedure, including its release gates. Agents without slash-command
integration follow the procedure manually. User instructions and applicable
repository instructions take precedence over conflicting skill guidance.
