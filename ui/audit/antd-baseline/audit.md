# Ant Design migration baseline audit

Date: 2026-07-13

Scope: Combined UX and accessibility baseline for the six primary management
pages at the in-app browser's default 1280 x 720 viewport. Screenshots in this
folder were captured and visually inspected during the same audit run.

## Overall strengths

- The navigation shell, page hierarchy, typography, density, and restrained
  teal identity are consistent and should remain custom CogniGraph UI.
- Every primary navigation item and core field has an accessible name.
- Empty, unavailable, loading, and selected states are generally honest about
  the server contract.

## Baseline steps and findings

1. **Overview — healthy with form-state gaps** (`01-overview.png`)
   - Status hierarchy and connection context are clear.
   - Connection fields rely on native required validation and have no inline
     URL rule, validating state, or structured error recovery.
2. **Collections — usable but interaction-heavy** (`02-collections.png`)
   - Table/inspector relationship is clear and appropriately dense.
   - Search, selects, pagination, tabs, create/delete dialogs, JSON validation,
     loading, and copy feedback are all hand-built and behave inconsistently.
3. **Query — strong specialist surface** (`03-query.png`)
   - CodeMirror syntax, diagnostics, and result pairing should remain custom.
   - Bind-variable validation and run/history controls need a consistent form
     and feedback layer around the editor.
4. **Graph — blocked by runtime error** (`04-graph.png`)
   - The circular graph, inspector, and selected-path hierarchy are strong.
   - G6 minimap rendering throws `Cannot read properties of undefined (reading
     'getData')`, producing Bun's runtime overlay after traversal. This is a P0
     functional blocker for the final audit.
5. **Users — honest unavailable state** (`05-users.png`)
   - The auth-disabled explanation is clear.
   - Loading, error, empty, and populated states should use one accessible
     result/table pattern with consistent status announcements.
6. **Operations — clear actions with unsafe confirmation primitive**
   (`06-operations.png`)
   - Operational grouping and destructive-action distinction are clear.
   - Cache clearing uses `window.confirm`, which is visually inconsistent and
     cannot provide contextual loading or structured confirmation copy.

## Accessibility limits

Screenshots establish visible hierarchy, labels, target presentation, and focus
styling risks, but not full WCAG conformance. The final audit must additionally
exercise keyboard focus, dialog focus trapping and restoration, validation
announcements, listbox interaction, loading states, and destructive-action
confirmation behavior in the browser.
