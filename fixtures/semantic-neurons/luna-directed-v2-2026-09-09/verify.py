"""Verify candidate replay/live captures and report paired development outcomes."""
import argparse
import json
from pathlib import Path

from common import ROOT, BASE, digest, save, verify_protocol, expected_wire
from score import score


def identity(fact):
    return tuple(fact[k] for k in ['_from', 'relation_type', '_to', 'evidence_chunk_id', 'trigger_start', 'trigger_end'])


def verify_capture(mode):
    manifest = json.loads((ROOT / mode / 'manifest.json').read_text())
    calls = json.loads((ROOT / mode / 'provider-calls.json').read_text())
    inputs = json.loads((BASE / 'inputs/development.json').read_text())
    settings = json.loads((BASE / 'settings.json').read_text())
    prior = json.loads((BASE / 'live/provider-calls.json').read_text())
    assert len(calls) == len(inputs) == len(manifest['attempts']) == 40
    assert manifest['complete'] and manifest['holdout_inference_calls'] == 0
    assert manifest['binary_sha256'] == verify_protocol()['binary_sha256']
    removed = []
    for call, old, doc, name in zip(calls, prior, inputs, manifest['attempts'], strict=True):
        assert call['context']['document_id'] == old['context']['document_id'] == doc['id']
        assert name == doc['id'] + '.json'
        assert call['request_from_server'] == expected_wire(old['request_from_server'], doc, settings)
        assert call['sent_to_provider'] is (mode == 'live')
        observation = json.loads((ROOT / mode / name).read_text())
        prior_observation = json.loads((BASE / 'live' / name).read_text())
        assert observation['request'] == prior_observation['request']
        if mode == 'replay':
            original = json.loads(old['raw_response'])
            replayed = json.loads(call['raw_response'])
            assert original['choices'] == replayed['choices'], 'Replay changed proposals'
            assert observation['status'] == 200 and call['status'] == 200
            before = {identity(f): f for f in prior_observation['facts']}
            after = {identity(f): f for f in observation['facts']}
            assert after.keys() <= before.keys(), 'Boundary-only replay introduced an occurrence'
            for key in before.keys() - after.keys():
                removed.append({'document_id': doc['id'], 'fact': before[key], 'skips': observation['response']['skips']})
    return {'status': 'PASS', 'protocol_sha256': digest(ROOT / 'protocol.json'),
        'manifest_sha256': digest(ROOT / mode / 'manifest.json'), 'requests_verified': len(calls),
        'prompt_unchanged_two_enums_added': True, 'holdout_inference_calls': 0,
        'fixed_proposal_occurrences_removed': removed}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--replay-only', action='store_true')
    args = parser.parse_args()
    verify_protocol(current_code=True)
    baseline = json.loads((BASE / 'results.json').read_text())
    replay = score(ROOT / 'replay')
    replay_check = verify_capture('replay')
    assert replay['raw'] == baseline['raw'], 'Same fixed proposals must receive identical raw scores'
    assert not replay['provider_failures']
    save(ROOT / 'replay-results.json', replay)
    save(ROOT / 'replay-validation.json', replay_check)
    if args.replay_only:
        print(json.dumps({'status': 'PASS', 'requests': 40, 'removed': len(replay_check['fixed_proposal_occurrences_removed']),
                          'accepted': replay['accepted']}))
        return
    live = score(ROOT / 'live')
    live_check = verify_capture('live')
    save(ROOT / 'live-results.json', live)
    comparisons = {}
    for label, report in [('baseline', baseline), ('fixed_proposals_new_gates', replay), ('new_live_candidate', live)]:
        comparisons[label] = {k: report[k] for k in ['raw', 'accepted', 'documents', 'evidence_occurrences_checked',
            'provider_failures', 'token_cost_usd', 'latency_seconds']}
        comparisons[label]['invalid_chunk_citations'] = sum(r['invalid_chunk_citations'] for r in report['per_document'])
        comparisons[label]['skipped'] = sum(len(r['skips']) for r in report['per_document'])
    result = {'status': 'PASS', 'replay': replay_check, 'live': live_check, 'comparison': comparisons,
        'interpretation': 'Single development pass with incomplete published references; no independently verified precision or negative-case qualification. Fixed-output replay isolates the gate change; live output differences also include generation variability.',
        'holdout_inference_calls': 0}
    save(ROOT / 'comparison.json', result)
    print(json.dumps({'status': 'PASS', 'comparison': comparisons}, indent=2))


if __name__ == '__main__':
    main()
