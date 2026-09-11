# DeepSeek and GLM against the Luna baseline — 2026-09-09

**Completed:** 38 provider calls; four complete configurations and one retained
DeepSeek Flash token-limit failure. See [results and recommendation](results.md).

This is a separate captured comparison requested by the user after the
[initial Luna/Astra experiment](../luna-baseline-2026-09-09/results.md).
The original package remains byte-for-byte unchanged. Results apply to the
same small synthetic regression corpus and do not qualify a production
default, an automatic judge, or a general model ranking.

## Frozen configurations

| Arm | Requested model | Reasoning | Endpoint provider |
|---|---|---|---|
| Luna JSON-mode reference | `gpt-5.6-luna` | none | OpenAI |
| DeepSeek Flash | `deepseek-v4-flash` | thinking enabled, low | DeepSeek |
| DeepSeek Pro | `deepseek-v4-pro` | thinking enabled, low | DeepSeek |
| GLM Flash | `glm-5.3-flash` | thinking enabled, low | Z.ai |
| GLM flagship | `glm-5.3` | thinking enabled, low | Z.ai |

The protocol reuses the original 48 fictional excerpts, taxonomy and labels:
32 directed gold triples, 20 negative excerpts, four relation categories and
twelve scenario types. Codex authored the texts and labels together; there is
no independent annotation. These already exposed regression cases are not a
new holdout. Each model receives four twelve-excerpt batches, repeated twice.
A separate one-excerpt preflight checks availability and the response envelope
before each arm. Gold and scenario labels never enter provider messages.

[GLM documentation](https://docs.z.ai/api-reference/llm/chat-completion) requires
thinking enabled for 5.3/Flash and supports low/high/max effort. DeepSeek also
[supports low effort](https://api-docs.deepseek.com/guides/thinking_mode/).
Matching effort labels do not imply identical reasoning compute or sampling.
No sampling override is sent; each provider's defaults remain in effect.

## Transport and comparison boundary

All measured construction requests go through the unchanged, authenticated
release server and its directed gates/Native writer. The loopback recorder
uses the official direct endpoints:

- `https://api.deepseek.com/chat/completions`, with `DEEPSEEK_API_KEY`.
- `https://api.z.ai/api/paas/v4/chat/completions`, with `ZHIPU_API_KEY`.
- `https://api.openai.com/v1/chat/completions`, with the existing OpenAI key.

The user explicitly identified the two new keys in `.env`. Keys are read only
into the recorder, never written, printed or included in artifacts. The
disposable server has a dummy key and receives no local `.env` configuration.
Redirects are refused. Embeddings remain synthetic loopback controls outside
this construction evaluation. No repository provider enum/default changes.

DeepSeek and Z.ai document JSON-object mode, rather than OpenAI's strict
JSON Schema response mode. All five arms therefore use `json_object`, with
the unchanged server schema appended identically to the system message. The
original task instructions and user excerpts remain unchanged. The Rust
deserializer validates model proposals before writing. The original server
request and actual outbound request are both captured, alongside full raw
responses, returned model IDs, usage, timing, errors, gate skips and complete
accepted fact/entity/chunk rows.

This adapter explicitly changes the provider formatting contract. Fresh Luna
JSON-mode results supply the direct comparison; the earlier strict Luna/Astra
results retain their separate configuration identity. A repeated result does
not retroactively upgrade either experiment's evidence.

## Pricing and execution budget

Official prices checked on 2026-09-09, USD per million tokens:

| Model / period | Uncached input | Cached input | Output |
|---|---:|---:|---:|
| Luna | $0.20 | $0.02 | $1.20 |
| DeepSeek Flash, off-peak | $0.22 | $0.007 | $0.66 |
| DeepSeek Flash, peak | $0.44 | $0.014 | $1.32 |
| DeepSeek Pro, off-peak | $0.66 | $0.022 | $1.98 |
| DeepSeek Pro, peak | $1.32 | $0.044 | $3.96 |
| GLM Flash, promotion | $0.075 | $0.015 | $0.25 |
| GLM Flash, regular | $0.15 | $0.03 | $0.50 |
| GLM-5.3 | $1.40 | $0.26 | $4.40 |

[DeepSeek's pricing page](https://api-docs.deepseek.com/quick_start/pricing/)
lists Flash-0731 and Pro-0813 behind the requested aliases, and peak windows
of 01:00–04:00 and 06:00–10:00 UTC on weekdays. All other times are off-peak.
[Z.ai's promotional Flash rates](https://docs.z.ai/guides/overview/pricing)
expire at 2026-09-09 16:00 UTC (24:00 UTC+8). Report regular Flash prices as well
as the observed promotion; neither price should silently be treated as permanent.
[Luna rates](https://developers.openai.com/api/docs/pricing) retain the original
baseline's standard pricing basis.

The experiment admits requests within a conservative $5 reservation using
worst listed prices, UTF-8 request bytes plus a 4,096-token input margin, and
the full 4,096-token output limit. It is not a provider-enforced billing cap.
Cost estimates use returned token counts and request-start UTC. Reasoning
tokens are already included in completion tokens and are not billed twice.
Missing cache/reasoning counters remain unknown; absent cache counts cause an
uncached upper estimate. Preflight costs are separate from measured costs.
No automatic retry, alternative model, or tuning follows a failure: stop that
arm, retain its evidence, and continue the other frozen arms. An incomplete
arm receives no complete-corpus quality score.

## Reproduce

Python standard library, no extra packages. The original Luna package supplies
the unchanged server-launch helper and exact normalization/matching functions.
All source/harness hashes and the release binary hash are frozen in
`protocol.json`. Code must match before a new live run. This is the already
validated CG-26 release; the protocol identifies its uncommitted source state
on parent revision `3fa7db7` without claiming a clean commit snapshot.

```sh
python3 -B -m unittest discover -s fixtures/semantic-neurons/cross-provider-baseline-2026-09-09 -p 'test_*.py'
python3 -B fixtures/semantic-neurons/cross-provider-baseline-2026-09-09/run.py --mode mock --output /tmp/cross-provider-control-new
python3 -B fixtures/semantic-neurons/cross-provider-baseline-2026-09-09/evaluate.py /tmp/cross-provider-control-new --output /tmp/cross-provider-control-results.json
# Live execution spends the frozen request budget. Use a new output directory.
python3 -B fixtures/semantic-neurons/cross-provider-baseline-2026-09-09/run.py --mode live --output /tmp/cross-provider-live-new
python3 -B fixtures/semantic-neurons/cross-provider-baseline-2026-09-09/evaluate.py /tmp/cross-provider-live-new --output /tmp/cross-provider-live-results.json
```

Stored-output scoring needs no keys or network access. It uses the same distinct
`(chunk, source, relation, target)` counting unit and exact normalized-name
matching as the original baseline. Every missing gold triple contributes one
false negative; partial category coverage never hides misses. Full occurrence
rows retain evidence bytes and provider/model attribution. Future generations
may differ and provider model aliases do not identify immutable weights.

`verify.py --output /tmp/cross-provider-validation.json` runs the post-capture
integrity audit and local credential scan, without provider calls. Repeatable
`--snapshot PATH` arguments can check earlier file-hash maps. The post-capture
verifier has its own hash in `validation.json`; it is outside the frozen
pre-execution runner/scorer set.
