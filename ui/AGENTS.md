# CogniGraph UI Instructions

These instructions apply to `ui/` and extend the [root instructions](../AGENTS.md).
Read the [UI tracker](TODO.md) for planned work and the
[management UI decision](../docs/decisions/decision_management_ui.md) for design
rationale and dated runtime findings.

For UI implementation or a UI/UX audit, follow the
[CogniGraph UI QA skill](../.agents/skills/cognigraph-ui-qa/SKILL.md). It provides
scoped console journeys, measured layout checks and a browser-evidence format;
use the sections relevant to the requested change.

## Development And Verification

- Use Bun for package management, the native HTML development server and the
  production bundler. Do not add Vite. Commands below run from `ui/` and are
  defined in [package.json](package.json).
- For initial setup, use `bun install --frozen-lockfile`. Preserve `bun.lock`
  during routine work. When dependencies change, prefer current stable releases,
  review compatibility, update the lockfile and verify the affected behavior.
- Start development with `bun run dev`. For UI implementation or runtime
  verification, run the required local UI/API servers yourself and open the
  browser preview. Reuse a suitable existing server when available; do not hand
  server-start instructions to the user when you can perform the work.
- For UI source, dependency or build changes, finish with:

```sh
bun run check
bun test
bun run build
```

- `check` runs Biome and TypeScript. `bun run check:fix` applies Biome fixes;
  inspect its diff and rerun the checks. Preserve unrelated work.
- Verify changed interactions against the real API in the browser, including
  relevant loading, empty, error and authorization states. Check browser console
  and network errors, keyboard operation and affected responsive layouts.
- When changing routing, assets or API-origin configuration, also verify the
  production build served by Rust with `COGNIGRAPH_UI_DIST`: direct page loads,
  refreshes, login redirects and API calls must work at the configured origin.
- Record scoped browser evidence under `ui/audit/` and summarize the result in
  the relevant issue or changelog. State which environments and flows were
  actually checked; dated audit results do not verify a newer build.
- Instruction/documentation-only edits use the root documentation checks.
  They do not require starting servers or rerunning the UI/Rust suites unless
  runnable behavior changes. Changes to Rust also require the root Rust gates.

## Visual Direction And Components

- Use [BrandMark](src/components/BrandMark.tsx) for product branding. Its bundled
  [SVG](src/assets/cognigraph-mark.svg) preserves the path from the
  [product master](../../docs/graphics/cognigraph-mark.svg), with transparent
  margins cropped in the UI copy. Color it through CSS for its surface; keep
  the mark centered when the sidebar wordmark is hidden. Builds must not depend
  on the sibling product-docs checkout.
- Preserve the restrained database-console layout: dark sidebar, light document
  table and right-side JSON inspector. Use the [approved Ant Design audit](audit/antd-final/audit.md)
  and [collection reference](audit/antd-final/02-collections.png) for historical
  visual context; the current [theme](src/antd-theme.ts) and [styles](src/styles/)
  define the implemented tokens and layout.
- Before substantial visual changes with an unclear source, use Product Design's
  `get-context` workflow when available. Otherwise inspect the current screen,
  relevant references and theme, and clarify only missing goals or design input.
  Do not block routine work on an unavailable plugin.
- When the user selects a mock, use it as the visual reference for layout,
  component anatomy, density, spacing, typography and hierarchy. Keep displayed
  data and available actions consistent with actual server capabilities.
  Record durable UI-specific design decisions in this `ui/AGENTS.md`.
- Reuse Ant Design management controls, Phosphor icons and
  [CogniGraphProvider](src/components/CogniGraphProvider.tsx). Keep CodeMirror for
  editors and AntV G6 for the graph canvas; follow their existing integrations.
- Reuse shared CSS tokens and base classes, including `--workspace-pad`,
  `workspace-header-base`, `empty-state-base`, `actions-row-base` and `chip-base`.
  Use [ErrorAlert](src/components/ErrorAlert.tsx) for inline errors and the
  existing `notify()` interface for toasts.
- Follow the root modularity convention: target coherent source/test files of
  roughly 300–400 lines and split by responsibility, not an arbitrary line count.

## API, Sessions And Routing

- Use the [central API client](src/api/client.ts). It adds `/api` to application
  paths; operational routes such as `/health` and `/metrics` remain at the root.
  Follow the [OpenAPI contract](../crates/cognigraph-server/openapi.yaml).
- Build screens around functional endpoints. Show role, tenant and backend
  limitations honestly; hidden or disabled controls do not replace server-side
  authorization. Do not present mock success as a completed server operation.
- Keep the API URL, bearer token and session identity scoped to the browser
  session through the existing `sessionStorage` handling in [App.tsx](src/App.tsx).
  Clear authentication state on logout and a token-bearing request's 401.
  Do not put credentials in URLs, logs, source, `localStorage` or repository files.
- Give each page a copyable React Router path. Use `NavLink`/`useNavigate`, with
  safe navigation identifiers and filters in path/query parameters so direct
  links, refresh and back/forward navigation work. Keep sensitive payloads and
  transient form/dialog state out of URLs; component state is appropriate for
  transient UI state.
- Snapshot import/restore must remain disabled until the flow provides a file
  preview, impact summary and explicit user confirmation, as required by the
  management UI decision. Preserve confirmation for existing destructive actions.

## Compatibility Workarounds

These preserve behavior observed in the
[July runtime findings](../docs/decisions/decision_management_ui.md#addendum-2026-07-15-api-namespace-sessions-routing-self-hosting).
Treat them as compatibility constraints; remove one only after reproducing its
scenario successfully on the replacement dependency/bundler versions.

- Mount/unmount Ant Design Modals conditionally (`{open ? <Dialog/> : null}`
  with constant `open` inside the dialog). Toggling `open` on a mounted Modal
  previously hung transitions in the Bun dev bundle. Preserve the theme's
  disabled motion and related CSS guards until their regressions are verified.
- Do not combine a custom Button `icon` with Ant Design's `loading` prop;
  that combination triggered bundled-icon warnings. Swap the icon manually,
  for example `icon={busy ? <CircleNotch/> : <RealIcon/>}`.

## Tracking

- Keep planned UI work in [TODO.md](TODO.md). Record defects in the shared
  [CG issue registry](../docs/issues/README.md) and link them from the tracker
  when relevant; avoid a second independent defect list.
- Tick completed items in the commit that lands the work. Git blame identifies
  that commit; add inline hashes only when referring to already-existing commits.
  Do not try to embed a commit's own hash in itself.
