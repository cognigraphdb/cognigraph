# Captured Luna baseline and Astra reference — results

Keep Luna as the economical default and Astra as a useful reference candidate.
Astra recovered every labelled triple in both runs; Luna had a repeatable
direction error and missed facts after distractor sentences. This is evidence
about the frozen synthetic sample, not a general model ranking.

The [protocol and limitations](README.md), [frozen settings](protocol.json),
[count/occurrence ledger](results.json), and [capture manifest](live/manifest.json)
make the result inspectable and offline scoring reproducible.

## Results from sixteen real HTTP/provider calls

Each run contains the same 48 excerpts, 32 gold triples and 20 negative
excerpts. Rates below use distinct directed triples within each chunk.

| Configuration | Run | TP | FP | FN | Precision | Recall | F1 |
|---|---:|---:|---:|---:|---:|---:|---:|
| Luna, reasoning none | 1 | 27 | 1 | 5 | 96.43% | 84.38% | 90.00% |
| Luna, reasoning none | 2 | 26 | 1 | 6 | 96.30% | 81.25% | 88.14% |
| Astra, reasoning medium requested | 1 | 32 | 0 | 0 | 100% | 100% | 100% |
| Astra, reasoning medium requested | 2 | 32 | 0 | 0 | 100% | 100% | 100% |

No model nominated a relation for any of the 20 negative excerpts in either
run, including the four embedded-instruction cases. All nominations passed
the deterministic gates, so before/after metrics are identical. There were
no duplicate live nominations or accepted occurrences. Zero gate rejections
does not prove zero semantic errors: Luna's reversed relation passed every
textual evidence check.

| Observed resource use | Luna, eight calls | Astra, eight calls |
|---|---:|---:|
| Input tokens | 5,392 | 5,392 |
| Output tokens | 2,603 | 2,989 |
| Reported cached input / reasoning tokens | 0 / 0 | 0 / 0 |
| Mean HTTP latency per twelve-excerpt batch | 3.725 s | 6.734 s |
| Estimated standard token cost | $0.004202 | $0.203370 |
| HTTP/provider errors, truncations, retries | 0 | 0 |

Astra cost 48.40 times as much and mean HTTP latency was 1.81 times Luna's.
The combined estimated token cost was **$0.207572**. Conservative pre-request
reservations totalled $2.3075904 within the $5 budget. Rates came from the
[official standard pricing table](https://developers.openai.com/api/docs/pricing)
checked on 2026-09-09. This is usage-based estimation, not invoice evidence;
all responses reported service tier `default` and zero cache writes.

Every response returned the requested model ID. Astra requests explicitly
specified medium effort, but the API reported zero reasoning tokens; this
report records the requested setting without inferring internal reasoning.
Luna's outgoing setting was none. Request/response IDs and fingerprints are
retained, but no immutable model-weight snapshot is exposed. Model order was
fixed rather than interleaved, and these short requests are not a production
latency benchmark.

## Concrete differences

- In `case-026`, "Cipher is required by Delta" means
  `Delta —DEPENDS_ON→ Cipher`. Luna asserted the reverse in both runs.
  Source names, the vocabulary, and the quotation were valid, illustrating
  why the current deterministic grounding gates do not prove direction.
- Luna missed `case-009`, `case-021`, `case-033`, and `case-045` in both runs.
  Each puts a positive fact after an unrelated first sentence. Astra found all
  four. These observations motivate a larger mixed-sentence test; they do not
  establish a universal first-sentence limitation.
- Luna additionally missed `Fern —OWNS→ Ember` from "Ember is a subsidiary of
  Fern" in its second run. Astra recovered it in both runs.

All 119 accepted occurrence rows across the four runs passed byte-span,
stored-entity, occurrence-schema, and model-attribution checks. Gold labels
were frozen before execution; no prompt, taxonomy, code or corpus tuning was
performed after observing results. This is not a signed promotion attestation
or independent adjudication of the synthetic labels.

## Validation and follow-up

Six scorer regression tests passed, including consistent false-negative grain,
wrong direction/chunk, Unicode normalization, and prevention of substring
credit. A separate sixteen-request loopback control through the same release
server yielded the expected 32/32 triples in every run. An additional
[gate control](controls/gate-rejection.json) retained four raw nominations,
one accepted fact, two rejection messages, and one suppressed duplicate.
These controls are synthetic provider responses, excluded from model metrics.
The [loopback scoring ledger](controls/mock-results.json) is retained; the
sixteen original loopback capture files were temporary and can be regenerated
with the documented mock command.

Offline scoring reproduces [results.json](results.json) byte for byte. The
[validation record](validation.json) verifies request parity, artifact/source
hashes, rejected corrupt captures, user-file preservation, and documentation
links. No Rust source, default-model configuration, judge policy, UI, or
publication export changed in this step. The evaluated release is the exact
CG-26 binary whose Rust gates and runtime checks already passed.

The first post-capture verifier compared Python tuple identities directly with
their JSON array representation and failed. The verifier was corrected to
compare exact serialized bytes. The frozen scorer, corpus, protocol, provider
captures and results did not change; no model request was repeated.

Next, independently label representative longer documents and include these
failure patterns. Use Astra as a comparison arm rather than as the sole
source of gold labels. If its advantage persists, test a cheaper intermediate
configuration or explicit escalation policy before changing defaults. Broader
DeepSeek/GLM/other-model comparisons remain a separate recorded follow-up.
