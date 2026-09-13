---
name: cognigraph-ui-qa
description: Implement or review CogniGraph console UI changes with browser evidence for visual consistency, responsive layout, persisted actions, query results, and access boundaries. Use for work in ui/ that changes screens or interactions, or an explicit UI/UX audit.
---

# CogniGraph UI QA

Read [root instructions](../../../AGENTS.md), [UI instructions](../../../ui/AGENTS.md)
and the affected items in [UI TODO](../../../ui/TODO.md). Paths in commands below
are relative to the code repository unless a different working directory is stated.

## Scope and preparation

1. Identify the requested route, interaction and acceptance criteria. For an audit,
   inspect and report before changing application code; an implementation request
   includes fixes needed for its acceptance. A small change needs its affected
   states, while a flow-level audit covers the complete relevant journey.
2. Inspect the current screen and surrounding components before editing. Capture
   a baseline for visual changes. Use the current theme and shared classes from
   UI instructions, plus any user-selected reference. Symphony's brand, typography,
   button sizes and workspace classes are not CogniGraph design requirements.
3. Read only the relevant checks in [console flows](references/console-flows.md).
   Use [browser evidence](references/browser-evidence.md) for viewport, geometry,
   accessibility and report requirements. These checks are acceptance targets,
   not claims that every existing screen already passes.

## Runtime verification

1. Reuse a suitable local instance or start the UI with `bun run dev` from `ui/`.
   Start the real API with the configuration documented in the
   [operations guide](../../../docs/operations/README.md), using a disposable
   Native database for mutation checks. Record UI/API origins, backend and actor
   role/tenant without credentials. Inspect existing processes before starting
   another server; retain data and processes that belong to the user.
2. Exercise the affected route through the available browser tooling. Verify the
   primary action and relevant empty, loading, error and denied states. A toast
   is feedback: confirm the server result, visible state, and hard-reload state
   after persisted changes. Keep transient editor/dialog state distinct from
   values promised to persist.
3. For routing, asset or API-origin changes, also build the UI and test the Rust
   server's `COGNIGRAPH_UI_DIST` mode. Check direct URLs and refresh at a configured
   origin, including a non-default API port when origin handling changes. Do not
   infer production behavior from the Bun dev server or a saved browser override.
4. For UI source, dependencies or build changes, finish from `ui/` with:

   ```sh
   bun run check
   bun test
   bun run build
   ```

   Use the root Rust gates when Rust changes. Instruction-only edits use the root
   documentation checks. A successful build does not substitute for browser
   evidence; report unavailable runtime checks as untested.
5. Put raw reports and screenshots in a dated private evidence package or an
   explicit external capture directory. Publish a reviewed summary in the issue,
   changelog or `docs/reviews/ui/`, with private path/hash references where needed.
   Follow the [evidence policy](../../../docs/operations/evidence-policy.md).
   Register verified defects with the next unused CG number in the
   [shared registry](../../../docs/issues/README.md); link the report from the
   relevant ticket or changelog. Mark only executed acceptance criteria complete.
   Identify and remove disposable records created for the run where supported.

This workflow adapts Symphony's measured layout, reload verification and scoped
evidence practices. Its application-specific governance and build tooling are
replaced by CogniGraph's current contracts.
