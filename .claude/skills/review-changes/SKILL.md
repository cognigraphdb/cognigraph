---
name: review-changes
description: Review incoming merged CogniGraph commits or PRs since a known checkpoint and explain their effect on current work. Use for catching up on teammate changes or merged dependency updates; local uncommitted reviews and merging open PRs are separate tasks.
---

# Review incoming changes

Review what landed, its rationale and its impact on current work. Keep an audit
read-only unless fixes or repository updates are requested. Follow
[root instructions](../../../AGENTS.md) and relevant nested instructions.

## Establish the checkpoint

1. Inspect `git status --short --branch`, local refs and configured remotes.
   Preserve uncommitted work. Fetch the relevant remote when the task needs
   current incoming changes; fetching does not authorize pulling or merging.
2. Resolve the requested baseline and target to commit IDs. Use the user's named
   checkpoint when provided. Otherwise derive a defensible range from local
   history/tracking state and state that assumption. Explain an empty range or
   diverged history; do not invent a previous review checkpoint.
3. Read the diffstat, commit bodies and affected files at that target. Use
   `git show <target>:<path>` when the target differs from the working tree.
   Group large diffs by subsystem without requiring a particular agent tool or model.

## Verify behavior and rationale

- Match changes to the [CG issue registry](../../../docs/issues/README.md),
  [decisions](../../../docs/decisions/README.md) and linked PRs where available.
  GitHub tooling can supply PR bodies and CI evidence; report unavailable remote
  evidence honestly. Verify descriptions against the implementation and tests.
- Trace changed contracts through affected crates and consumers: `GraphBackend`,
  CGQL, HTTP/OpenAPI, Lua, persistent formats and UI API calls as applicable.
  Identify effects on uncommitted work without rewriting it during review.
- For dependency updates, inspect manifest/lockfile changes, upstream release
  notes and affected APIs. Run representative checks; do not carry over another
  project's dependency exceptions or merge a bump merely because it is newer.

## Verification and handoff

Validate the reviewed target before calling it a green checkpoint. If it differs
from the active checkout, use an isolated checkout when validation is in scope;
do not test the current dirty tree and attribute its result to another commit.
For Rust checkpoints use the [check procedure](../check/SKILL.md); use UI and
documentation checks for their respective changes. Apply root live-verification
requirements before claiming affected behavior works, and distinguish tests
that returned early from executed service coverage.

Report the reviewed commit range, verified fixes, remaining findings with file
references, contract/dependency impacts and validation limitations. Where the
user requested a registry/status update, retain issue resolution evidence and
update the owning documents. Persistent personal memory updates require an
explicit user request. Review findings alone do not authorize merging, pushing
or publishing changes.

For work being prepared for a push, also follow the root
[push workflow](../../../AGENTS.md#push-workflow). This incoming-commit review
does not replace its fresh open-PR check, dispositions, version bump or final
local CI checks after integration.
