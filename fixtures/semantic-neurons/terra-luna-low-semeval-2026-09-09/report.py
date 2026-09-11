"""Render measured results; interpretation is documented after the fixed run."""
import json
from common import ROOT, save


def pct(value):
    return f'{100*value:.2f}%'


def money(low, high):
    return f'${high:.6f}' if abs(low-high) < 1e-12 else f'${low:.6f}–${high:.6f}'


def main():
    comparison = json.loads((ROOT/'comparison.json').read_text())
    validation = json.loads((ROOT/'validation.json').read_text())
    result = json.loads((ROOT/'results.json').read_text())
    protocol = json.loads((ROOT/'protocol.json').read_text())
    arms, costs = comparison['arms'], comparison['costs']
    calls = json.loads((ROOT/'live/provider-calls.json').read_text())
    failures = []
    for call in calls:
        ctx = call['context']
        raw = json.loads(call['raw_response'])
        observation = json.loads((ROOT/'live'/f"{ctx['arm']}-r{ctx['repetition']}-b{ctx['batch']}.json").read_text())
        finish = raw.get('choices', [{}])[0].get('finish_reason')
        config = next(a for a in protocol['arms'] if a['id'] == ctx['arm'])
        if observation['status'] != 200 or call['status'] != 200 or finish != 'stop' or raw.get('model') not in config['accepted_returned_models']:
            failures.append({'arm': ctx['arm'], 'phase': ctx['phase'], 'repetition': ctx['repetition'], 'batch': ctx['batch'],
                             'server_http': observation['status'], 'recorder_http': call['status'],
                             'finish_reason': finish, 'server_response': observation['response'],
                             'usage_available': 'usage' in raw, 'committed_facts': len(observation['facts'])})
    save(ROOT/'operational-failures.json', failures)
    names = {'luna-none': 'Luna none (historical)', 'glm-flash-low': 'GLM Flash low (historical)',
             'luna-low': 'Luna low', 'terra-low': 'Terra low'}
    best = max(arms, key=lambda n: arms[n]['pooled']['raw']['accuracy'])
    lines = ['# Terra low and Luna low — captured SemEval results', '',
             f"**{names[best]} has the highest observed raw accuracy: {pct(arms[best]['pooled']['raw']['accuracy'])}.** "
             'This is supplied-pair classification on a public benchmark. Production defaults remain unchanged.', '',
             'The useful next comparison is **Luna low versus GLM Flash on representative documents**, with Terra low retained '
             'as a stronger reference. Terra gains 2.33 percentage points over Luna low (paired interval +0.42 to +4.25), '
             'but costs about 10.4 times as much and takes about 42% longer per batch here. That small quality margin does '
             'not justify promoting Terra to the economical baseline on this evidence.', '',
             'Luna low is the main finding: it gains 27.42 points over the historical Luna none capture, reduces direction '
             'errors from 136 to 1, and reduces repeat disagreements from 181 to 71. Its measured token estimate is about '
             '1.74 times the corrected historical Luna none estimate, with 2.27 times the HTTP latency. This historical '
             'contrast strongly motivates qualification of low effort, but is not a randomized causal experiment. '
             'Luna low leads GLM by 4.33 observed points, with an interval spanning −1.83 to +10.00; that comparison does '
             'not establish a clear winner. GLM remains cheaper on reported usage. All three candidates assert a relation '
             'on more than half of the repeated Other observations, so restraint remains a qualification blocker.', '',
             'The two new configurations completed the frozen 600-case, two-repetition schedule through real release-build '
             'CogniGraph servers. Each configuration has 1,200 observations over 600 distinct sentences. '
             '[Protocol](protocol.json), [scope and provenance](README.md), [full new scores](results.json), '
             '[four-configuration comparison](comparison.json), and [verification](validation.json) retain the evidence.', '',
             '| Configuration | Raw accuracy run 1 / run 2 | Pooled raw accuracy | Nine-relation macro F1 | False assertions on Other / 220 | Direction errors |',
             '|---|---:|---:|---:|---:|---:|']
    for name, arm in arms.items():
        raw = arm['pooled']['raw']
        reps = arm['repetitions']
        lines.append(f"| {names[name]} | {pct(reps[0]['raw']['accuracy'])} / {pct(reps[1]['raw']['accuracy'])} | {pct(raw['accuracy'])} | {pct(raw['macro_f1_nine_relations'])} | {raw['false_assertions_on_other']} | {raw['direction_errors']} |")
    lines += ['', 'Other counts concern 110 negative sentences repeated twice. Failed cases are always wrong for primary accuracy '
              'and are listed separately; they do not count as either correct abstentions or false assertions. '
              'Endpoint direction must match the original human label. No labels, prompts, gates or scoring rules were tuned.', '',
              '## Paired accuracy contrasts', '',
              '| Contrast (right minus left) | Raw difference, percentage points | Paired 95% interval | Capture relationship |',
              '|---|---:|---:|---|']
    for contrast in comparison['contrasts'].values():
        raw = contrast['raw']
        low, high = raw['paired_batch_bootstrap_95_percentile']
        lines.append(f"| {names[contrast['right']]} − {names[contrast['left']]} | {100*raw['right_minus_left_accuracy']:+.2f} | [{100*low:+.2f}, {100*high:+.2f}] | {'Historical secondary' if contrast['historical_contrast'] else 'Interleaved primary'} |")
    lines += ['', 'Intervals use 10,000 bootstrap resamples of the 50 paired batches, keeping both models and repetitions '
              'together. Two repetitions do not double independent sample size. The primary comparison is Terra low versus '
              'Luna low. Historical contrasts are exploratory, without adjustment for multiple comparisons; capture time, '
              'serving conditions and cache history differ. They do not isolate a randomized causal effect of reasoning effort.', '',
              '## Reliability, latency and costs', '',
              '| Configuration | Successful measured calls / 100 | FAILED cases / 1,200 | Repeat disagreements / 600 | Mean / p95 HTTP per 12-case batch | Known measured cost at regular tariffs |',
              '|---|---:|---:|---:|---:|---:|']
    for name, arm in arms.items():
        cost = costs[name]
        estimate = money(cost['regular_known_measured_lower_usd'], cost['regular_known_measured_upper_usd'])
        if cost['measured_calls_without_usage']:
            estimate += f" + {cost['measured_calls_without_usage']} unknown call(s)"
        lines.append(f"| {names[name]} | {arm['counts']['successful_calls']} | {arm['pooled']['raw']['failed_cases']} | {arm['repeat_disagreements']['raw']} | {arm['http_latency_mean_seconds']:.3f}s / {arm['http_latency_p95_seconds']:.3f}s | {estimate} |")
    known_low = sum(costs[name]['known_schedule_lower_usd'] for name in result['arms'])
    known_high = sum(costs[name]['known_schedule_upper_usd'] for name in result['arms'])
    unknown = sum(costs[name]['calls_without_usage'] for name in result['arms'])
    lines += ['', f"The **new 202-call schedule**, including both preflights, has a known token estimate of **{money(known_low, known_high)}**; "
              f"{unknown} call(s) have no usage record. The conservative reservation was $13.587759, below the $20 cap. "
              'This is not a billing invoice. The [accounting ledger](accounting.json) preserves raw usage counters, pricing calculations, '
              'cache uncertainty and historical corrections. Latency includes the real construct HTTP path and failed requests; '
              'it is an observed batch measurement, not a single-sentence or production throughput forecast.', '',
              'New execution failures: ' + ('; '.join(
                  f"{f['arm']} run {f['repetition']} batch {f['batch']} (server {f['server_http']}, recorder {f['recorder_http']}, finish {f['finish_reason']})"
                  for f in failures) if failures else 'none') + '. '
              'See the [failure ledger](operational-failures.json). No retry or completion repair was performed. '
              'Recorder 502, if present, denotes a local transport failure without an upstream response. '
              'The earlier GLM/Luna schedule retains its separate four Luna content-filter failures, one GLM schema failure and one GLM timeout.', '',
              'OpenAI costs now include cache-write tokens at the published 1.25× ordinary-input tariff. '
              'Input, cache-read and cache-write buckets are disjoint; reasoning tokens are already included in output usage. '
              'Missing cache counters produce a bounded estimate rather than invented zero usage. '
              'The historical Luna report used an older helper that omitted the write premium; its original files remain unchanged. '
              'GLM is repriced at regular rather than temporary promotional rates, using the same observed usage; its timeout remains unknown. '
              'These are observed-cache estimates, not cold-cache forecasts. '
              '[OpenAI pricing](https://developers.openai.com/api/docs/pricing), '
              '[cache accounting](https://developers.openai.com/api/docs/guides/prompt-caching), and '
              '[frozen GLM tariff evidence](../glm-luna-semeval-2026-09-09/protocol.json) document the assumptions.', '',
              '## Token usage', '',
              '| Configuration | Known input tokens | Known output tokens, including reasoning | Known reasoning tokens | Known cached-input tokens | Known cache-write tokens |',
              '|---|---:|---:|---:|---:|---:|']
    for name in arms:
        counters = costs[name]['measured_token_counters']
        def counted(key):
            item = counters[key]
            return f"{item['known_total']:,}" + (f" (+ {item['unreported_calls']} unreported calls)" if item['unreported_calls'] else '')
        lines.append(f"| {names[name]} | " + ' | '.join(counted(k) for k in ('prompt_tokens', 'completion_tokens', 'reasoning_tokens', 'cached_input_tokens', 'cache_write_tokens')) + ' |')
    lines += ['', 'These are measured-call totals only. Unreported counters are not measured zeroes. '
              'GLM cache-write counts are not exposed in these captures and are not a separate component of its frozen tariff. '
              'Reasoning tokens must not be added to output usage a second time.', '',
              '## Gates and matched-success sensitivity', '',
              '| Configuration | Post-gate accuracy | Post-gate macro F1 | Post-gate false assertions on Other / 220 | Accepted occurrences |',
              '|---|---:|---:|---:|---:|']
    for name, arm in arms.items():
        accepted = arm['pooled']['accepted']
        lines.append(f"| {names[name]} | {pct(accepted['accuracy'])} | {pct(accepted['macro_f1_nine_relations'])} | {accepted['false_assertions_on_other']} | {arm['counts']['accepted_occurrences']} |")
    sensitivity = comparison['sensitivity']
    lines += ['', 'The finite vocabulary gates were frozen in the previous experiment. A correct full-excerpt gold-proposal '
              'control loses 34 of 490 positive facts per repetition, leaving 94.33% overall accuracy for that evidence form. '
              'This is a control reference, not a universal maximum or proof of semantic correctness. No gate was widened after seeing results.', '',
              f"The prespecified four-way matched-success sensitivity removes {len(sensitivity['excluded_ids'])} sentences with any FAILED "
              f"observation, leaving {sensitivity['remaining_unique_sentences']} matched sentences in both repetitions. Raw accuracies are " +
              '; '.join(f"{names[name]} {pct(stats['accuracy'])}" for name, stats in sensitivity['arms'].items()) +
              '. This conditional analysis does not replace the primary results that include failures.', '',
              'The [illustrative errors](illustrative-errors.json) select the first run-one case in each fixed category: '
              'Terra correct/Luna low wrong, the converse, Terra correct/GLM wrong, all four wrong on complete responses, '
              'and a Terra false assertion on Other. These are explanatory examples, not a new scoring subset or corrected labels.', '',
              '## Verification and limits', '',
              f"Ten unit tests and 202 gold-proposal controls passed. Exact new and historical replay, all 404 outgoing prompt comparisons, "
              f"600 source-gold checks, deterministic resampling, eight new official Perl scorer comparisons, "
              f"{validation['accepted_occurrences_verified']} new evidence-span/attribution checks and five tamper-rejection probes passed. "
              f"All {validation['code_files_verified']} pinned code/harness files, the original release binary and all 248 inherited package files match their hashes. "
              'The comparison makes no Rust source changes; earlier Rust validation remains tied to that binary and source checkpoint.', '',
              'Published human labels provide an independent reference, but the public 2010 corpus may have appeared in training. '
              'The task supplies two common-noun arguments and a benchmark-only pair instruction. It does not establish entity discovery, '
              'exhaustive document extraction, unseen customer-domain accuracy or judge qualification. Keep the economical Luna baseline '
              'until representative, independently checked documents establish an acceptable quality/cost tradeoff. DeepSeek V4 Flash remains retired.', '']
    (ROOT/'results.md').write_text('\n'.join(lines))


if __name__ == '__main__':
    main()
