# Collection search limits and filter scope — CG-59

## Environment and build

- Base commit: `4de66d5e0295267f5bfdce7e2991c3ae6bd995e9`; uncommitted CG-58
  and CG-59 changes. Production UI served by the actual Enterprise 2.7.0 Rust
  binary with a disposable Native store, authenticated Admin in `default`.
- Direct origin: `http://127.0.0.1:38493`; forwarding/fault-fixture origin:
  `http://127.0.0.1:38494`. Browser: Codex in-app browser, macOS. One owned QA
  tab; the user's Docker Hub tab was retained.
- Final assets: `index-0d54tqzv.js`, `index-0pnq4bz5.css`. The
  [manifest](manifest.json) binds source, built assets, runtime and evidence.
  The binary was reused, not rebuilt; Rust source did not change.
- [Seed script](seed.py) creates 126 synthetic documents: 125 text matches and
  a separate exact-key document. [API fixture](api-fixture.json) confirms 125
  text hits at limit 200, versus the UI's requested limit 100. No providers or
  model calls. The stored Ready status is a display fixture, not a generated
  embedding-quality claim.

## Build checkpoints

Capture 01 is the original CG-58 production bundle. Captures 02–16 are the
[preflight build](preflight-build.json), which exposed a new health-poll clearing
regression during the delayed request check. That regression was fixed before
acceptance. Captures 17–31 use the [functional build](functional-build.json),
`index-8pq7eamm.js`, with the final data-loading, filter and pagination behavior.
The final build changes only failure guidance and the failed-list footer so a
failed request cannot display a misleading zero count; final captures 32 onward
recheck the relevant states and layouts. Historical checkpoints are retained,
not relabelled as final-build captures.

## Executed checks

| Check | Result and evidence |
|---|---|
| Text cap and separate exact key | PASS: 100 text hits plus `constellation` produce `1–25 of 101 retrieved`; the heading remains 126 documents and the cap warning states more matches may exist. [Capped search](19-final-cap-1067.jpg), [terminal fifth page](20-final-terminal-page.jpg). |
| Page-local embedding filter | PASS: Ready returns `0 of 25 on this page` on page one while Next stays enabled. Page two returns 24 Ready records of 25 loaded. [Empty page](17-final-filtered-empty.jpg), [later page](18-final-later-page.jpg). |
| Category availability | PASS: page one's choices exclude `later-only`; page two offers it. Selecting it and going back preserves the selected label with an empty page. [Retained category](05-retained-category-empty-page.jpg); this check was on the preflight build, and its filter logic is unchanged in the final build. |
| Search filter/reset | PASS: filtering the terminal retrieved page to Missing returns to page one with `1–25 of 25 filtered retrieved`. Reset clears search/category/embedding and restores `1–25 of 126 documents`. [Filtered search](22-final-search-filter-reset.jpg), [Reset](23-final-reset-collection.jpg). |
| Off-page key and no matches | PASS: direct `?doc=item-124` resolves the actual stored document and inspector. A no-match query shows `0 of 0 retrieved` with no inspector. [Off-page key](29-final-off-page-key.jpg), [empty search](28-final-no-matches.jpg). |
| Routine health polling | PASS: terminal page five and its result remain stable across a real health poll, with no additional text search. [Browser](21-final-after-health-polls.jpg), [trace comparison](health-poll-stability.json). |
| Partial search failure | PASS: closing only the text-search connection retains the successful exact-key read, displays an inline error and provides Retry. [Partial result](24-final-partial-search-error.jpg). This is controlled transport failure, not a backend denial. |
| Keyboard Retry and delayed recovery | PASS: Tab reaches Retry with a visible 3px outline; Enter starts a delayed real request. Previous rows/inspector disappear during loading, then 101 retrieved documents return without the error. [Focus](25-final-keyboard-retry-focus.jpg), [loading](26-final-retry-loading.jpg), [recovery](27-final-retry-recovered.jpg), [focus trace](retry-keyboard.json). |
| Listing failure/retry | PASS: controlled listing failure offers Retry collection. Retry obtains the real page. [Recovery](31-final-list-recovered.jpg); the final build also replaces the failed-list count with `Collection page unavailable`. |
| Keyboard paging and scroll owner | PASS: activating the last loaded row scrolls the internal table to it; Tab then reaches page controls with visible focus, and Enter advances. [Last row](14-last-loaded-row.jpg), [Next focus](15-keyboard-next-focus.jpg), [focus trace](keyboard.json). These preflight observations use unchanged table-control behavior. |
| Desktop layout | PASS: actual CSS viewports 1280×800 and 1067×667 emulate 125% and 150% scaling of a 1600×1000 display. Both sides of the sidebar breakpoint and filter container breakpoint were exercised, including a full-width empty table. Scope text, error, footer and inspector remain within bounds; tables scroll internally. [Measurements](layout.json). This is resize emulation, not native Windows scaling or universal mobile coverage. |

Final-build captures: [1280×800 capped search](32-final-cap-1280.jpg),
[1067×667 capped search](33-final-cap-1067.jpg),
[failed listing and truthful footer](34-final-unavailable-page.jpg), and
[retry recovery](35-final-collection-recovered.jpg).

## Diagnostics and validation

The [proxy](proxy.py) forwards ordinary requests to Rust; its mode file selects
an eight-second text delay or closes only a text/listing connection. The
[sanitized trace](requests.json) records routes, synthetic search inputs,
statuses and timing without headers or credentials. Missing-key 404s are expected.
No mutations were performed through the UI; fixture import and fresh API reads
establish the persisted corpus. CRUD, ArangoDB, new role/edition matrices and
reversed completion timing were not qualification targets for this ticket.

UI lint/types, 161 tests in 22 files (783 assertions) and the production build
pass. Documentation, decision index and issue-registry checks pass. No Rust suite
was required or rerun for this UI-only change. Local results do not establish
remote CI or publication. [Direct browser logs](console-direct.json) and
[proxy-origin logs](console-proxy.json) are retained; controlled network failures
are separately evidenced by the request trace.

## Cleanup and remaining scope

Owned QA sessions were signed out, the QA tab closed and the viewport restored.
The owned Rust server/proxy were stopped and `/tmp/cg59-qa` removed. Existing
user data and the Docker Hub tab were preserved. CG-58 sealed captures remain
unchanged.

[CG-59](../../../docs/issues/CG-59.md) is resolved by explicit bounded retrieval
and filter scope; it does not add a cursor, exhaustive search or whole-collection
facets. The remaining UI findings are CG-63, CG-61 and CG-60. No commit or push
was performed during this turn.
