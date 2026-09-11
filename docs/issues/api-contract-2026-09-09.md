# CG-22 API and operator contract verification — 2026-09-09

CG-22 reconciles the current construction/job OpenAPI, architecture, operator
configuration, and executable examples with the Rust routes. It adds regression
checks for wire enums and request behavior; production handlers and provider
defaults are unchanged.

## Changes

- OpenAPI lists all four `JobKind` values and includes
  `JobProgress.side_views_written`. Generic submissions use a discriminator and
  four tagged envelopes, so the wrong kind/input pairing is invalid. The shared
  operation-input union uses `anyOf`: ingest and draft inputs overlap and cannot
  correctly use an untagged `oneOf`.
- Added strict generic draft/side-view input schemas, the asynchronous draft
  wrapper fields and responses, source eligibility and caps, nullable defaults,
  and the side-view count clamp. Non-NFC source handles remain supported by CG-33.
- Directed construction now documents the 1–32 chunk, 2 MiB body, and configured
  HTTP deadline limits; one completion call; auto-created rule-less spaces;
  deployment fencing; and replacement of all occurrences for each supplied
  chunk. Valid empty output replaces with zero; malformed output preserves
  occurrence data. A model replay is not a frozen extraction result.
- Updated the main/side-view provider selection and model defaults in both
  operator and API examples. The independent judge endpoint defect was recorded
  as [CG-36](CG-36.md), subsequently resolved in its
  [dedicated endpoint batch](judge-endpoint-2026-09-09.md); no model benchmark or
  qualification changed.
- Added [seven reusable request files and examples](../examples/construction/README.md),
  with HTTP envelopes distinguished from CLI input-only submission. Jobs freeze
  kind-specific execution data; side-view keys are frozen while text and provider
  configuration are read during execution.
- Standard OpenAPI validation additionally exposed existing bare commas in
  inline YAML descriptions that parsed into accidental response fields, plus a
  missing inherited `name` parameter on collection deletion. Corrected those
  declarations while preserving nested response schemas and header schemas.

## Automated drift checks

The existing path and governance-method checks remain. Five new tests in
[`openapi_drift/contracts.rs`](../../crates/cognigraph-server/src/openapi_drift/contracts.rs)
parse the actual YAML, obtain job wire enum names directly from Serde, compare
tagged request/schema bindings and progress fields, and invoke production
routers with the checked-in examples. They check missing required fields,
unknown-field policy, directed chunk boundaries, async idempotency, wrapper
controls, and frozen side-view defaults/clamping. Construction/job method checks,
required path parameters, and response-field checks catch the structural defects
that path-only coverage missed.

The YAML parser is a development-only dependency,
[`serde-saphyr` 1.2.0](https://docs.rs/serde-saphyr/1.2.0/serde_saphyr/fn.from_str.html).
Cargo also refreshed compatible existing `getrandom`/`windows-sys` dependency
references in the lockfile; package versions for those existing entries did not
change. The existing CG-33 dependency changes remain intact. No new YAML parser
is linked into the release server.

The focused run passed all **15 OpenAPI drift tests**. Final formatting and
`cargo clippy --all-targets -- -D warnings` passed; `cargo test --all` reported
**950 passed, zero failed, zero ignored**, across 71 test/doc-test result groups.
Eight reported passes were the unconfigured Arango entries that early-returned.
The existing two provider-embedding smoke entries also reported success as part
of the workspace run; they retain their existing environment-based setup and
are separate from the loopback-only feature harness. Source hashes, gate logs,
and binary identities are recorded in the
[validation artifact](evidence/api-contract-validation-2026-09-09.json).

## Release HTTP and CLI verification

The [release harness](evidence/api-contract-http.py) starts the real server with
authentication and a disposable persistent Native store. Its environment is
explicit and does not load repository `.env` credentials. OpenAI main completion,
Gemini side-view completion, and Ollama embedding endpoints are all synthetic
loopback servers. No production data or external provider was used by this
harness, and Docker was not required.

[Raw evidence](evidence/api-contract-http-2026-09-09.json) records **57 checks**
and **16 local provider calls**. The served `/openapi.yaml` matched the source
byte-for-byte and passed a standard OpenAPI 3.0 validator. Request fixtures and
actual job/submission/list responses passed OpenAPI schema validation.

The HTTP run verified:

- Directed extraction writes one grounded fact; malformed completion returns
  500 with the exact earlier fact collection preserved. Valid empty output
  withdraws the supplied chunk's occurrence while keeping an independent peer.
- Missing required directed fields and unknown controls return 422; empty or
  oversized chunks and invalid taxonomies return 400; a body over 2 MiB returns
  413. Unsupported job kinds return 422, mismatched kind/input returns 400,
  missing auth returns 401, and missing idempotency returns 400.
- All four generic kinds and the async draft/side-view wrappers submit, finish
  successfully, and replay the same job. New submissions return 202 and the
  exact `Location` header; replay returns 200. The generic draft rejects wrapper
  controls and 2,001 title groups; the wrapper removes its async/per-document
  controls from frozen public input.
- Side-view generation skips existing rows without regeneration, accepts
  0/51/null counts as frozen 1/50/12, defaults blank `text_field` to `text`,
  rejects negative/fractional counts and ineligible source collections, and
  returns 409 for changed idempotent input. Regeneration succeeds with the
  independently selected side-view provider.
- Real CLI document creation, job status for three kinds, and filtered side-view
  job listing succeed. Main calls use `gpt-5.6-luna` with `reasoning_effort: none`;
  side-view calls use `gemini-3.8-flash`. These checks verify configured routing,
  not real-model quality, latency, pricing, or qualification.

This batch does not repeat the full signed deployment ceremony or all Native
storage/restart permutations. Directed deployment fencing and side-view restart
semantics retain the focused CG-3/CG-13/CG-33 evidence. Arango behavior was not
changed or live-tested in this documentation batch. The full Rust runner's
unconfigured Arango entries early-return and must not be read as live database
coverage.

## Corrected verification attempts

The first parsed-schema run failed on an unquoted comma-containing description.
An initial broad quoting edit also swallowed nested response/header fields;
those changes were corrected and unrelated formatting restored before the final
full OpenAPI validation. The standard validator then caught the pre-existing
ambiguous descriptions and missing collection path parameter described above.

The first HTTP run assumed a case-sensitive `Location` header dictionary. HTTP
header names are case-insensitive; the harness now normalizes them. An unused
test import and deprecated Python resolver usage were removed before the final
gates. The final schema and HTTP runs passed. The release build succeeded without
disk-space or Docker errors.

## Reproduce and status

```bash
cargo test -p cognigraph-server openapi_drift
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
cargo build --release -p cognigraph-server -p cognigraph-cli
uv run docs/issues/evidence/api-contract-http.py --output /tmp/cg22-api.json
```

The harness declares its Python dependencies and validates the complete served
spec before invoking examples. Runtime schema checks exercise representative
requests; the OpenAPI remains hand-maintained and does not encode every stateful
governance condition. Operational bounds and known restrictions are explicit in
the prose and linked procedures.

CG-21 is committed as `f74f0aa`. CG-33 and CG-22 were committed locally as
`9fb4933`; nothing was pushed. Unrelated research/draft changes and the existing `CLAUDE.md` deletion
were preserved. The subsequent roadmap reconciliation is recorded in
[CG-23 verification](roadmap-parity-2026-09-09.md).
