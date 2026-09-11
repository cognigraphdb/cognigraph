# CG-54 final status-filter layout — 2026-09-11

**PASS on the final candidate.** This record supplements the
[initial functional audit](audit.md), whose captures and manifest are preserved.
Final screenshot inspection found that its capture 11 clipped the later status
choices inside the segmented control, despite the outer containers fitting.
The outer-width measurements alone did not establish label visibility.

The final scoped CSS rule prevents the status group from shrinking. It can move
to another controls row when necessary. No JavaScript or Rust behavior changed
after the initial audit's last candidate. The final assets are `index-nv021cbh.js`
and `index-m5675c9e.css`; [final manifest](manifest-final.json) binds the candidate,
this follow-up and the original evidence without rewriting the earlier hashes.

Verification used the same Enterprise release binary at `127.0.0.1:38481` with
a fresh disposable Native store and synthetic Admin. The existing seed harness
created another 101-space/402-neuron fixture. Its output went to the owned
temporary directory so the earlier seed record and persisted-verdict evidence
were not replaced. No provider calls or existing user records were involved.

- [Final screenshot](14-final-status-options.jpg): all five labels are fully
  visible with an off-page selected inspector at 1067×667.
- [Final per-label geometry](status-geometry-final.json): at 1600×1000, 1280×800
  and 1067×667, each label has equal client/scroll widths and lies inside the
  326px status group. At the narrowest size, the group ends at x=620 within the
  queue panel's right boundary x=661. Sizes are CSS resize emulation on macOS,
  not native Windows scaling or mobile qualification.
- [Keyboard status transitions](status-keyboard-final.json): focusing the visible
  Accepted label then pressing Right reaches Rejected, Retired and All. Each
  updates the URL, resets page one and clears the previous selected identity.
  [All-status capture](15-keyboard-all-status.jpg) shows continuation still available.
  Directly targeting the zero-sized radio input with a locator timed out; the
  visible label and normal keyboard path succeeded.
- [Console diagnostics](console-layout-final.json): no captured warnings/errors.
  The browser session was signed out and closed; the viewport override reset.
  The temporary server/store were removed after verification.

Final `bun run check`, 132 tests (19 files, 684 assertions), production build,
documentation, decision-index, issue and whitespace checks pass. The initial
audit owns paging, verdict, catalog and failure/retry coverage and its limitations.
No Rust suite, remote CI or publication was added by this CSS follow-up.
