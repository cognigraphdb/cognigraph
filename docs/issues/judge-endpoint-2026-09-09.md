# CG-36 dedicated judge endpoint verification — 2026-09-09

Dedicated primary and partner review judges now honor the validated
`OPENAI_BASE_URL` from the shared completion configuration. They retain their
explicit OpenAI model selection even when construction uses Gemini. An absent
primary still falls back to the completion provider; an absent partner still
withholds Lane A+. [CG-36](CG-36.md) is resolved.

CG-35 was committed locally as `d98e374`, excluding unrelated research/draft
work and the existing `CLAUDE.md` deletion. CG-36 was subsequently committed
locally as `57d65af`; nothing was pushed. The evidence below records its
verification before that commit.

## Implementation and compatibility

The [shared configuration](../../crates/cognigraph-embeddings/src/completion/config.rs)
captures both judge model settings alongside completion, side-view, credential,
and endpoint settings. The two new provider methods resolve dedicated models
through the existing OpenAI `named_spec` path. The
[server](../../crates/cognigraph-server/src/main.rs) resolves all four lanes
before opening storage and installs the resulting provider handles in state.
The old dedicated constructors passed `None` as the base URL, silently selecting
the external OpenAI default. Test-only state builders remain available for
deterministic injected providers.

No new environment variable is introduced. Dedicated judges share
`OPENAI_API_KEY` and `OPENAI_BASE_URL`; their model names remain independent of
the main provider/model. Existing URL parsing trims whitespace and trailing
slashes, requires an absolute HTTP(S) URL, and rejects query/fragment components
without exposing the rejected value in diagnostics. Unset/blank URLs retain
the provider default.

**Compatibility change:** a nonempty dedicated judge model now requires a
nonempty OpenAI key. Missing/blank keys and invalid selected judge endpoints
fail startup before storage opens. Previously, a missing key silently disabled
the requested judge (primary fell back; partner was omitted), while a blank
key could create an unusable provider. Blank/absent model settings still mean
no dedicated provider. An unused OpenAI endpoint remains irrelevant to Gemini
fallback when no dedicated judges are selected.

This changes provider construction only. Review prompts and `POLICY_REV`,
model/pair qualification, symbolic validation, publication fencing, and
acceptance/queueing rules are unchanged. The
[review-policy decision](../decisions/decision_review_policy.md#dedicated-judge-endpoint-resolution-cg-36-2026-09-09)
records the compatibility decision and live outcome; the
[operator table](../operations/configuration.md#configuration-reference) and
[example environment](../../.env.example) document configuration.

## Rust gates

- `cargo test -p cognigraph-embeddings`: **21 passed**, zero failed/ignored.
  The three added configuration tests cover OpenAI selection independent of
  Gemini construction, model/key/base normalization, default/absent lanes,
  invalid endpoint rejection, missing/blank keys, and redacted diagnostics.
- `cargo test -p cognigraph-server routes::construct::tests`: **24 passed**,
  including qualification, agreement, attribution, and policy regressions.
- `cargo fmt --all -- --check`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo test --all`: **961 reported passes** in 72 result groups, zero
  failed/ignored. Eight unconfigured Arango integration entries early-returned.
- `cargo build --release -p cognigraph-server -p cognigraph-cli`: passed.

The full formatting, Clippy, and workspace-test gates ran in that order.
The embeddings crate's two standard live integration tests load the existing
repository `.env`; both provider keys were configured and those tests passed.
They are existing embedding regressions, separate from the synthetic judge
routing harness. No live judge qualification or new model comparison was run.
The [validation artifact](evidence/judge-endpoint-validation-2026-09-09.json)
records gate logs, source/binary hashes, documentation checks, and limits.

## Release verification

The [harness](evidence/judge-endpoint-http.py) uses disposable authenticated
Native servers and two protocol paths on a loopback provider: OpenAI chat
completions and Gemini generation. It checks exact request paths, synthetic
credential headers, explicit model names, and each screen/quality stage.
Every release process runs outside the checkout with explicit settings and
embedding disabled. A local outbound proxy rejects CONNECT without forwarding
it; this reproduces the old default-endpoint behavior without contacting OpenAI
or sending a judge request outside the local process. All processes stop after
verification, with temporary stores/logs retained for inspection.

The saved pre-fix server has SHA-256
`eed0bdac848e4928e730881f12a7e443e9ccb2c236a4d6ec67e394f2695fb806`.
The corrected server has SHA-256
`42b4397f8e032a15e9ab0dad30bebc7e1caa75ab6bf94990bf4ad452dce21ca5`.
Both artifacts record HEAD during the run; their binary hashes distinguish the
saved executable from the release built with uncommitted CG-36 changes.

Six routing cases run in resident/embedded, resident/sidecar, and paged/sidecar
modes: OpenAI fallback, Gemini fallback, dedicated primary with Gemini main,
dedicated partner with Gemini fallback primary, two dedicated OpenAI judges
with Gemini main, and blank judge models with an unused invalid OpenAI URL.
The corrected run additionally verifies four policy controls in resident/sidecar:
an unqualified primary, mismatched attested pair, missing partner, and partner
disagreement. All four queue proposals with correct attribution; configured
models alone do not confer acceptance authority.

| Observation | Saved release | Corrected release |
|---|---:|---:|
| Routing review scenarios across three Native modes | 18 | 18 |
| Additional qualification/agreement controls | — | 4 |
| Successful local review responses | 9 | 22 |
| Blocked attempts to `api.openai.com:443` | 9 | 0 |
| Forwarded outbound connections | 0 | 0 |
| Loopback screen/quality requests | 24 | 58 |
| Invalid judge configurations rejected before storage opens | 0 of 12 | 12 of 12 |

The [baseline artifact](evidence/judge-endpoint-baseline-http-2026-09-09.json)
records nine dedicated-judge failures as expected defect reproductions. Every
failed review leaves its proposal exactly unchanged. All twelve invalid
configurations instead start the old server and create storage; the startup
probes send no provider requests.

The [corrected artifact](evidence/judge-endpoint-fixed-http-2026-09-09.json)
verifies 22 successful review scenarios and 12 startup rejections. All eighteen
routing cases accept the synthetic eligible proposal; the four authority
controls retain it as proposed with triage. Qualified A+ acceptance records
both named judges and both verdicts. The startup matrix tests missing keys,
blank keys, relative URLs, non-HTTP schemes, query strings, and fragments for
each dedicated lane while Gemini main/side-view configuration remains valid.
Error messages identify the selected setting without leaking the marker value
or synthetic keys, and the rejected starts create no database file.

The synthetic policy flags and model names exist only in disposable stores.
These checks validate wiring and existing policy enforcement; they do not
qualify real models, change prompts, measure provider quality, or benchmark
latency/cost. The judge release harness sends no external model requests. The
standard Rust suite's existing live embedding checks are disclosed above.

## Reproduction and remaining work

```bash
cargo test -p cognigraph-embeddings
cargo test -p cognigraph-server routes::construct::tests
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
cargo build --release -p cognigraph-server -p cognigraph-cli
python3 docs/issues/evidence/judge-endpoint-http.py --output /tmp/cg36-fixed-http.json
```

While the saved binary is available:

```bash
python3 docs/issues/evidence/judge-endpoint-http.py \
  --binary /tmp/cognigraph-pre-cg36-server --expect-default-endpoint \
  --output /tmp/cg36-baseline-http.json
```

All final Rust gates and both release matrices completed without failed
verification attempts. Baseline blocked connections and accepted invalid
configuration are deliberately asserted historical defects, not desired
behavior. The first documentation check found a stale operations anchor in the
API examples; it was corrected and the link check rerun. Local links,
registry statuses/counts, and all 24 pre-existing
user-owned paths were checked. No research drafts, `ui/`, secrets, or local
environment files were edited; `.env.example` documents the new resolution.
The registry had **33 Resolved and 5 Open** issues at this checkpoint. Its next
bounded issue, CG-24 research counting-unit consistency, was subsequently
resolved in the [counting-unit report](paper-counting-units-2026-09-09.md).
Broader model benchmarking remains deferred
until the original ticket list closes, with Luna as the economical baseline.
