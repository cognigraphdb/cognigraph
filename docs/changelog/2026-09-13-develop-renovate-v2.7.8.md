# Develop integration and dependency maintenance — v2.7.8

- Date: 2026-09-13
- Status: v2.7.8
- Kind: Maintenance

## Changes

[CG-73](../issues/CG-73.md) replaces Dependabot update PRs with Renovate targeting
only develop, adds CI coverage and dependency-target enforcement, and defines
protected integration and release branches. Main promotion remains separate.

Incoming PRs #2–#7 are included with React/React DOM 19.3.0 alignment, CodeMirror
view 6.43.11 deduplication, Bun 1.4.2 types, quick-xml 0.42 source migration,
tokenizers 0.23.2 and mlua 0.12.1. XML and offline tokenizer regressions cover
the changed parsing/tokenization boundaries. The ledger retains exact PR heads.

This candidate also includes the [responsive login](2026-09-13-responsive-login.md)
and previous [Railway operating evidence](2026-09-12-railway-acceptance.md).
CG-73 owns candidate verification and remote transition evidence. The production
Community database remains at v2.7.7 until a separately authorized promotion.

## Verification

Full local CI and Docker suites pass, plus optional ONNX strict Clippy/tests,
a release-binary synthetic XML parse/verification run and four offline example
executions. Fifteen Community/Enterprise browser cases include real CGQL/Lua
editor submissions. Provider/model tests remain excluded. [CG-73](../issues/CG-73.md)
records the evidence boundaries and pending remote activation.
