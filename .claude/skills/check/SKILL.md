---
name: check
description: Run CogniGraph's standard Rust formatting, strict Clippy and workspace test checks. Use for an explicit Rust validation request or /check; UI-only and documentation-only work use their own repository checks.
argument-hint: "[optional-focus]"
disable-model-invocation: true
user-invocable: true
allowed-tools: Bash
---

# Check

Run this workflow from the project root:

1. `cargo fmt --all -- --check`
2. `cargo clippy --all-targets -- -D warnings`
3. `cargo test --all`

Rules:

- Execute the commands in that order.
- Stop immediately if `cargo fmt --all -- --check` or `cargo clippy --all-targets -- -D warnings` fails, and report the failing command plus the key error output.
- If `cargo test --all` fails, report the failing tests and the most relevant error output.
- Do not change files automatically as part of `/check` unless the user explicitly asks for fixes.
- Keep the final report concise: one line per command with `PASS` or `FAIL`, then the most important failure details if any.
- If an earlier failure stops the sequence, label remaining commands `NOT RUN`.
- Honor an explicitly requested focus and label the result as partial validation.
  Completed Rust changes still require the full root Rust gates.
- When validating a change, also apply the root instructions' relevant server
  modularity and documentation checks. UI-only work follows [UI instructions](../../../ui/AGENTS.md).
- Distinguish credential-gated tests that returned early from executed service
  coverage. This suite does not by itself establish live feature verification;
  follow [root instructions](../../../AGENTS.md#feature-verification) for that.
- For push readiness, complete the [push workflow](../../../AGENTS.md#push-workflow):
  incoming PRs, the version bump and every applicable local CI check are required.
  A passing Rust suite alone does not satisfy that gate.
