"""Render the captured comparison without changing labels, prompts or scores."""
import json
from collections import Counter
from common import ROOT, save, usage_cost
from evaluate import classification


def pct(x):return f'{100*x:.2f}%'

def main():
    r=json.loads((ROOT/'results.json').read_text());v=json.loads((ROOT/'validation.json').read_text())
    protocol=json.loads((ROOT/'protocol.json').read_text());arms=r['arms']
    live_calls=json.loads((ROOT/'live/provider-calls.json').read_text())
    failures=[]
    for call in live_calls:
        ctx=call['context'];raw=json.loads(call['raw_response'])
        obs=json.loads((ROOT/'live'/f"{ctx['arm']}-r{ctx['repetition']}-b{ctx['batch']}.json").read_text())
        finish=raw.get('choices',[{}])[0].get('finish_reason')
        if obs['status']!=200 or finish!='stop':
            failures.append({'arm':ctx['arm'],'phase':ctx['phase'],'repetition':ctx['repetition'],'batch':ctx['batch'],
                'server_http':obs['status'],'recorder_http':call['status'],'finish_reason':finish,'server_response':obs['response'],
                'usage_available':'usage' in raw,'committed_fact_count':len(obs['facts'])})
    save(ROOT/'operational-failures.json',failures)
    # Descriptive sensitivity only, added after a content-filter failure was observed.
    # Primary endpoint and full-denominator scores stay unchanged.
    excluded={row['id'] for arm in arms.values() for row in arm['case_ledger'] if row['raw']=='FAILED'}
    relations=[x['relation'] for x in json.loads((ROOT/'corpus.json').read_text())['taxonomy']]
    sensitivity={'pre_registered':False,'policy':'Remove any sentence with a FAILED observation for either model in either repetition; retain matched remaining sentences and both repetitions.',
        'excluded_sentence_ids':sorted(excluded),'remaining_sentences':600-len(excluded),
        'models':{name:classification([row for row in arm['case_ledger'] if row['id'] not in excluded],'raw',relations) for name,arm in arms.items()}}
    save(ROOT/'sensitivity.json',sensitivity)
    first_calls=json.loads((ROOT/'attempt-1/live/provider-calls.json').read_text())
    first_ledger=[]
    for call in first_calls:
        raw=json.loads(call['raw_response']);arm=next(a for a in protocol['arms'] if a['id']==call['context']['arm'])
        first_ledger.append({'arm':arm['id'],'phase':call['context']['phase'],'batch':call['context']['batch'],
            'status':call['status'],'accounting':usage_cost(raw['usage'],arm,call['started_utc']) if 'usage' in raw else None})
    first_known=sum(x['accounting']['estimated_usd'] for x in first_ledger if x['accounting'])
    total_unknown=sum(a['counts'].get('calls_without_usage',0) for a in arms.values())+sum(x['accounting'] is None for x in first_ledger)
    save(ROOT/'attempt-1/accounting.json',{'calls':first_ledger,'known_usage_estimated_usd':first_known,'calls_without_usage':sum(x['accounting'] is None for x in first_ledger),
        'quality_ranked':False,'note':'Incomplete operational attempt; provider timeout has unknown usage. No gold or quality output inspection before the restart.'})
    delta=r['paired_comparison']['raw'];ci=delta['paired_batch_bootstrap_95_percentile']
    if ci[0]>0:
        finding='GLM Flash leads Luna on this supplied-pair benchmark; the paired accuracy interval stays above zero.'
    elif ci[1]<0:
        finding='Luna leads GLM Flash on this supplied-pair benchmark; the paired accuracy interval stays below zero for GLM minus Luna.'
    else:
        finding='The paired accuracy interval overlaps zero; this trial does not establish a clear accuracy winner.'
    lines=['# GLM Flash versus Luna — captured independent-label results','',finding,
        'Keep Luna as the production baseline pending representative document qualification. DeepSeek V4 Flash is retired.','',
        'GLM is the stronger candidate for the next domain trial. Its remaining restraint errors matter: it asserted a relation on 113 of 220 Other observations; Luna did so on 175. After gating, those counts were still 88 and 131. These counts concern 110 distinct negative sentences repeated twice. GLM also took longer per batch. The result supports candidate prioritization, not unattended fact acceptance.','',
        'The trial uses 600 distinct SemEval-2010 sentences with published human labels: 490 positive nominal pairs and 110 Other pairs. Two repetitions produce 1,200 observations per model, not 1,200 independent examples. The [protocol](protocol.json), [source and limits](README.md), [full results](results.json), [data-quality notebook](data-quality.ipynb) and [verification](validation.json) make the comparison reproducible.','',
        '| Model/configuration | Raw accuracy, run 1 / run 2 | Pooled nine-relation macro F1 | False assertions on Other, run 1 / run 2 | Mean / p95 batch HTTP | Known measured token cost | Known usage at regular prices |',
        '|---|---:|---:|---:|---:|---:|---:|']
    for name,a in arms.items():
        reps=a['repetitions']; pooled=a['pooled']['raw']
        lines.append(f"| {a['model']} ({a['reasoning_effort']}) | {pct(reps[0]['raw']['accuracy'])} / {pct(reps[1]['raw']['accuracy'])} | {pct(pooled['macro_f1_nine_relations'])} | {reps[0]['raw']['false_assertions_on_other']} / {reps[1]['raw']['false_assertions_on_other']} of 110 | {a['http_latency_mean_seconds']:.3f}s / {a['http_latency_p95_seconds']:.3f}s | ${a['estimated_measurement_usd']:.6f} | ${a['regular_price_measurement_usd']:.6f} |")
    lines += ['',f"GLM minus Luna raw accuracy: **{100*delta['glm_minus_luna_accuracy']:+.2f} percentage points**, paired 95% percentile interval **[{100*ci[0]:+.2f}, {100*ci[1]:+.2f}]**. Ten thousand bootstrap samples resample the 50 paired batches while carrying both repetitions and both models together. GLM alone was correct on {delta['glm_only_correct_observations']} observations; Luna alone on {delta['luna_only_correct_observations']}. This is sample uncertainty, not a bound on unseen domains or pretraining contamination.", '',
        '| Model | Raw correct / 1,200 | Direction errors | Raw repeat disagreements / 600 | Accepted accuracy | Accepted macro F1 | Nominations / accepted occurrences |',
        '|---|---:|---:|---:|---:|---:|---:|']
    for name,a in arms.items():
        raw=a['pooled']['raw'];acc=a['pooled']['accepted'];c=a['counts']
        lines.append(f"| {a['model']} | {raw['correct']} | {raw['direction_errors']} | {a['repeat_disagreements']['raw']} | {pct(acc['accuracy'])} | {pct(acc['macro_f1_nine_relations'])} | {c['nominations']} / {c['accepted_occurrences']} |")
    gate=r['paired_comparison']['accepted'];gci=gate['paired_batch_bootstrap_95_percentile']
    lines += ['',f"After gates, GLM minus Luna accuracy is {100*gate['glm_minus_luna_accuracy']:+.2f} points, interval [{100*gci[0]:+.2f}, {100*gci[1]:+.2f}]. The finite vocabulary policy rejected 34 of 490 correct relations in the gold-proposal control, leaving 94.33% overall accuracy for that exact full-excerpt evidence form. This is a gold-proposal reference, not a proven maximum over every possible evidence quote. These losses are a separate policy limitation; no vocabulary was tuned on measured output. Evidence-span validation does not establish relation correctness.", '',
        'Three [illustrative errors](illustrative-errors.json) retain the first run-one example in each explicitly named category: GLM-only correct, both wrong, and a GLM false assertion on Other. They are explanatory examples, not a new scoring subset or relabelled gold.','',
        '## Relation breakdown','', '| Relation | Unique gold pairs | Luna raw F1 | GLM Flash raw F1 |','|---|---:|---:|---:|']
    corpus=json.loads((ROOT/'corpus.json').read_text());cats=Counter(c['category'] for c in corpus['cases'])
    for rel in [x['relation'] for x in corpus['taxonomy']]:
        lines.append(f"| {rel} | {cats[rel]} | {pct(arms['luna-json']['pooled']['raw']['by_relation'][rel]['f1'])} | {pct(arms['glm-flash']['pooled']['raw']['by_relation'][rel]['f1'])} |")
    lines += ['', '## Operational evidence and cost','',
        'Recorded execution failures: ' + ('; '.join(f"{f['arm']} run {f['repetition']} batch {f['batch']}: recorder HTTP {f['recorder_http']}, finish reason {f['finish_reason']}, server HTTP {f['server_http']}" for f in failures) if failures else 'none') + '. See the [failure ledger](operational-failures.json). Luna had four content-filtered batches; GLM had one schema-invalid response missing target_type and one transport timeout. CogniGraph committed no facts for any failed batch.', '',
        f"A descriptive [sensitivity check](sensitivity.json), added after observing an execution failure, removes all {len(excluded)} affected sentences from both models and both repetitions. On the remaining {sensitivity['remaining_sentences']} matched sentences, raw accuracy is " + '; '.join(f"{arms[name]['model']} {pct(stats['accuracy'])}" for name,stats in sensitivity['models'].items()) + '. This is not the preregistered primary endpoint and does not replace the full-denominator results above.', '',
        f"The completed schedule contains 202 calls: two train-only preflights and 200 measured requests. Success counts: " + '; '.join(f"{a['model']} {a['counts']['successful_calls']}/100 measured calls" for a in arms.values()) + '. Failed or malformed cases count as FAILED, never successful abstention. No per-call retry or completion repair was used.', '',
        'An earlier attempt stopped after five provider calls. Four completed observations were captured; the fifth was a GLM transport timeout after 110 seconds, and a recorder bug then lost the server HTTP observation while querying absent collections. The untouched [first attempt](attempt-1/live/manifest.json), original protocol, runner and progress log are retained. The recorder was corrected to persist HTTP before store queries. The full trial was restarted once with identical data, prompts, models and scoring; no quality outputs were inspected or used for tuning before restart. The amendment is recorded in the current protocol.', '',
        f"Known token estimates: completed schedule **${r['estimated_total_usd']:.6f}** including its preflights; stopped attempt **${first_known:.6f}**; combined known usage **${r['estimated_total_usd']+first_known:.6f}**. **{total_unknown} call(s) have unknown usage**, so the combined amount is incomplete, not an exact total or evidence those calls were free. The conservative maximum reservation for both schedules was $1.786226, below the $5 cap.", '',
        'GLM promotional pricing expires September 9, 2026 at 16:00 UTC. The regular-price column recalculates identical observed tokens at the ordinary tariff, including reported cached input; it is not a cold-cache forecast. The stopped attempt may have warmed the preflight and earliest batch prompts. Costs come from provider token counters and published rates, not invoices, and omit taxes, cache-storage charges and unreported adjustments. See [Z.ai pricing](https://docs.z.ai/guides/overview/pricing) and [Luna pricing](https://developers.openai.com/api/docs/models/gpt-5.6-luna).', '',
        '## Verification and interpretation','',
        f"Six scoring/adapter tests passed. The amended harness passed 202 gold-proposal HTTP controls plus two explicit timeout/missing-collection controls; the earlier harness also completed 202 gold controls before exposing that failure path. The local scorer agrees with SemEval's original Perl scorer on gold, post-gate, failed-case controls, and all eight measured model/repetition/stage combinations. Exact replay, {v['accepted_occurrences_verified']} stored evidence-span/attribution checks, {v['code_files_verified']} pinned code files, 600 source-gold row checks and five negative integrity probes passed. The source and release binary match the earlier validated Rust checkpoint; this trial changes no Rust source.", '',
        'The [original dataset paper](https://aclanthology.org/S10-1006/) describes independent human annotation and adjudication. This trial retains those labels without an LLM judge. It supplies the two nominal arguments, covers a public 2010 benchmark and uses a benchmark-specific pair instruction. It therefore supports a model comparison on this task; it does not establish entity discovery, exhaustive extraction, customer-document accuracy, judge qualification or production readiness. A new representative document holdout with independently checked, exhaustive labels remains necessary before changing the product default.', '']
    (ROOT/'results.md').write_text('\n'.join(lines))

if __name__=='__main__':main()
