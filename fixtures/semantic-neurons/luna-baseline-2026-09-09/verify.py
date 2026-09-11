"""Post-capture integrity audit; read-only except the requested validation output."""
import argparse
import importlib.util
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
from urllib.parse import unquote, urlsplit

from prepare import prepare
from score import score
from transport import ROOT, REPO, digest, existing_key, save


def rejects_mutation(name, mutate, refresh_hashes=False):
    with tempfile.TemporaryDirectory(prefix='luna-capture-negative-') as temporary:
        target = Path(temporary) / 'capture'
        shutil.copytree(ROOT / 'live', target)
        mutate(target)
        if refresh_hashes:
            manifest = json.loads((target / 'manifest.json').read_text())
            manifest['artifact_sha256'] = {p.name: digest(p) for p in target.glob('*.json')
                                           if p.name != 'manifest.json'}
            save(target / 'manifest.json', manifest)
        try:
            score(target)
        except (AssertionError, KeyError, ValueError, IndexError):
            return {'probe': name, 'rejected': True}
    raise AssertionError('Corruption was accepted: ' + name)


def modify(path, transform):
    value = json.loads(path.read_text())
    transform(value)
    save(path, value)


def verify(preservation_snapshot=None):
    protocol = json.loads((ROOT / 'protocol.json').read_text())
    corpus = json.loads((ROOT / 'corpus.json').read_text())
    assert corpus == prepare()
    for name, expected in protocol['code_sha256'].items():
        assert digest(REPO / name) == expected, name
    assert digest(REPO / 'target/release/cognigraph-server') == protocol['binary_sha256']
    results = score(ROOT / 'live')
    assert (json.dumps(results, ensure_ascii=False, indent=2) + '\n').encode() == (ROOT / 'results.json').read_bytes()
    calls = json.loads((ROOT / 'live/provider-calls.json').read_text())
    reference = {}
    for call in calls:
        incoming, outgoing = call['request_from_server'], call['request_to_provider']
        arm = next(a for a in protocol['arms'] if a['model'] == incoming['model'])
        assert set(incoming) == {'model', 'messages', 'response_format'} | (
            {'reasoning_effort'} if arm['id'] == 'luna' else set())
        assert outgoing == dict(incoming, reasoning_effort=arm['reasoning_effort'],
                                max_completion_tokens=protocol['max_completion_tokens'])
        assert incoming['response_format']['json_schema']['strict'] is True
        comparable = {key: incoming[key] for key in ('messages', 'response_format')}
        batch = call['context']['batch']
        assert comparable == reference.setdefault(batch, comparable)
        assert 'gold' not in incoming['messages'][1]['content']
        raw = json.loads(call['raw_response'])
        assert raw['service_tier'] == 'default'
        assert raw['usage']['prompt_tokens_details']['cached_tokens'] == 0
        assert raw['usage']['prompt_tokens_details']['cache_write_tokens'] == 0
        assert raw['usage']['completion_tokens_details']['reasoning_tokens'] == 0
        assert call['mode'] == 'live' and call['sent_to_provider']
    assert sum(c['reserved_usd'] for c in calls) <= protocol['budget_usd']
    control = json.loads((ROOT / 'controls/mock-results.json').read_text())
    assert control['mode'] == 'mock'
    assert sum(a['provider_calls'] for a in control['arms'].values()) == 16
    for arm in control['arms'].values():
        for run in arm['repetitions']:
            assert (run['after_gates']['tp'], run['after_gates']['fp'], run['after_gates']['fn']) == (32, 0, 0)
    rejection = json.loads((ROOT / 'controls/gate-rejection.json').read_text())
    assert rejection['mode'] == 'mock'
    assert rejection['response']['proposed'] == 4
    assert rejection['response']['facts_grounded'] == 1
    assert len(rejection['response']['skips']) == 2

    # Confirm scored semantic data maps onto the final recorded Rust refactor.
    rust_checkpoint = json.loads((REPO / 'docs/issues/evidence/server-modularity-validation-2026-09-09.json').read_text())
    compared = 0
    for name, expected in rust_checkpoint['changed_file_sha256'].items():
        if name.endswith('.rs') and expected is not None:
            assert digest(REPO / name) == expected, name
            compared += 1

    probes = [
        rejects_mutation('byte tamper', lambda p: (p / 'luna-r1-b1.json').write_text('{}')),
        rejects_mutation('incomplete run', lambda p: modify(p / 'manifest.json', lambda m: m.update(complete=False))),
        rejects_mutation('missing batch', lambda p: modify(p / 'manifest.json', lambda m: m['attempts'].pop())),
        rejects_mutation('invalid byte span despite refreshed artifact hash', lambda p: modify(
            p / 'luna-r1-b4.json', lambda b: b['facts'][0].update(trigger_end=999999)), True),
    ]
    def wrong_model(path):
        def change(calls):
            raw = json.loads(calls[0]['raw_response'])
            raw['model'] = 'unexpected-model'
            calls[0]['raw_response'] = json.dumps(raw)
        modify(path / 'provider-calls.json', change)
    probes.append(rejects_mutation('wrong returned model despite refreshed artifact hash', wrong_model, True))

    credential = existing_key().encode()
    checked_files = [p for p in ROOT.rglob('*') if p.is_file() and '__pycache__' not in p.parts]
    assert all(credential not in p.read_bytes() for p in checked_files)
    decisions = json.loads(subprocess.check_output(
        ['python3', 'scripts/check-decision-index.py'], cwd=REPO, text=True))
    assert not decisions['errors']
    spec = importlib.util.spec_from_file_location('decision_links', REPO / 'scripts/check-decision-index.py')
    links_module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(links_module)
    docs = [REPO / p for p in (
        'CHANGELOG.md', 'docs/implementation-plan.md', 'docs/issues/CG-25.md',
        'docs/issues/README.md', 'docs/issues/cuad-recovery-2026-09-09.md',
        'docs/issues/server-modularity-2026-09-09.md', 'docs/decisions/README.md',
        'docs/decisions/decision_luna_baseline.md',
        'docs/decisions/decision_sideviews_provider_and_benchmark.md',
        'fixtures/semantic-neurons/cuad-2026-07-22/README.md')]
    docs += list(ROOT.glob('*.md'))
    local_links, anchors = 0, 0
    for doc in docs:
        for _, destination in links_module.LINK.findall(doc.read_text()):
            target = urlsplit(destination.strip('<>'))
            if target.scheme or target.netloc:
                continue
            path = (doc.parent / unquote(target.path)).resolve() if target.path else doc
            assert path.exists(), (doc.name, destination)
            local_links += 1
            if target.fragment and path.suffix == '.md':
                assert unquote(target.fragment) in links_module.anchors(path), (doc.name, destination)
                anchors += 1
    preserved = None
    if preservation_snapshot:
        preserved = json.loads(preservation_snapshot.read_text())
        for name, expected in preserved.items():
            path = REPO / name
            assert (digest(path) if path.exists() else None) == expected, name
    subprocess.run(['git', 'diff', '--check'], cwd=REPO, check=True)
    registry = (REPO / 'docs/issues/README.md').read_text()
    rows = [line.split('|')[3].strip() for line in registry.splitlines() if line.startswith('| [CG-')]
    assert rows.count('Resolved') == 37 and rows.count('Closed without change') == 1 and len(rows) == 38
    return {'date': '2026-09-09', 'source': 'Post-capture local integrity verification; not an independent attestation',
        'frozen_code_files_checked': len(protocol['code_sha256']),
        'rust_files_identical_to_prior_CG26_validation': compared,
        'rust_gates': 'Prior CG-26 formatting, strict Clippy, 961 reported tests and release build passed; unchanged Rust/binary verified, not rerun',
        'corpus_repreparation_exact': True, 'offline_scores_byte_identical': True,
        'requests_with_same_prompt_schema_by_batch': len(calls),
        'model_identity_and_explicit_settings_verified': True,
        'synthetic_control_requests': 17,
        'control_results_sha256': digest(ROOT / 'controls/mock-results.json'),
        'gate_control_sha256': digest(ROOT / 'controls/gate-rejection.json'),
        'negative_probes': probes, 'files_checked_for_actual_credential': len(checked_files),
        'user_file_preservation_sha256': preserved,
        'documentation': {'files': len(docs), 'local_links': local_links, 'anchors': anchors},
        'decision_index': decisions, 'registry': {'Resolved': 37, 'Closed without change': 1, 'Open': 0},
        'results_sha256': digest(ROOT / 'results.json'),
        'live_manifest_sha256': digest(ROOT / 'live/manifest.json'),
        'live_provider_calls': len(calls), 'accepted_occurrences': sum(
            r['accepted_occurrences'] for a in results['arms'].values() for r in a['repetitions']),
        'verification_script_sha256': digest(Path(__file__))}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--preservation-snapshot', type=Path)
    args = parser.parse_args()
    save(args.output, verify(args.preservation_snapshot))
