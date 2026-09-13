# CG-29 completion-provider selection — 2026-09-08

CG-29 is resolved locally after CG-32. Both batches remain uncommitted; the
last local commit is `291fbb1`. No push was performed, and unrelated research,
draft changes, and the `CLAUDE.md` deletion were preserved.

## Change and contract

`cognigraph-embeddings::completion::CompletionConfig` captures the environment
once for server startup. Its resolver also powers the existing completion,
side-view, and named-provider harness helpers. Server construction now honors
`COGNIGRAPH_COMPLETION_PROVIDER` even when both keys exist. Without an override,
OpenAI takes precedence when its key is nonempty, then Gemini. No keys and no
explicit provider leaves the optional server lanes disabled.

Side-views inherit the main provider and model unless overridden. An explicit
side-view provider uses its own default model when the side-view model is
unset, including when that provider is OpenAI. A named benchmark provider also
uses its own default independently of the main completion model. This prevents
the legacy OpenAI constructor's environment fallback from leaking across lanes.

Selected providers share model and base-URL handling in the server and
harnesses. `OPENAI_BASE_URL` and `GEMINI_BASE_URL` accept absolute HTTP(S) base
URLs without a query or fragment; trailing slashes are normalized. Surrounding
whitespace is trimmed. Blank keys count as absent; blank models/URLs select
provider defaults. Invalid explicit providers, including empty strings, missing
selected keys, and invalid selected URLs fail startup before opening stores.
Side-view configuration errors are no longer swallowed. Remove an explicit
provider setting to inherit rather than assigning an empty string.

Direct low-level constructors retain their existing APIs. Dedicated review
and partner judges retain their separate OpenAI configuration. This batch does
not change embedding-provider selection or claim an external model-quality
measurement.

## Verification

Required Rust gates passed:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
```

The full suite passed **888 tests**, zero failed or ignored, across 66 result
summaries. Twelve new configuration tests cover default precedence, explicit
selection, missing/blank keys, invalid values, inherited and separate models,
base URLs, and named benchmark defaults. The focused completion suite passed
16 tests. Both release binaries built successfully:

```bash
cargo build --release -p cognigraph-server -p cognigraph-construct \
  --bin cognigraph-server --bin sideviews-benchmark
python3 docs/issues/evidence/completion-provider-http.py
```

## Real release HTTP and harness evidence

The saved pre-fix release reproduced the original mismatch: with both synthetic
keys and an explicit Gemini override, a real construction HTTP request reached
the OpenAI stub. Baseline side-view calls were intentionally excluded because
the old helpers ignore local base URLs.

The fixed release passed **13 server configurations**, **13 invalid-startup
checks**, and **3 benchmark runs**, using authenticated disposable Native
databases and loopback OpenAI/Gemini completion and Ollama embedding stubs.

- Eleven enabled configurations each completed one construction request and
  one durable side-view job, recording the actual provider and model observed
  by the HTTP stub. Each job succeeded with one side-view written. Cases
  included both-key precedence, each key alone, explicit OpenAI/Gemini,
  whitespace-only OpenAI key fallback, inherited models, independent provider
  defaults, a side-view model override, and provider-plus-model overrides.
- Two configurations with no usable keys started successfully. Construction
  and side-view generation reported unavailable providers without making a
  completion request.
- Thirteen invalid configurations exited nonzero with the expected diagnostic
  before creating the persistent database, making zero completion requests.
  Cases included invalid/empty providers, missing/blank selected keys, and
  malformed selected URLs in both completion lanes.
- Three real `sideviews-benchmark` processes verified inherited Gemini,
  explicit OpenAI with its default model, and three named provider/model pairs.
  All five observed calls matched their expected provider/model; generated
  output files contained the expected synthetic pair.

No validation or live check failed. No production data or external provider
requests were used. These are routing/configuration and job-execution checks;
external provider availability and model quality were not tested.

- [Reproducible loopback regression](../evidence/engineering-historical-checks.md#artifact-9599d4e1f8da5ec17718)
- [Pre-fix observation](../evidence/engineering-historical-checks.md#artifact-65fc48fd0507d84c76f9)
- [Fixed release observations and binary SHA-256 values](../evidence/engineering-historical-checks.md#artifact-aebb355b52fd7db98b17)

The next bounded issue is [CG-7](CG-7.md), where accepted `DOCUMENT()`
expressions in mutations can silently store null.

## Follow-up: current model defaults

At the user's request on 2026-09-08, the active defaults were refreshed to
`gpt-5.6-luna` and `gemini-3.8-flash`. The shared resolver and the low-level
constructors use the same defaults. Luna explicitly sends
`reasoning_effort: "none"`, preserving the prior OpenAI default's effective
reasoning policy. Other model overrides retain their request shape. The
[provider decision](../decisions/decision_sideviews_provider_and_benchmark.md#current-model-defaults-2026-09-08)
records the official model capability sources and compatibility checks.

The release routing matrix was rerun with these defaults: all 13 server
configurations, 13 rejected startups, and 3 benchmark runs passed. HTTP stubs
asserted Luna's reasoning field and its absence on other OpenAI model
overrides. Each enabled side-view job again wrote one generated pair. The
[new release evidence](../evidence/engineering-historical-checks.md#artifact-201201a66072dda40b7c)
records the new binary hashes and model IDs; the original CG-29 evidence and
July model-quality benchmark remain historical records. These checks use
synthetic loopback providers and do not measure the new models' actual output
quality, latency, or external API availability.

The required formatting, Clippy, and full test commands were rerun and all
passed: 888 tests, zero failed or ignored, across 66 result summaries. The model
refresh remains uncommitted with CG-29 and CG-32; nothing was pushed.

## Subsequent real provider comparison

The user subsequently authorized the
[real side-view model comparison](../research/experiments/sideviews-model-comparison-2026-09-08/README.md):
both API preflights and all 30 measured requests succeeded. The earlier
loopback-only limitations above describe the initial implementation checks;
the new report records actual provider usage, latency, outputs, cost estimates,
and source-fidelity review separately. No Rust code or judge qualifications
changed during the experiment.
