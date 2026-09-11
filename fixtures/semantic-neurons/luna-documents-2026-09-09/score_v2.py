"""V2 scoring amendment: retain invalid chunk citations as unmatched predictions.

Original scorer and protocol remain frozen. No generation or gold changes.
"""
from collections import Counter
import json
from pathlib import Path
import statistics
import sys

from prepare import ROOT, canonical_relation, digest, normalize, save

sys.path.insert(0, str(ROOT.parent / 'terra-luna-low-semeval-2026-09-09'))
from accounting import usage_cost


def aliases(reference):
    result = {}
    for entity in reference['entities']:
        for alias in entity['aliases']:
            result.setdefault(normalize(alias), set()).add(f"e:{entity['id']}")
    return result


def resolve(name, alias_map):
    candidates = alias_map.get(normalize(name), set())
    if len(candidates) == 1:
        return next(iter(candidates))
    return ('ambiguous:' if candidates else 'unmapped:') + normalize(name)


def reference_keys(reference, symmetric):
    return {canonical_relation(f"e:{r['head']}", r['relation'], f"e:{r['tail']}", symmetric)
            for r in reference['relations']}


def prediction_keys(proposals, reference, symmetric):
    names = aliases(reference)
    return {canonical_relation(resolve(p['source'], names), p['relation'],
                               resolve(p['target'], names), symmetric) for p in proposals}


def attributed_prediction_keys(proposals, reference, symmetric, doc_id):
    valid = [p for p in proposals if p['chunk_id'] == doc_id]
    result = prediction_keys(valid, reference, symmetric)
    for p in proposals:
        if p['chunk_id'] != doc_id:
            result.add(canonical_relation(
                'invalid_chunk:' + p['chunk_id'] + ':' + normalize(p['source']),
                p['relation'],
                'invalid_chunk:' + p['chunk_id'] + ':' + normalize(p['target']), symmetric))
    return result


def metrics(gold, predictions):
    tp, extra, missed = len(gold & predictions), len(predictions - gold), len(gold - predictions)
    return {'matched': tp, 'unmatched': extra, 'missed': missed,
            'reference_precision': tp / (tp + extra) if tp + extra else None,
            'reference_recall': tp / (tp + missed) if tp + missed else None,
            'reference_f1': 2 * tp / (2 * tp + extra + missed) if 2 * tp + extra + missed else None}


def score(directory):
    directory = Path(directory)
    manifest = json.loads((directory / 'manifest.json').read_text())
    assert manifest['complete'] and manifest['split'] == 'development'
    for name, expected in manifest['artifact_sha256'].items():
        assert digest(directory / name) == expected, name
    protocol = json.loads((ROOT / 'protocol.json').read_text())
    assert manifest['protocol_sha256'] == digest(ROOT / 'protocol.json')
    for name, expected in protocol['package_sha256'].items():
        assert digest(ROOT / name) == expected, name
    settings = json.loads((ROOT / 'settings.json').read_text())
    inputs = {d['id']: d for d in json.loads((ROOT / 'inputs/development.json').read_text())}
    gold = {d['id']: d for d in json.loads((ROOT / 'gold/development.json').read_text())}
    symmetric = {r['relation'] for r in settings['taxonomy'] if r['symmetric']}
    calls = json.loads((directory / 'provider-calls.json').read_text())
    assert len(calls) == len(manifest['attempts']) == len(inputs)
    assert {c['context']['document_id'] for c in calls} == set(inputs)
    report = {'mode': manifest['mode'], 'split': 'development', 'documents': len(inputs),
        'metric_interpretation': 'Agreement with published annotations; unmatched predictions require human review.',
        'per_document': [], 'provider_failures': [], 'unknown_usage_calls': [],
        'evidence_occurrences_checked': 0, 'holdout_inference_calls': 0}
    all_gold, raw_all, accepted_all = set(), set(), set()
    costs, latencies, usage = [], [], Counter()
    endpoint_gold, endpoint_raw, endpoint_accepted = set(), set(), set()
    for call, filename in zip(calls, manifest['attempts'], strict=True):
        observation = json.loads((directory / filename).read_text())
        doc_id = call['context']['document_id']
        assert observation['document_id'] == doc_id
        assert observation['request']['chunks'] == [inputs[doc_id]]
        incoming, outgoing = call['request_from_server'], call['request_to_provider']
        assert outgoing == dict(incoming, max_completion_tokens=settings['max_completion_tokens'])
        assert outgoing['reasoning_effort'] == 'low' and outgoing['model'] == 'gpt-5.6-luna'
        assert outgoing['response_format']['type'] == 'json_schema'
        assert outgoing['response_format']['json_schema']['strict'] is True
        raw = json.loads(call['raw_response'])
        raw_usage = raw.get('usage')
        if raw_usage:
            cost = usage_cost(raw_usage, protocol['arm'], call['started_utc'])
            costs.append(cost)
            for key in ['prompt_tokens', 'completion_tokens', 'reasoning_tokens']:
                if cost[key] is not None:
                    usage[key] += cost[key]
        else:
            report['unknown_usage_calls'].append(doc_id)
        predictions = []
        if observation['status'] == 200:
            assert call['status'] == 200 and raw['model'] == settings['model']
            assert raw['choices'][0]['finish_reason'] == 'stop'
            predictions = json.loads(raw['choices'][0]['message']['content'])['facts']
        else:
            report['provider_failures'].append({'document_id': doc_id, 'http_status': observation['status'],
                                              'provider_status': call['status']})
            assert not observation['facts']
        names = {e['_key']: e['name'] for e in observation['entities']}
        stored = []
        for fact in observation['facts']:
            assert fact['evidence_chunk_id'] == doc_id
            assert fact['space_id'] == 'document-trial-' + doc_id
            text = inputs[doc_id]['text'].encode()
            start, end = fact['trigger_start'], fact['trigger_end']
            assert 0 <= start < end <= len(text)
            assert text[start:end].decode() == fact['trigger']
            assert fact['construction_schema'] == 'occurrence-v1'
            assert fact['reviewed_by'] == 'directed:gpt-5.6-luna@directed-policy-v1'
            stored.append({'source': names[fact['_from'].split('/', 1)[1]],
                'target': names[fact['_to'].split('/', 1)[1]], 'relation': fact['relation_type']})
            report['evidence_occurrences_checked'] += 1
        expected = reference_keys(gold[doc_id], symmetric)
        proposed = attributed_prediction_keys(predictions, gold[doc_id], symmetric, doc_id)
        accepted = prediction_keys(stored, gold[doc_id], symmetric)
        assert accepted <= proposed, 'Writer introduced an unproposed relation identity'
        row = {'document_id': doc_id, 'title': inputs[doc_id]['title'], 'status': observation['status'],
            'raw': metrics(expected, proposed), 'accepted': metrics(expected, accepted),
            'unmatched_raw_keys': sorted(proposed - expected), 'unmatched_accepted_keys': sorted(accepted - expected),
            'missed_raw_keys': sorted(expected - proposed), 'missed_accepted_keys': sorted(expected - accepted),
            'skips': observation['response'].get('skips', []),
            'invalid_chunk_citations': sum(p['chunk_id'] != doc_id for p in predictions)}
        report['per_document'].append(row)
        all_gold.update((doc_id, *key) for key in expected)
        raw_all.update((doc_id, *key) for key in proposed)
        accepted_all.update((doc_id, *key) for key in accepted)
        for target, values in [(endpoint_gold, expected), (endpoint_raw, proposed), (endpoint_accepted, accepted)]:
            target.update((doc_id, key[side]) for key in values for side in [0, 2])
        latencies.append(observation['latency_seconds'])
    report['raw'] = metrics(all_gold, raw_all)
    report['accepted'] = metrics(all_gold, accepted_all)
    report['relation_endpoint_discovery'] = {'scope': 'Entities participating in in-scope relation predictions, not standalone NER.',
        'raw': metrics(endpoint_gold, endpoint_raw), 'accepted': metrics(endpoint_gold, endpoint_accepted)}
    report['per_relation'] = {}
    for rule in settings['taxonomy']:
        relation = rule['relation']
        expected = {p for p in all_gold if p[2] == relation}
        report['per_relation'][relation] = {'reference_count': len(expected),
            'raw': metrics(expected, {p for p in raw_all if p[2] == relation}),
            'accepted': metrics(expected, {p for p in accepted_all if p[2] == relation})}
    report['macro_reference_f1'] = {stage: statistics.mean(
        row[stage]['reference_f1'] for row in report['per_relation'].values() if row[stage]['reference_f1'] is not None)
        for stage in ['raw', 'accepted']}
    report['latency_seconds'] = {'mean': statistics.mean(latencies), 'median': statistics.median(latencies),
                                 'maximum': max(latencies)}
    report['usage'] = dict(usage)
    report['token_cost_usd'] = {'lower': sum(c['estimated_usd_lower'] for c in costs),
        'upper': sum(c['estimated_usd_upper'] for c in costs), 'unknown_usage_calls': len(report['unknown_usage_calls']),
        'note': 'Standard frozen token tariff estimate; not an invoice.'}
    return report


if __name__ == '__main__':
    import argparse
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('directory', type=Path)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    save(args.output, score(args.directory))
