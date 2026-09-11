# CG-50 / CG-62: document and construction data preservation

- Date: 2026-09-11
- Result: **PASS for the scoped acceptance below.** This does not establish full UI coverage.
- Build: product 2.7.0 at `8830e8000c3af075e34df607e2a2d7072feb1dfb`, with
  uncommitted UI fixes. [Manifest](manifest.json) records the final source and asset hashes.
- Runtime: Rust-served production Bun build, Native resident storage. Enterprise
  embedded vectors for the main browser run; Community embedded vectors for
  create → edit → reload; Community sidecar vectors for a separate HTTP probe.
- UI/API: `http://127.0.0.1:3001`; sidecar HTTP: `http://127.0.0.1:3002`.
  Localhost intentionally matches the current UI origin constraint; CG-49 remains open.
- Browser: Codex in-app Chromium. Actors: synthetic Admin and Viewer, tenant `default`.
  All data and credentials were disposable. No external models or holdouts ran.

## Document editing — CG-50

The inspector now displays the raw API document. Display labels and inferred
embedding metadata remain separate. Save computes changed top-level fields
against the snapshot taken when Edit opened, then PATCHes only those fields.
It never converts a string/array/null content value into the display remainder,
adds template defaults, or resends a vector that was absent from the API read.
Unknown top-level fields are edited at their actual location.

`_id`, `_key`, `_rev`, `created_at`, and `updated_at` are read-only. Removing a
field is rejected with guidance to use null when clearing its value. This is
intentional: Native replaces nested objects during PATCH, whereas Arango merges
them. The editor does not pretend that field deletion is portable. Arrays and
scalar/object type changes remain supported. Concurrent edits to the same
changed top-level field still use the existing server PATCH semantics.

POST creation returns an identity receipt rather than the stored document.
The new-document path now GETs the stored record before exposing its inspector.
If that read fails, the UI reports that creation succeeded but loading failed;
it closes the creation dialog to avoid inviting a duplicate submission.

| Executed check | Evidence / result |
| --- | --- |
| Title-only browser edits of object, string, array, null, number and boolean content | Six fresh HTTP reads changed only `title` and server `updated_at`; content, custom objects, mixed-type tags and vectors retained. [Before](00-seeded.json), [after](01-title-edits.json), [editor](01-raw-json-edit.png). |
| Custom field edits | Changed nested numeric field to null and added a mixed-type array; retained original string content. Browser reload and independent GET agree. [Capture](02-custom-and-validation.json). |
| Invalid JSON and identity edit | Save refused; draft remains editable. [Identity validation](02-readonly-validation.png). Field-removal and reserved-field cases also covered by helper regressions. |
| Viewer save | Real PATCH returned 403; draft remained visible, server value survived reload. [Screenshot](10-viewer-edit-denied.png), [denials](denied-requests.json). |
| Hidden vector | Actual Community sidecar read omitted the vector; title-only helper PATCH preserved its searchable result. [HTTP probe](sidecar-probe.ts), [before/after](sidecar.json). This probe did not use a browser. |
| Create → edit → reload | Created collection/document through the Community UI. Immediate JSON had server timestamps and no creation-receipt `collection` field. Changed content object to null, added custom data and changed title. Reload and GET agree. [Initial inspector](created-inspector-before.json), [GET](created-after-http.json), [reloaded UI](12-created-edited-reloaded.png). |

## Construction imports — CG-62

Plain text and JSONL without IDs receive `ui-` plus SHA-256 of a versioned tuple
containing the optional title and NFC evidence text. IDs survive retries and
reordering. Distinct text/title yields distinct identity. Explicit JSONL IDs
remain caller-owned. Duplicate or colliding IDs are rejected before requests;
repeated identical text needs distinct explicit IDs when it represents distinct
sources.

Every import previews all chunks. Exact-key reads check stored space/chunk
identities with at most six concurrent requests, without a collection-page
limit. Existing sources show current and incoming text and require the explicit
**Replace evidence and ground** action. The warning includes rebuilding facts
and mentions, even for unchanged text. Authorization, transport errors and raw
identity collisions fail closed. Stored records are read again before submit;
changes while the preview was open require a new review.

The last read and POST are separate requests: this is a stale-preview guard,
**not an atomic compare-and-swap guarantee against concurrent external writers**.
The unchanged backend contract still treats an existing space/chunk ID as an
intentional atomic replacement of that source's occurrence set.

| Browser action | Independent stored chunks / facts / mentions |
| --- | --- |
| Preview first corpus without confirmation | 0 / 0 / 0 — [capture](03-preview-no-write.json) |
| Confirm `DataCloud runs on Nimbus.` | 1 / 1 / 2 — [capture](04-first-import.json) |
| Confirm separate `Nimbus hosts DataCloud.` corpus | 2 / 2 / 4 — [capture](05-second-import.json) |
| Reload and retry the first corpus with confirmation | 2 / 2 / 4, same identities — [capture](06-retry.json) |
| Preview unrelated replacement under the first explicit ID, then Escape | Entire stored records unchanged — [capture](07-cancelled.json) |
| Explicitly confirm that replacement | 2 / 1 / 2; second source survives — [capture](08-confirmed-replacement.json) |
| Preview restoring the first text; change source via independent HTTP writer before confirming | UI refuses stale preview; no additional write — [external write](09-intervening-write.json), [after refusal](10-stale-and-invalid-rejected.json), [UI](07-stale-preview-rejected.png) |
| Submit duplicate plain-text lines | Validation refuses before mutation; same capture as preceding row |
| Viewer confirms an existing-source import | Real POST returns 403; stored records unchanged — [capture](11-viewer-denials.json), [UI](09-viewer-import-denied.png) |
| Admin confirms restoring the first source on corrected bundle | 2 / 2 / 4; both original corpora retained — [capture](12-final-confirmed-import.json) |

Shared asynchronous draft parsing also reached the real draft endpoint with a
valid request. With no completion provider configured, it returned the expected
explicit provider error. [Screenshot](11-draft-provider-unavailable.png). This
checks request wiring and error presentation, not successful model drafting.

## Layout, keyboard, and diagnostics

Tested 1600×1000, 1280×800 and 1067×667 CSS viewports: resize emulation of normal,
125% and 150% desktop scaling, not native OS scaling. The replacement dialog
and inspector controls stayed within horizontal bounds. Long IDs/text wrapped.
The preview owned vertical scrolling; PageDown advanced it, End reached the
last of eight long sources (scrollTop 2462, scrollHeight 2727, clientHeight 265).
Escape closed the modal and returned focus to Review import. Enter opened it
and confirmed replacement. [Final preview geometry](geometry-final.json),
[inspector geometry](inspector-geometry.json), [long preview](08-long-preview-final.png).

Verification caught and fixed missing focus restoration and Ant Design's
`maskClosable` deprecation in the new dialog. The corrected import bundle was
`index-cxk1mghh.js`; the final create-path bundle is `index-f4bewe5k.js`, with
unchanged import logic. Each had zero observed console warnings/errors during
its subsequent scoped checks: [import console](console-final.json),
[create console](console-created-final.json). Earlier screenshots show the
intermediate count wording before its singular/plural correction; they are
retained as captured. Expected HTTP diagnostics include missing-chunk 404s,
auth probes, the two Viewer 403s and the unconfigured-provider response.

## Validation and reproduction

- `bun run check`: PASS (Biome and TypeScript).
- `bun test`: PASS, 85 tests across 15 files, 222 assertions.
- `bun run build`: PASS, 1717 modules; final assets recorded in the manifest.
- [Capture assertions](verify-captures.py): PASS. These validate saved observations;
  they do not substitute for rerunning the browser steps.
- `audit/` is excluded from Biome to preserve captured JSON bytes. The previous
  full-review captures were not reformatted to satisfy application formatting.
- No Rust source changed. Existing release images were exercised over HTTP;
  no fresh full Rust/CI/Docker build or remote CI run is claimed by this UI batch.

To repeat, start an isolated Enterprise Native image with auth enabled,
embedding/completion providers unset, and the current `ui/dist` mounted read-only
at `COGNIGRAPH_UI_DIST`. Publish only on localhost:3001. Set `CG_QA_PASSWORD` to
its disposable bootstrap password and run `python3 verify-api.py seed` from this
directory. Execute the browser actions in the tables and run
`python3 verify-api.py snapshot LABEL` at each checkpoint. `intervene` performs
the synthetic external source update while the preview is open. The script
expects a fresh database; do not use it against a user's existing dataset.

Use a separate Community Native sidecar instance on localhost:3002 for
`bun sidecar-probe.ts`. For the Community create flow, start another fresh
embedded-vector instance on localhost:3001, then create `qa_created` and its
document through the UI. Rebuilding Bun replaces the mounted dist directory;
restart the disposable container to rebind the current build before reloading.

All three owned containers, anonymous data volumes, and browser tabs were
removed after verification. Existing project data, containers and user tabs were
preserved. No commit, push, version bump, or publication was performed.

Remaining UI work stays in the [registry](../../../docs/issues/README.md).
CG-49 (production origin) and CG-51 (tenant deletion disclosure) are the next P1
items. Live Arango, real provider-backed drafting, other roles/tenants and the
remaining 13 UI findings are outside this scoped acceptance.
