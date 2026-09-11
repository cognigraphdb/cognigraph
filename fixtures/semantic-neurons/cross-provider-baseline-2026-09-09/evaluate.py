"""Offline exact-triple evaluation with provider-specific usage and pricing."""
import argparse
from collections import Counter
import json
from pathlib import Path
import statistics
import unicodedata

from shared import ROOT, BASELINE, digest, save, identity, metrics
from providers import adapt, usage_cost


def evaluate(directory):
    protocol = json.loads((ROOT / 'protocol.json').read_text())
    corpus = json.loads((BASELINE / 'corpus.json').read_text())
    manifest = json.loads((directory / 'manifest.json').read_text())
    assert manifest['finished'], 'Interrupted run; no complete comparison'
    assert manifest['protocol_sha256'] == digest(ROOT / 'protocol.json')
    assert protocol['corpus_sha256'] == digest(BASELINE / 'corpus.json')
    for name, expected in manifest['artifact_sha256'].items():
        assert digest(directory / name) == expected, name
    calls = json.loads((directory / 'provider-calls.json').read_text())
    assert len(calls) == len(manifest['attempts'])
    cases = {c['id']: c for c in corpus['cases']}
    gold = {identity(c['id'], f['source'], f['relation'], f['target']) for c in cases.values() for f in c['gold']}
    assert len(gold) == 32
    report = {'mode': manifest['mode'], 'corpus_cases': len(cases), 'gold_triples_per_repetition': len(gold),
        'negative_cases': sum(not c['gold'] for c in cases.values()), 'arms': {}}
    prompt_reference = {}
    for arm in protocol['arms']:
        arm_calls = [c for c in calls if c['context']['arm'] == arm['id']]
        cost_ledger, unknown_usage = [], 0
        for call in arm_calls:
            assert call['request_to_provider'] == adapt(call['request_from_server'], arm, protocol['max_completion_tokens'])
            context = call['context']
            outgoing = call['request_to_provider']
            key = (context['phase'], context['batch'])
            assert outgoing['messages'] == prompt_reference.setdefault(key, outgoing['messages'])
            raw = json.loads(call['raw_response'])
            if 'usage' in raw:
                cost_ledger.append(dict(usage_cost(raw['usage'], arm, call['started_utc']),
                                        phase=context['phase'], repetition=context['repetition'], batch=context['batch']))
            else:
                unknown_usage += 1
        summary = {'model': arm['model'], 'reasoning_effort': arm['reasoning_effort'],
            'provider': arm['provider'], 'execution': manifest['arms'][arm['id']],
            'provider_calls': len(arm_calls), 'usage_ledger': cost_ledger,
            'calls_without_usage': unknown_usage,
            'estimated_measurement_cost_usd': sum(c['estimated_usd'] for c in cost_ledger if c['phase'] == 'measurement'),
            'estimated_preflight_cost_usd': sum(c['estimated_usd'] for c in cost_ledger if c['phase'] == 'preflight'),
            'cost_note': 'Reported token usage at frozen rates by request-start UTC; excludes tax, storage charges and unreported billing adjustments. Missing usage is unknown, not free.',
            'returned_models': sorted({json.loads(c['raw_response']).get('model', '') for c in arm_calls})}
        report['arms'][arm['id']] = summary
        if not summary['execution']['complete']:
            continue
        measurements = [c for c in arm_calls if c['context']['phase'] == 'measurement']
        assert len(measurements) == protocol['repetitions'] * len(cases) // protocol['batch_size']
        assert len(arm_calls) == len(measurements) + 1
        repetitions, timings = [], []
        for repeat in range(1, protocol['repetitions'] + 1):
            selected = [c for c in measurements if c['context']['repetition'] == repeat]
            selected.sort(key=lambda c: c['context']['batch'])
            assert [c['context']['batch'] for c in selected] == [1, 2, 3, 4]
            nominated, nomination_count, skips, batches, seen_chunks = set(), 0, [], [], []
            for call in selected:
                assert call['status'] == 200 and call['mode'] == manifest['mode']
                raw = json.loads(call['raw_response'])
                assert raw['model'] in arm['accepted_returned_models']
                assert raw['choices'][0]['finish_reason'] == 'stop'
                facts = json.loads(raw['choices'][0]['message']['content'])['facts']
                nomination_count += len(facts)
                nominated.update(identity(f['chunk_id'], f['source'], f['relation'], f['target']) for f in facts)
                batch = json.loads((directory / f"{arm['id']}-r{repeat}-b{call['context']['batch']}.json").read_text())
                assert batch['status'] == 200 and batch['response']['proposed'] == len(facts)
                assert batch['request']['taxonomy'] == corpus['taxonomy']
                for chunk in batch['request']['chunks']:
                    assert chunk == {k: cases[chunk['id']][k] for k in ('id', 'title', 'text')}
                    seen_chunks.append(chunk['id'])
                timings.append(batch['latency_seconds'])
                skips.extend(batch['response']['skips'])
                batches.append(batch)
            assert Counter(seen_chunks) == Counter(cases.keys())
            final = batches[-1]
            entities = {e['_key']: e['name'] for e in final['entities']}
            accepted, ledger = set(), []
            for fact in final['facts']:
                chunk = fact['evidence_chunk_id']
                key = identity(chunk, entities[fact['_from'].removeprefix('entities/')],
                               fact['relation_type'], entities[fact['_to'].removeprefix('entities/')])
                accepted.add(key)
                text = unicodedata.normalize('NFC', cases[chunk]['text']).encode()
                start, end = fact['trigger_start'], fact['trigger_end']
                assert 0 <= start < end <= len(text)
                assert text[start:end].decode() == fact['trigger']
                assert fact['reviewed_by'] == f"directed:{arm['model']}@directed-policy-v1"
                assert fact['construction_schema'] == 'occurrence-v1'
                assert fact['space_id'] == f"comparison-{arm['id']}-{repeat}"
                ledger.append({'fact_key': fact['_key'], 'identity': key, 'gold_match': key in gold,
                               'trigger_start': start, 'trigger_end': end})
            assert len(final['facts']) == sum(b['response']['facts_grounded'] for b in batches)
            repetitions.append({'repetition': repeat, 'nominations': nomination_count,
                'distinct_nominations': len(nominated), 'accepted_occurrences': len(final['facts']),
                'distinct_accepted': len(accepted), 'before_gates': metrics(gold, nominated),
                'after_gates': metrics(gold, accepted), 'skips': skips,
                'false_positive_triples': sorted(accepted - gold), 'missed_triples': sorted(gold - accepted),
                'negative_cases_with_facts': sorted({f[0] for f in accepted if not cases[f[0]]['gold']}),
                'by_category': {r['relation']: metrics({f for f in gold if f[2] == r['relation']},
                    {f for f in accepted if f[2] == r['relation']}) for r in corpus['taxonomy']},
                'by_scenario': {s: metrics({f for f in gold if cases[f[0]]['scenario'] == s},
                    {f for f in accepted if cases[f[0]]['scenario'] == s}) for s in sorted({c['scenario'] for c in cases.values()})},
                'fact_ledger': ledger})
        summary.update(repetitions=repetitions, http_latency_mean_seconds=statistics.mean(timings),
            http_latency_median_seconds=statistics.median(timings), http_latency_max_seconds=max(timings))
    report['estimated_total_cost_usd'] = sum(a['estimated_measurement_cost_usd'] + a['estimated_preflight_cost_usd'] for a in report['arms'].values())
    return report


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('directory', type=Path)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    save(args.output, evaluate(args.directory))
