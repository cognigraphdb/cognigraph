# Graph to Users Navigation Regression

Date: 2026-07-13  
Viewport: Codex in-app browser default, 1280 x 720 CSS pixels  
Runtime: `http://localhost:3000/` with live API at `http://localhost:3001/`

## Outcome

Fixed. Rapidly leaving Graph no longer allows G6's delayed minimap render to
run after its graph model has been destroyed.

## Steps

1. **Enter Graph — healthy when settled.** The traversal, graph canvas,
   inspector, selected path, controls, and minimap render normally. Evidence:
   `01-graph.png`.
2. **Navigate immediately to Users — failed before the fix.** The Users page
   loaded underneath Bun's runtime overlay. G6's minimap called `getData()`
   after graph teardown. Evidence: `03-rapid-users.png`.
3. **Repeat rapid Graph to Users navigation — healthy after the fix.** Ten
   consecutive rapid transitions reached the Users state and produced no
   browser warnings or errors. Evidence: `04-users-fixed.png`.
4. **Leave Graph open — healthy after the fix.** The normal graph state still
   renders and DOM inspection confirms the minimap container and canvas remain
   mounted. Evidence: `05-graph-minimap-fixed.png`.

## Root Cause And Fix

G6 5.1.1's minimap debounces its `AFTER_RENDER` handler. The previous React
cleanup waited for `graph.render()` and destroyed the graph in the promise
microtask; the queued minimap timer then ran against G6's cleared context. The
minimap delay is now explicit and destruction is deferred by 32 ms after the
render promise, allowing queued plugin work to finish first.

## Accessibility Evidence

The regression affected availability rather than semantics: the runtime overlay
blocked the Users page. After the fix, DOM snapshots again expose the Users
heading, refresh action, account region, and auth-disabled explanation. This
targeted pass did not repeat screen-reader or browser-zoom testing.
