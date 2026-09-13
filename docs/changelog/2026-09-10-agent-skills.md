# UI QA skill and workflow revision

- Date: 2026-09-10
- Status: Unreleased
- Kind: Documentation

## Change

Adapt Symphony's `symphony-ui-qa` skill and its four flow references, inspected
on 2026-09-10, into the [CogniGraph UI QA skill](../../.agents/skills/cognigraph-ui-qa/SKILL.md).
Reuse verification after reload, executed-query provenance, measured layout at
effective desktop scaling, keyboard checks and scoped evidence. Retain CogniGraph's
theme, Bun commands, API/session boundaries, data safety and current capabilities.
Symphony branding, Modern.js/Taskfile commands and medical/governance flows are not
dependencies of this skill. Symphony's source files were only read.

Keep the procedure in `.agents/skills/` with a small Claude discovery entry point;
link it from root/UI instructions. Extend documentation validation to check links
inside the new skill and its references.

Review all six existing project skills:

- [check](../../.claude/skills/check/SKILL.md): narrow discovery to Rust validation,
  preserve explicit invocation and report skipped commands and service coverage.
- [review-changes](../../.claude/skills/review-changes/SKILL.md): distinguish the
  reviewed commit from a dirty checkout; remove foreign API/dependency assumptions,
  mandatory named-agent delegation and automatic personal-memory updates.
- [release workflow](../../.claude/skills/release-workflow/SKILL.md): discover actual
  branches/protection and manual CI; remove the unsupported develop/release-workflow
  claims, push-before-confirmation sequence and hard-reset recovery. Preserve
  release decisions while honoring authorization already given. Validate the final
  versioned candidate and stage new change records explicitly.
- [CGQL](../../.claude/skills/cgql-dev/SKILL.md),
  [Native](../../.claude/skills/native-backend-dev/SKILL.md) and
  [backend contract](../../.claude/skills/backend-contract/SKILL.md): retain their
  recently corrected implementation boundaries, fix argument hints to YAML strings,
  and include affected query/Lua checks in contract work.

Hooks and runtime configuration remain for the subsequent review.

## Validation

Skill validation passed for the canonical UI skill, Claude entry point and the
two rewritten review/release procedures. YAML checks passed for all seven Claude
entry points, including string argument hints and the preserved explicit-only
`/check` policy; the new Codex interface metadata also passed. The validator's
PyYAML dependency ran in an isolated `uv` environment without adding project dependencies.

Documentation/changelog and decision-index checks passed, as did `git diff --check`.
A temporary skill probe verified `.agents/skills/` discovery, acceptance of a valid
link, and rejection of both a missing file and a missing Markdown anchor; the
probe was removed afterward.

Documented UI commands were checked against `ui/package.json`; branch and workflow
assumptions were checked against local refs and tracked CI configuration. Remote
branch protection and release permissions were not queried and remain release-time
checks. This change did not execute a UI audit, Rust release or provider benchmark;
no application suites were rerun for instruction-only edits.
