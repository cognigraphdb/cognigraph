# GLM Flash versus Luna: independently labelled supplied-pair trial

> Public reading copy; original research captures and scripts are private.
> Original path: `fixtures/semantic-neurons/glm-luna-semeval-2026-09-09/README.md`.
> Original SHA-256: `93ee3a997d5192913da33592881b3f3177da6f76afaff5bea4d244c593cb765b` (8362 bytes).
> Navigation changed; dated results and limitations retain their original scope.


This package compares `glm-5.3-flash` (thinking low) and `gpt-5.6-luna`
(reasoning none) on 600 public, human-labelled SemEval-2010 Task 8 sentences.
It expands the earlier 48-case synthetic experiment. DeepSeek V4 Flash is
retired by user decision and is not an arm in this or any planned follow-up.
Historical benchmark packages remain unchanged. Luna remains the default.

**Completed:** GLM Flash scored 65.75% pooled accuracy; Luna scored 42.67%.
Read the [measured results and limits](results.md). Both models had substantial
restraint errors on Other cases; Luna remains the product baseline.

Execution and analysis follow the frozen [protocol](../../../evidence/research-glm-luna-semeval-2026-09-09.md#artifact-374c44e808cb051c0fd6).
[Package hashes](../../../evidence/research-glm-luna-semeval-2026-09-09.md#artifact-5f1369a31b4175fc262d) cover the retained artifacts;
[workspace checks](../../../evidence/research-glm-luna-semeval-2026-09-09.md#artifact-cc2b91079093ad81de12) record preservation and credential scanning. The executable
[data-quality notebook](../../../evidence/research-glm-luna-semeval-2026-09-09.md#artifact-fa75cdbc5a5ee50af3da) checks the source-to-sample mapping.

## Source and limits

[Hendrickx et al. (2010)](https://aclanthology.org/S10-1006/) describe collection
by Web search, two independent annotators, and resolution of disagreements.
The nine relations are directed; Other identifies a supplied pair that does
not fit them. These published annotations were neither authored nor relabelled
by Codex, Luna or GLM. Human labels can still contain ambiguity or errors.

The source was released July 16, 2010 under
[CC BY 3.0](https://creativecommons.org/licenses/by/3.0/), as stated in the
[original release notice](../../../evidence/research-glm-luna-semeval-2026-09-09.md#artifact-6e7a950766b88b68fcb8). Attribution: Iris Hendrickx,
Su Nam Kim, Zornitsa Kozareva, Preslav Nakov, Diarmuid Ó Séaghdha, Sebastian
Padó, Marco Pennacchiotti, Lorenza Romano and Stan Szpakowicz.
A pinned [archive mirror](https://github.com/JoelNiklaus/SemEval2010Task8/tree/3ed01ed5cec6731f631b83f3ab5cd3c8532f7e68)
is used for availability; [provenance](../../../evidence/research-glm-luna-semeval-2026-09-09.md#artifact-e4df00c6bceb82f4cb9e) records the
exact URL, commit, archive SHA256 and retained member hashes. The package's
Git attributes preserve source bytes, including original CRLF line endings. This is a mirror,
not an assertion that the authors published an authenticated archive digest.

Mechanical adaptation removes `<e1>`/`<e2>` markup, normalizes NFC, and supplies
the marked nominal strings separately in text order. Original text and labels
remain inspectable in the source files. Source comments are not prompts.
Eligibility excludes 17 train-text overlaps, two repeated test sentences
(keep lowest id), and four pairs with identical nominal strings. Deterministic
SHA256 ranking selects 600 of the remaining 2,694 cases: 490 positives and
110 Other cases, with 600 distinct sentences and no normalized training-text
overlap. [Data checks](../../../evidence/research-glm-luna-semeval-2026-09-09.md#artifact-5cd183106eb99c646eae) record the complete exclusions and
class/direction distribution. No exclusions depend on model output.

This is supplied-pair classification over common nominals, not named-entity
discovery or exhaustive relation extraction. Its public age means training
contamination is unknown for both models. Removing benchmark train/test text
overlap does not remove possible model pretraining exposure. A single gold
label is not an exhaustive list of every fact in the sentence. Do not compare
these scores directly with the earlier synthetic corpus, the retired CUAD
experiment, or a leaderboard using the full SemEval test set.

## Execution

Both models receive identical messages and a shared JSON schema appendix via
an evaluation-only adapter. The production directed endpoint supplies its
usual prompt/schema; the adapter appends the nominal-pair task and converts
strict schema mode to the shared `json_object` mode. It forwards only
id/title/text/pair from the corpus, never gold labels. This adapter is not a
production provider integration. Reusable transport/pricing helpers come from
the frozen prior packages; protocol hashes pin every dependency.

Two repetitions retain the same fifty 12-case batches. Provider order alternates
by batch and reverses in the second repetition. Each request runs through a
fresh release-build server with authenticated HTTP and an isolated Native DB;
all facts/entities/chunks are captured, including after errors. Embeddings are
fixed loopback vectors because retrieval quality is not under test. GLM's low
thinking and Luna's none are economical configurations, not equal compute.
Both have an 8,192-token output cap; provider sampling defaults are retained.

Two train-only preflights precede 200 measured calls. There is no per-call retry, completion repair,
quality-based stopping or prompt retuning. An initial operational attempt was
archived after a provider timeout exposed a capture bug; the full schedule was
restarted once under the protocol's explicit execution amendment, with unchanged
data, prompts, models and scoring. Three consecutive failed requests
halt an arm; failed cases score FAILED, never a correct Other abstention.
The local conservative request reservation cap is $5, using maximum published
rates and no cache assumption. Actual usage estimates are not invoices.
[GLM pricing](https://docs.z.ai/guides/overview/pricing) changes at September 9,
2026 16:00 UTC: regular input/cache/output rates are $0.15/$0.03/$0.50 per
million, twice the promotional rates. Long-term comparisons use regular prices.
[Luna rates](https://developers.openai.com/api/docs/models/gpt-5.6-luna) are
$0.20/$0.02/$1.20. Missing usage remains unknown; reasoning is included in
completion usage, not billed a second time.

## Scoring and verification

Primary: exact directed supplied-pair classification accuracy before gates.
Endpoint matching uses NFC, casefold and whitespace normalization, without
fuzzy matching. A wrong endpoint, unknown relation or multiple facts for a
pair is INVALID. No fact means Other only for a complete successful response.
Secondary: nine-relation directed macro F1, Other restraint, direction errors,
repeat disagreements, latency, and token cost. The full case ledger preserves
all errors. No LLM judge or post-hoc gold correction is used.

Post-gate scores diagnose the frozen hand-written restraint vocabulary and
unchanged Rust evidence writer. That vocabulary can remove correct implicit
relations. We do not broaden it after observing test outcomes or present these
gates as semantic validation. Verified byte spans and provider attribution are
reported separately from correctness.

Paired 95% intervals use 10,000 bootstrap resamples of the 50 batches, carrying
both repetitions and models together. Repetition doubles observations, not the
number of independent examples. Intervals address this sample and protocol,
not unmeasured domains or unknown pretraining contamination.

From the repository root:

```bash
python3 -B -m unittest discover -s fixtures/semantic-neurons/glm-luna-semeval-2026-09-09 -p test_score.py
python3 -B fixtures/semantic-neurons/glm-luna-semeval-2026-09-09/run.py --mode mock --output /tmp/cognigraph-semeval-new-control
python3 -B fixtures/semantic-neurons/glm-luna-semeval-2026-09-09/evaluate.py /tmp/cognigraph-semeval-new-control --output /tmp/cognigraph-semeval-new-control-results.json
```

The initial [attempt](../../../evidence/research-glm-luna-semeval-2026-09-09.md#artifact-b735778122c5f09facc5) retains five provider calls,
including a 110-second GLM timeout with unknown usage. Its fifth server HTTP
observation was lost when the recorder encountered absent collections. The
amended runner saves HTTP first and records collection-query statuses. Two
[explicit failure controls](../../../evidence/research-glm-luna-semeval-2026-09-09.md#artifact-956779d346dd924a4500) verify that path.
Both runner versions completed 202 gold-proposal controls; the current
[control scores](../../../evidence/research-glm-luna-semeval-2026-09-09.md#artifact-6a099f2e558f6e0724b2) and [official-scorer controls](../../../evidence/research-glm-luna-semeval-2026-09-09.md#artifact-662a8d167ee25bf76858)
are retained. Each control run has raw accuracy 100%; the frozen vocabulary
removes 34 correct relations per repetition and leaves 94.33% accepted accuracy.
The stopped attempt may have warmed the preflight and earliest batch prompts;
latency and cost describe observed cache conditions. The combined conservative
reservation of the restarted schedule and stopped
attempt is below $1.79, within the $5 trial cap.

Paid replay requires the existing authorized `OPENAI_API_KEY` and
`ZHIPU_API_KEY`, and a fresh output path. Neither `.env` nor production model
settings are edited. Do not regenerate protocol hashes to conceal code drift.
