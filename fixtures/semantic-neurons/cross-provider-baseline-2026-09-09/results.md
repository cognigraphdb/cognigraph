# DeepSeek and GLM comparison — results

**GLM-5.3-Flash is the leading candidate for the next representative-document
trial.** On this synthetic sample it recovered all gold triples twice, with
latency close to Luna and lower estimated token cost even at regular pricing.
Keep Luna as the current default while that independent evaluation is prepared.

The [frozen protocol](protocol.json), [method and limitations](README.md),
[count/occurrence ledger](results.json), [provider/server captures](live/manifest.json)
and [integrity checks](validation.json) document the result. These are evaluation
adapter runs through the real server, not native DeepSeek/Z.ai provider integration.

## Same-format comparison

Each complete arm made eight measured calls: two runs over the same 48 excerpts,
32 gold triples and 20 negative excerpts. All arms used JSON-object mode and the
same schema appendix to the original system prompt. Preflights are excluded
from quality and latency metrics and costed separately.

| Configuration | Run 1 TP/FP/FN | Run 2 TP/FP/FN | Mean HTTP batch latency | Estimated measured token cost |
|---|---|---|---:|---:|
| Luna, none | 31 / 0 / 1 | 31 / 1 / 1 | 3.710 s | $0.00467520 |
| DeepSeek V4 Pro, low | 32 / 0 / 0 | 32 / 0 / 0 | 19.964 s | $0.03379746 |
| GLM-5.3-Flash, low | 32 / 0 / 0 | 32 / 0 / 0 | 4.165 s | $0.00123481 promotional; $0.00246962 at regular rates |
| GLM-5.3, low | 32 / 0 / 0 | 32 / 0 / 0 | 17.160 s | $0.03034416 |
| DeepSeek V4 Flash, low | Incomplete: first measured batch truncated | Not run | No full-run average | $0.00285340 for the failed measured request only |

Luna's recall was 96.875% in each run; precision was 100% and 96.875%, and F1
was 98.413% and 96.875%. The three completed non-Luna configurations scored
100% precision/recall/F1 on these labels. All four complete arms abstained on
every negative excerpt in both runs. No live nomination was rejected by the
grounding gates; before/after scores match. Luna's wrong-direction fact still
passed those textual gates.

The fresh Luna run differs from the earlier strict-schema experiment: it found
31 triples twice, versus 27 and 26 previously. The format/prompt configuration
changed and outputs are stochastic; the experiments do not isolate a causal
format effect. Compare these providers primarily against the fresh Luna row,
not against an earlier weaker result. No corpus, gold, or task-description
tuning was performed after observing provider results.

## Why DeepSeek Flash is unranked

The one-excerpt preflight succeeded. Its first twelve-excerpt batch then used
the entire 4,096-token output allowance: the API reported 3,586 reasoning
tokens and returned `finish_reason: length`. The answer JSON was cut off.
CogniGraph returned HTTP 500 with a completion-JSON parsing error. This is
recorded as a capacity/parsing failure, not a complete-corpus false-negative
count or proof of poor extraction capability.

The frozen stop policy prevented retries, JSON repair, model substitution, or
changing effort/output limits mid-run. Seven planned Flash measurement calls
were not made. The original request, full provider response and server error
remain in the capture. The failed observation's empty fact/entity/chunk arrays
mean they were not queried after failure; they are not an independent claim
that storage was empty or unchanged.

A separate follow-up can test Flash with thinking disabled, which
[DeepSeek supports](https://api-docs.deepseek.com/guides/thinking_mode/), or
with more output headroom. It must retain this failed configuration and be
labelled separately. That additional configuration was not executed here.

## Cost, caching and timing

| Measured token use | Input | Cached input | Completion, including reasoning | Reported reasoning |
|---|---:|---:|---:|---:|
| Luna | 5,688 | 0 | 2,948 | 0 |
| DeepSeek Pro | 5,982 | 3,968 | 16,354 | 11,690 |
| GLM Flash | 6,224 | 2,304 | 3,625 | 533 |
| GLM-5.3 | 6,224 | 5,056 | 6,226 | 3,154 |

The completed GLM Flash arm cost 26.41% of Luna's measured cost at promotional
rates, or **52.82% at regular rates with the same token/cache counts**. Mean
HTTP latency was 12.26% higher. This is the strongest observed cost/accuracy
tradeoff in the sample; model order, provider load, caching, and unequal
reasoning compute limit any broader latency claim.

All DeepSeek calls fell in the documented off-peak window. GLM Flash's
[$0.075 input / $0.25 output promotion](https://docs.z.ai/guides/overview/pricing)
ends at **16:00 UTC on September 9, 2026**. Regular rates are $0.15/$0.50;
the table explicitly includes both estimates. The
[DeepSeek time-dependent rates](https://api-docs.deepseek.com/quick_start/pricing/)
and [Luna standard rates](https://developers.openai.com/api/docs/pricing) are
frozen in the protocol. Estimates use actual reported cache hits; reasoning
tokens are included once within completion tokens. Taxes, cache storage,
credits and unreported billing adjustments are not reconstructed.

Five successful preflights cost an estimated $0.002657045 total. Including
them and the failed Flash call, **all 38 provider requests cost an estimated
$0.075562071**. The conservative admission reservation was $0.59997573,
within $5. No retries or new calls followed scoring. These are token estimates,
not invoice evidence or promises of future pricing.

## Validation and decision boundary

- Six new tests passed for UTC pricing boundaries, cache accounting, missing
  counters, reasoning double-count prevention, equal prompt/schema adaptation,
  wrong direction and consistent false-negative grain.
- Forty-five synthetic HTTP controls completed through the exact release
  server; every control run reproduced 32/32. The [control ledger](controls.json)
  is retained; original temporary control captures can be regenerated.
- Offline scoring reproduced `results.json` byte for byte. All 255 accepted
  measured occurrence rows passed canonical UTF-8 span, entity, space,
  occurrence-schema, and model-attribution checks. Preflight rows are excluded.
- Five negative integrity probes reject tampered captures, interrupted runs,
  missing attempts, wrong model IDs, and invalid evidence spans. Frozen source
  and binary hashes, documentation links, all 33 earlier baseline files,
  user-owned drafts, and `.env` preservation passed verification.

All providers returned their requested model aliases. DeepSeek's documentation
identified Flash-0731 and Pro-0813 at lookup time; aliases and fingerprints are
not independently immutable weight attestations. The old Luna/Astra package,
Rust source, model defaults, judge qualification and UI remain unchanged. The
existing CG-26 Rust gates were not rerun because the evaluated source/binary
hashes still match that validated state. All spawned servers were stopped.

The small, Codex-authored corpus has no independent annotator, shared templates,
and short excerpts. Perfect scores here do not establish customer accuracy or
safe automatic review. Next priorities are an independently labelled trial
centred on GLM Flash versus Luna, and a separately captured DeepSeek Flash
configuration that leaves enough capacity for a complete answer.
