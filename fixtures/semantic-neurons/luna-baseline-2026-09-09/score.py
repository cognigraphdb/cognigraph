"""Offline scoring of frozen captures at distinct (chunk, source, relation, target) grain."""
from collections import Counter
import argparse
import json
from pathlib import Path
import statistics
import unicodedata

from transport import ROOT, digest, save


def normalize(text):
    return ' '.join(unicodedata.normalize('NFC', text).split()).casefold()


def identity(chunk, source, relation, target):
    return (chunk, normalize(source), relation, normalize(target))


def metrics(gold, predicted):
    tp, fp, fn = len(gold & predicted), len(predicted - gold), len(gold - predicted)
    return {'tp': tp, 'fp': fp, 'fn': fn,
            'precision': tp / (tp + fp) if tp + fp else None,
            'recall': tp / (tp + fn) if tp + fn else None,
            'f1': 2 * tp / (2 * tp + fp + fn) if 2 * tp + fp + fn else None}


def score(directory):
    corpus = json.loads((ROOT / 'corpus.json').read_text())
    protocol = json.loads((ROOT / 'protocol.json').read_text())
    manifest = json.loads((directory / 'manifest.json').read_text())
    assert manifest['complete'], 'Incomplete runs must be reported as failures, not scored as complete'
    assert manifest['protocol_sha256'] == digest(ROOT / 'protocol.json')
    assert protocol['corpus_sha256'] == digest(ROOT / 'corpus.json')
    for name, expected in manifest['artifact_sha256'].items():
        assert digest(directory / name) == expected, name
    calls = json.loads((directory / 'provider-calls.json').read_text())
    cases = {c['id']: c for c in corpus['cases']}
    gold = {identity(c['id'], f['source'], f['relation'], f['target'])
            for c in cases.values() for f in c['gold']}
    assert len(gold) == sum(len(c['gold']) for c in cases.values())
    expected_calls = len(protocol['arms']) * protocol['repetitions'] * len(cases) // protocol['batch_size']
    assert len(calls) == len(manifest['attempts']) == expected_calls
    report = {'schema': 'cognigraph-synthetic-directed-score-v1', 'mode': manifest['mode'],
              'corpus_cases': len(cases), 'gold_triples_per_repetition': len(gold),
              'negative_cases': sum(not c['gold'] for c in cases.values()), 'arms': {}}
    for arm in protocol['arms']:
        repeats, arm_calls, latencies = [], [], []
        for repetition in range(1, protocol['repetitions'] + 1):
            selected = [c for c in calls if c['context']['arm'] == arm['id'] and
                        c['context']['repetition'] == repetition]
            arm_calls += selected
            nominated, nominations_count, skipped, batches = set(), 0, [], []
            observed_ids = []
            for call in selected:
                assert call['mode'] == manifest['mode'] and call['status'] == 200
                assert call['request_to_provider']['model'] == arm['model']
                assert call['request_to_provider']['reasoning_effort'] == arm['reasoning_effort']
                response = json.loads(call['raw_response'])
                assert response['model'] == arm['model'], 'Returned model identity differs'
                assert response['choices'][0]['finish_reason'] == 'stop', 'Truncated/refused completion'
                facts = json.loads(response['choices'][0]['message']['content'])['facts']
                nominations_count += len(facts)
                nominated.update(identity(f['chunk_id'], f['source'], f['relation'], f['target']) for f in facts)
                context = call['context']
                batch = json.loads((directory / f"{arm['id']}-r{repetition}-b{context['batch']}.json").read_text())
                assert batch['status'] == 200
                assert batch['response']['proposed'] == len(facts)
                assert batch['request']['taxonomy'] == corpus['taxonomy']
                chunk_ids = [c['id'] for c in batch['request']['chunks']]
                observed_ids.extend(chunk_ids)
                assert all(c == {k: cases[c['id']][k] for k in ('id', 'title', 'text')}
                           for c in batch['request']['chunks'])
                skipped.extend(batch['response']['skips'])
                latencies.append(batch['latency_seconds'])
                batches.append(batch)
            assert Counter(observed_ids) == Counter(cases.keys()), 'Missing/repeated input cases'
            final = batches[-1]
            entities = {e['_key']: e['name'] for e in final['entities']}
            accepted, ledger = set(), []
            for fact in final['facts']:
                chunk_id = fact['evidence_chunk_id']
                assert chunk_id in cases
                source = entities[fact['_from'].removeprefix('entities/')]
                target = entities[fact['_to'].removeprefix('entities/')]
                key = identity(chunk_id, source, fact['relation_type'], target)
                accepted.add(key)
                text = unicodedata.normalize('NFC', cases[chunk_id]['text']).encode()
                start, end = fact['trigger_start'], fact['trigger_end']
                assert 0 <= start < end <= len(text)
                assert text[start:end].decode() == fact['trigger']
                assert fact['reviewed_by'] == f"directed:{arm['model']}@directed-policy-v1"
                assert fact['construction_schema'] == 'occurrence-v1'
                ledger.append({'fact_key': fact['_key'], 'identity': key, 'gold_match': key in gold,
                               'trigger_start': start, 'trigger_end': end})
            assert len(final['facts']) == sum(b['response']['facts_grounded'] for b in batches)
            repeats.append({'repetition': repetition, 'nominations': nominations_count,
                'distinct_nominations': len(nominated), 'accepted_occurrences': len(final['facts']),
                'distinct_accepted': len(accepted), 'skips': skipped,
                'before_gates': metrics(gold, nominated), 'after_gates': metrics(gold, accepted),
                'false_positive_triples': sorted(accepted - gold), 'missed_triples': sorted(gold - accepted),
                'negative_cases_with_facts': sorted({f[0] for f in accepted if not cases[f[0]]['gold']}),
                'by_category': {relation['relation']: metrics(
                    {f for f in gold if f[2] == relation['relation']},
                    {f for f in accepted if f[2] == relation['relation']}) for relation in corpus['taxonomy']},
                'by_scenario': {scenario: metrics(
                    {f for f in gold if cases[f[0]]['scenario'] == scenario},
                    {f for f in accepted if cases[f[0]]['scenario'] == scenario})
                    for scenario in sorted({c['scenario'] for c in cases.values()})},
                'fact_ledger': ledger})
        usage = Counter()
        for call in arm_calls:
            raw_usage = json.loads(call['raw_response'])['usage']
            for key in ('prompt_tokens', 'completion_tokens', 'total_tokens'):
                usage[key] += raw_usage[key]
            usage['cached_input_tokens'] += (raw_usage.get('prompt_tokens_details') or {}).get('cached_tokens', 0)
            usage['reasoning_tokens'] += (raw_usage.get('completion_tokens_details') or {}).get('reasoning_tokens', 0)
        cost = ((usage['prompt_tokens'] - usage['cached_input_tokens']) * arm['usd_per_million_input'] +
                usage['cached_input_tokens'] * arm['usd_per_million_cached_input'] +
                usage['completion_tokens'] * arm['usd_per_million_output']) / 1e6
        report['arms'][arm['id']] = {'model': arm['model'], 'reasoning_effort': arm['reasoning_effort'],
            'repetitions': repeats, 'usage': dict(usage), 'estimated_standard_token_cost_usd': cost,
            'http_latency_mean_seconds': statistics.mean(latencies),
            'http_latency_median_seconds': statistics.median(latencies),
            'http_latency_max_seconds': max(latencies),
            'provider_calls': len(arm_calls), 'failed_provider_calls': 0,
            'note': 'Token estimate excludes tax and unreported billing adjustments; no invoice evidence.'}
    return report


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('directory', type=Path)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    save(args.output, score(args.directory))
