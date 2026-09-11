# Browser evidence

Use the subset affected by the task. A shared shell, typography or component
change needs representative consumers; a local copy edit does not require every
route or destructive workflow.

## Layout and interaction

- Compare a normal desktop viewport with effective 125% and 150% desktop scaling.
  Derive CSS dimensions from the target physical display divided by scale. For
  example, 1600 × 1000 yields 1280 × 800 and approximately 1067 × 667. Record the
  actual CSS viewport and whether this is resize emulation or native OS scaling;
  resizing alone does not establish native Windows rendering coverage.
- Check the changed layout on both sides of its current CSS breakpoint. Test a
  narrower/mobile viewport when the requested surface supports it; do not infer
  universal mobile support from a compact sidebar.
- Measure viewport bounds, element bounds, `clientWidth`/`scrollWidth` and
  `clientHeight`/`scrollHeight`. Exercise scrolling to the last row/action. A
  hidden overflow rule can hide inaccessible content without a page scrollbar.
  Identify the intended scroll owner in the current route's styles: tables,
  inspectors, editors and graph panes can have deliberate internal scrolling.
- Compare shared gutter, typography, button height and icon alignment with an
  unchanged equivalent component and the current tokens. Check long identifiers,
  JSON, validation messages, table pagination and inspector/dialog actions for
  clipping and wrapping. A screenshot and computed geometry serve different checks.
- Check keyboard access, visible focus, meaningful control names, heading
  structure, focus entry/return for dialogs and keyboard dismissal where supported.
  Inspect disabled/loading feedback and text contrast against the current theme.
  Preserve the documented Modal and Button/icon workarounds until their specific
  regressions pass with the replacement bundle.
- Observe console and network activity during the interaction, not only initial
  load. Distinguish expected polling from duplicate mutations or retry loops;
  explain expected denial/error responses rather than hiding them from the report.

Use the DOM and geometry APIs supported by the available browser tool. Do not
install a new browser framework just to follow this checklist. If the tool cannot
observe a required condition, record that limitation and the evidence available.

## Evidence record

Create `ui/audit/YYYY-MM-DD-short-scope/audit.md` with relative screenshot links.
Keep these details sufficient to repeat the observation:

| Field | Record |
| --- | --- |
| Build | Commit and relevant uncommitted changes; dev or Rust-served production UI |
| Environment | Browser, CSS viewport/scale method, UI/API origins, backend, actor role and tenant |
| Scope | Routes, changed interaction and the acceptance criteria actually exercised |
| Result | Steps, expected/observed outcome, PASS/FAIL/UNTESTED per criterion |
| Persistence | Mutation result and fresh read/reload evidence, or why not applicable |
| Diagnostics | Relevant console/network failures and sanitized response details |
| Cleanup | Disposable data/processes removed or explicitly retained |
| Follow-up | CG issue links and any coverage limitations |

Keep credentials and real user data out of screenshots, logs and tracked files.
Use synthetic data so token displays can be excluded without obscuring the tested
state. An intercepted error fixture can verify presentation; label it as synthetic
and verify real API authorization or persistence separately. Do not describe
synthetic responses, historical screenshots or static checks as current live proof.
