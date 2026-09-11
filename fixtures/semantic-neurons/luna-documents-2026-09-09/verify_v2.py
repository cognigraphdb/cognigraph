"""Independently check frozen cohort reconstruction, capture integrity and scores."""
import argparse
import json
from pathlib import Path

from prepare import ROOT, REPO, build, digest, normalize, save
from score_v2 import score


def verify_data():
    settings, selected, quality = build()
    for split, (inputs, reference) in selected.items():
        assert inputs == json.loads((ROOT / 'inputs' / f'{split}.json').read_text())
        assert reference == json.loads((ROOT / 'gold' / f'{split}.json').read_text())
        assert all(set(d) == {'id', 'title', 'text'} for d in inputs)
    assert quality == json.loads((ROOT / 'data-quality.json').read_text())
    candidates = json.loads((ROOT / 'review/development-negative-candidates.json').read_text())
    assert all(c['verdict'] == 'unreviewed' for c in candidates)
    return {'source_documents_verified': quality['source_documents_validated'],
        'selected_documents_reproduced': sum(settings['counts'].values()),
        'negative_review_candidates_unlabelled': len(candidates),
        'published_reference_relations': sum(q['reference_relations'] for q in quality['splits'].values())}


def verify_control():
    report = score(ROOT / 'control')
    gold = json.loads((ROOT / 'gold/development.json').read_text())
    representable = 0
    for doc in gold:
        by_alias = {}
        for entity in doc['entities']:
            for alias in entity['aliases']:
                by_alias.setdefault(normalize(alias), set()).add(entity['id'])
        unique_ids = set().union(*(ids for ids in by_alias.values() if len(ids) == 1))
        representable += sum(r['head'] in unique_ids and r['tail'] in unique_ids for r in doc['relations'])
    assert report['raw']['matched'] == representable
    assert report['raw']['unmatched'] == 0
    assert not report['provider_failures']
    assert report['evidence_occurrences_checked'] > 0
    return {'status': 'PASS', 'protocol_sha256': digest(ROOT / 'protocol.json'),
        'provider_calls_sha256': digest(ROOT / 'control/provider-calls.json'),
        'documents': report['documents'], 'reference_relations_with_unique_aliases': representable,
        'ambiguous_reference_relations_retained_in_denominator': report['raw']['missed'],
        'accepted_occurrences_verified': report['evidence_occurrences_checked'],
        'raw_control': report['raw'], 'accepted_control': report['accepted']}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--control-only', action='store_true')
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    protocol = json.loads((ROOT / 'protocol.json').read_text())
    amendment = json.loads((ROOT / 'scoring-amendment-v2.json').read_text())
    for name, expected in amendment['code_sha256'].items():
        assert digest(ROOT / name) == expected, name
    for name, expected in protocol['package_sha256'].items():
        assert digest(ROOT / name) == expected, name
    for name, expected in protocol['code_sha256'].items():
        assert digest(REPO / name) == expected, name
    control = verify_control()
    if args.control_only:
        save(args.output, control)
        print(json.dumps(control, indent=2))
        return
    report = score(ROOT / 'live')
    assert json.loads(json.dumps(report)) == json.loads((ROOT / 'results.json').read_text())
    controls = {c['context']['document_id']: c['request_from_server']
                for c in json.loads((ROOT / 'control/provider-calls.json').read_text())}
    calls = json.loads((ROOT / 'live/provider-calls.json').read_text())
    assert all(c['request_from_server'] == controls[c['context']['document_id']] for c in calls)
    result = {'status': 'PASS', 'data': verify_data(), 'control': control,
        'source_files_verified': len(protocol['code_sha256']),
        'package_files_verified': len(protocol['package_sha256']),
        'live_requests_verified': len(calls), 'gold_control_prompt_parity': True,
        'live_occurrences_verified': report['evidence_occurrences_checked'],
        'scores_and_costs_replayed': True, 'holdout_inference_calls': 0}
    save(args.output, result)
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
