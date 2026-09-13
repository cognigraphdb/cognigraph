# Luna low runtime adoption — 2026-09-09

The user selected `gpt-5.6-luna` with reasoning `low` as CogniGraph's economical
baseline. The shared OpenAI completion provider now sends that setting whenever
Luna is resolved, including inherited side-views and explicit Luna selections.
Other OpenAI model selections retain their provider defaults. Model resolution,
Gemini requests, prompts, schemas, timeouts and response parsing are unchanged.

The original issue registry remains closed. This is the accepted follow-up to
the [baseline decision](../decisions/decision_luna_baseline.md), not a new audit
defect or a qualification of automatic review.

## Rust and release verification

The required commands ran in order and passed:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
cargo build --release --bin cognigraph-server --bin sideviews-benchmark
```

The suite reported 962 passed tests and no failures or ignored tests. The new
HTTP request regression exercises both Luna and a custom model with closed and
open schemas. It checks the actual request body, authentication, JSON response
decoding, Luna's `low` setting, omission of effort for the custom model, and
preservation of schema-specific strictness. Existing configuration tests cover
inherited lanes, named providers, and independent judge-model resolution.

Environment-gated entries are included in the reported test count: eight Arango
integration entries returned early because `ARANGO_PASSWORD` was absent. This
run supplies no new live Arango coverage. The existing OpenAI and Gemini live
embedding tests both completed; provider credentials were available to them.

## Release HTTP evidence

The [loopback matrix](../evidence/engineering-historical-checks.md#artifact-bf189a2e9245548ba8d0) passed 13
server configurations, 13 invalid configurations rejected before opening
storage, and three side-view CLI cases. Its 27 captured completion requests
verify Luna low, original strict JSON schemas, provider inheritance, explicit
model overrides, Gemini's native schema shape, generated-pair persistence, and
disabled behavior without credentials. All provider responses in this matrix
are synthetic; the server and CLI binaries are real release builds.

The [real OpenAI smoke](../evidence/engineering-luna-low-runtime-live-2026-09-09.md#artifact-2d698e923ca868f6e8fa)
then exercised the default model and inherited side-view lane. Its recorder
forwarded the server's request bytes unchanged to OpenAI, preserving
`response_format: json_schema`, `strict: true`, and `reasoning_effort: low`.
The [two complete request/response captures](../evidence/engineering-luna-low-runtime-live-2026-09-09.md#artifact-59e8d9a3532a27094867)
include usage and request identifiers, without credentials. No format adapter,
prompt alteration, or reasoning override was applied, and neither call retried.

- Directed construction returned HTTP 200, proposed and stored the two expected
  facts from active and passive fictional sentences, and produced no fact from
  a meeting-only negative excerpt. Both stored directions, source byte spans,
  model attribution, and occurrence schema were verified.
- The asynchronous side-view job succeeded and stored one pair:
  “Who supplies Bramble?” / “Alpine supplies Bramble.”
- OpenAI accepted both requests and returned `gpt-5.6-luna` with a `stop` finish.
  Provider latencies were 3.441 s and 2.791 s. Usage totaled 557 input and 125
  output tokens; reported reasoning-token usage was zero for these simple
  inputs even though both requests explicitly selected low effort.

Both harnesses used isolated Native databases, synthetic authentication and
local deterministic embeddings. No existing local database was reset. These
are bounded compatibility and persistence checks, not document-quality,
retrieval-quality, reasoning-token-consumption or automatic-judge benchmarks.

## Checkpoint and reproduction

Local commit `8b8cceb` checkpoints the completed CG-26 refactor and all four
frozen evaluation packages before this runtime change. Before adoption, the
600-case benchmark verifier passed, including source replay and integrity
probes. All 545 pinned source/harness files were also checked against that
commit. The old release binary's SHA-256 is
`4267ee99ffab295e55e1c56ebc74c5fe3ed4d1ba1158b789483857876c0f8cc7`.
Use that source revision and matching binary to replay the historical packages;
their old provider settings and hashes intentionally remain frozen.

The newly tested release server SHA-256 is
`46a61487809b6e65e58f42fe929422cf224eb40d251a4d38a14ce20ebc72af32`.
The [validation record](../evidence/engineering-historical-checks.md#artifact-0ff3dc76b45e874b073f)
records commands, source and artifact hashes, preservation checks and limits.
Reproduce the runtime checks with:

```bash
python3 -B docs/issues/evidence/luna-low-runtime-http.py --output /tmp/luna-low-http.json
python3 -B docs/issues/evidence/luna-low-runtime-live.py --output /tmp/luna-low-live-new-attempt
```

The second command makes two real OpenAI requests using the existing key and
requires a fresh output directory. Historical benchmark sources, local secrets,
and the six pre-existing user-owned changes were preserved. No remote push or
publication was performed.

Next, freeze independently checked representative documents with exhaustive
relation labels, entity discovery, near-miss negatives, and separate development
and holdout splits. Measure precision, abstention, evidence fidelity, recall,
failures, latency and cost with Luna low before tuning restraint on development
data and evaluating the untouched holdout. Other models remain optional
references, and DeepSeek V4 Flash remains retired.
