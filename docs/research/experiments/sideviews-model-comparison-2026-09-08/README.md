# Real side-view model comparison — 2026-09-08

> Public reading copy; original research captures and scripts are private.
> Original path: `fixtures/semantic-neurons/sideviews-model-comparison-2026-09-08/README.md`.
> Original SHA-256: `f93915aaa5c0959007cb8aaf85c85a35e33d0481b2f6512c178c4efb9bf06f55` (7314 bytes).
> Navigation changed; dated results and limitations retain their original scope.


**Luna was the cheaper and faster configured baseline on this small fixture.**
Gemini met the requested pair count more consistently. The review does not
establish a general quality winner or qualify either model for governed judging.

Both official APIs accepted the requested model IDs. We made two preflight
calls followed by 30 measured calls: the existing five public passages, three
repetitions per model, requesting 12 Q&A pairs per passage. All 32 requests
succeeded; the comparison below excludes the two preflights.

| Measure | `gpt-5.6-luna` | `gemini-3.8-flash` |
|---|---:|---:|
| Measured requests / valid schema | 15 / 15 | 15 / 15 |
| Generated Q&A pairs | 178 | 180 |
| Exactly 12 pairs | 13/15 | 15/15 |
| Mean latency | 3.679 s | 5.141 s |
| Median latency | 3.577 s | 4.653 s |
| Maximum latency | 4.638 s | 9.602 s |
| Mean answer words | 9.65 | 10.00 |
| Answer/source content-word overlap | 95.90% | 95.55% |
| Lexically distinct / self-contained questions | 100% / 100% | 100% / 100% |
| Input tokens | 5,790 | 5,106 |
| Output tokens, including thinking | 5,090 | 18,976 |
| Thinking/reasoning tokens within output | 0 | 11,780 |
| Estimated cost, 15 requests | $0.007266 | $0.074990 |
| Estimated cost per 1,000 similar passages | $0.4844 | $4.9993 |

Gemini's estimated token cost was **10.32 times** Luna's. Luna's observed mean
latency was **28.4% lower**. This compares the actual configured defaults:
Luna explicitly uses `reasoning_effort: none`; Gemini's thinking setting is
omitted, and the API reported thinking tokens. It is not a comparison under
equal reasoning budgets. The apparent historical Gemini price advantage from
the July comparison does not apply to this model/settings pair.

## Quality and completeness review

[The unblinded assistant review](../../../evidence/research-sideviews-model-comparison-2026-09-08.md#artifact-ba9b38924c772ff5488b) checked all 358 measured
Q&A pairs against the supplied passages. It records two concrete additions
that the lexical scores did not identify:

- Luna, engineering round 3, pair 9: assigns A318/A319 to shorter variants and
  A321 to longer variants. The passage names the variants but does not make
  those individual assignments. External truth would not satisfy the prompt's
  source-only requirement.
- Gemini, engineering round 2, pair 12: asks for the A320's “launch customer”
  and answers with Air France/service entry in 1988. The passage supplies
  service-entry facts, not the separate commercial launch-customer role.

Completeness differs from the pair count. Luna returned 11 pairs for biography
in rounds 2 and 3, yet covered Curie's pioneering radioactivity research in all
three rounds. Gemini returned 12 each time but omitted that explicit fact in
all three. Luna also omitted the traditional-database disk-storage contrast in
one software run. Both models produced semantically overlapping questions even
though no question strings were duplicates after normalization.

These are qualitative observations, not a calibrated hallucination rate or
complete semantic-recall score. There was one reviewer, model identities were
visible, and the review criteria were applied after generation. Neither
retrieval recall nor construction/judge correctness was measured.

## Method, evidence, and cost basis

The production Rust `sideviews-benchmark` binary and shared generation prompt
were used. No Rust implementation changed for this run. A loopback recording
proxy forwarded requests to `api.openai.com` and
`generativelanguage.googleapis.com`, preserving request bodies and returning
the upstream responses. Keys were loaded by the existing Rust dotenv handling;
no credential file was edited, and request headers were not persisted.

Provider order alternated by repetition. Each repetition used a recorded seeded
shuffle of the same five passages, shared by both providers. Repeated requests
for each provider/passage had identical body hashes; all calls shared one
canonical schema hash. Provider-returned model IDs matched the requests.
No retries, prompt changes, or adaptive model tuning were performed.

Latency measures the complete upstream HTTP request, including a fresh
connection/TLS setup through the recorder. It is not token-first latency or
production latency with a pooled direct connection. There are only five unique
passages; repetitions do not make this a representative 30-document sample.

Costs use the API-reported token counts and standard short-context text rates
checked on 2026-09-08: OpenAI input/cached-input/cache-write/output rates of
$0.20/$0.02/$0.25/$1.20 per million tokens from the
[Luna model page](https://developers.openai.com/api/docs/models/gpt-5.6-luna).
Gemini input/cached-input/output rates are $0.75/$0.075/$3.75 per million tokens,
including thinking in output; those introductory rates run through 2026-12-31.
[Google pricing](https://ai.google.dev/gemini-api/docs/pricing#gemini-3.8-flash)
No cache hits or cache writes were reported. Total estimated list-price cost
including preflights was **$0.08832945**. Actual invoices, free-tier allowances,
regional adjustments, discounts, and taxes were not observed.

- [Run manifest: UTC times, binary and input hashes](../../../evidence/research-sideviews-model-comparison-2026-09-08.md#artifact-c197e6640f6ee8ab87a1)
- [Per-call outputs, usage, latency, schema and request hashes](../../../evidence/research-sideviews-model-comparison-2026-09-08.md#artifact-11b564b5d4750e104193)
- [Aggregate and per-round metrics](../../../evidence/research-sideviews-model-comparison-2026-09-08.md#artifact-8656d7f40f89c2e8695b)
- [Qualitative source review](../../../evidence/research-sideviews-model-comparison-2026-09-08.md#artifact-ba9b38924c772ff5488b)
- [Recorder and bounded runner](../../../evidence/research-sideviews-model-comparison-2026-09-08.md#artifact-eb744fc4ad0ed9278b4c)
- [Offline analysis](../../../evidence/research-sideviews-model-comparison-2026-09-08.md#artifact-d6a3473e775cd98b702c)

Each round also retains the Rust harness log, ordered input passages, and raw
Q&A dump. Checks confirmed agreement between the Rust metrics and the offline
analysis, equality between captured outputs and Rust output files, identical
repeated request bodies, schema/model identities, and no credential patterns
in the new artifacts.

To reproduce, build the existing release harness and use a new output directory:

```bash
cargo build --release -p cognigraph-construct --bin sideviews-benchmark
python3 fixtures/semantic-neurons/sideviews-model-comparison-2026-09-08/run.py \
  --out fixtures/semantic-neurons/sideviews-model-comparison-2026-09-08/run-2
python3 fixtures/semantic-neurons/sideviews-model-comparison-2026-09-08/analyze.py \
  fixtures/semantic-neurons/sideviews-model-comparison-2026-09-08/run-2
```

The runner makes at most 32 paid API requests and stops the measured sequence
on a failed round. `--preflight-only` limits it to two calls. It does not create
keys, alter model defaults, write to a graph, or change judge qualifications.

## Next decision

Keep Luna as the economical completion baseline and retain configurable
side-view providers. This sample provides no cost or quality basis for moving
the default side-view lane to Gemini. It also does not justify changing any
judge qualification. The user has deferred broader model benchmarking until
the current ticket list, CG-1 through CG-32, is resolved or explicitly closed.
Then compare the latest available DeepSeek, GLM, and other relevant models
against Luna, checking versions and pricing when that work resumes. The
[issue registry](../../../issues/README.md#after-the-current-ticket-list)
tracks the follow-up. Construction and judge replay/injection evaluations
remain separate prerequisites before claiming improvements in those roles.
