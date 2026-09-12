# Bound asynchronous lifecycle test waits by elapsed time

- Date: 2026-09-12
- Status: v2.7.4
- Kind: Test reliability

## Change

The next [CG-69](../issues/CG-69.md) Linux run passed strict Clippy, then exposed
three Enterprise lifecycle tests whose shared helper stopped after 500 polls
spaced five milliseconds apart. A nonterminal job was treated as a test failure
after roughly 2.5 seconds plus repository-read time, without reporting its phase.

The test helper now waits within a 60-second Tokio timeout covering the reads
and sleeps. Timeout diagnostics include the last status, attempt and progress.
Terminal failures still reach the existing assertions; no test is skipped or
retried. The M21 lifecycle exercises a deliberately four-second delayed
successful evaluation, beyond the previous polling allowance. Production job
timeouts and runtime contracts are unchanged.

Qualification and the original failing run are recorded in CG-69. This is a
correction to the open CI PR; main merge, deployment and image publication
remain outside the current authorization.

## Verification

The original helper failed the deliberately delayed evaluation in a local
negative control. With the corrected helper restored, all 22 promotion tests
passed, including that case and the three GitHub failures. Formatting, module
size and documentation checks pass. The installed pre-push hook runs the full
local CI/Docker gates on the final commit; the PR owns its subsequent Linux
qualification result.
