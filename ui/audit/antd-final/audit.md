# Ant Design Migration Audit

Date: 2026-07-13  
Viewport: Codex in-app browser default, 1280 x 720 CSS pixels  
Runtime: `http://localhost:3000/` with live API at `http://localhost:3001/`

## Outcome

Passed. Ant Design 6.5.1 now supplies the appropriate stateful management
controls while the CogniGraph shell, CodeMirror editor, JSON presentation, and
AntV G6 canvas retain the approved visual identity. Fresh-session browser logs
were clean after the final fixes.

## Page Walkthrough

1. **Overview — healthy.** URL validation rejects incomplete server addresses;
   refresh and connection loading states remain explicit. Evidence:
   `01-overview.png`.
2. **Collections — healthy.** Search, selects, row selection, inspector tabs,
   compact paging, create validation, and destructive dialog behavior were
   exercised. A temporary document was created and deleted through the live API;
   invalid creation stays in the modal with field feedback and no unhandled
   rejection. Evidence: `02-collections.png`.
3. **Query — healthy.** CodeMirror remains the CGQL authority-facing editor;
   `Control+Enter` executed the live query and returned thirteen documents.
   Result gutters remain visible. Evidence: `03-query.png`.
4. **Graph — healthy.** Initial traversal, explicit rerun, neighborhood
   expansion, selected-path state, and Visual/JSON switching were exercised.
   The prior G6 minimap `getData` race no longer reproduces. Evidence:
   `04-graph.png`.
5. **Users — healthy, capability disabled.** The live server's auth-disabled
   state is presented as an intentional, actionable empty state. The current
   Ant Design Spin API is used. Evidence: `05-users.png`.
6. **Operations — healthy.** Cache statistics load into the result panel and
   cache clearing opens an explicit confirmation that was cancelled during QA.
   Snapshot import remains honestly disabled. Evidence: `06-operations.png`.

## Findings Closed

- P0: G6 minimap crashed after replacing graph data during traversal reruns.
- P1: Bun rendered Ant Design's CommonJS icon definitions incorrectly in
  transient controls; Phosphor overrides now cover those states.
- P1: rejected create validation surfaced as an unhandled promise.
- P2: deprecated `Spin.tip` emitted a runtime warning.

## Accessibility Evidence

DOM snapshots confirmed labelled primary navigation, headings, form labels,
dialog names, table semantics, tab roles, graph application/toolbar labels,
and disabled states. Keyboard execution was tested in CodeMirror. This pass did
not include screen-reader announcement testing, browser zoom testing, or a
mobile breakpoint because those require separate target contexts.
