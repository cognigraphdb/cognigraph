"""Post-execution integrity, negative probes, and shared-worktree preservation."""
import argparse
import importlib.util
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
from urllib.parse import unquote, urlsplit

from shared import ROOT, REPO, BASELINE, digest, save
from providers import ENDPOINTS, key_from_env
from evaluate import evaluate


def edit(path, function):
    value = json.loads(path.read_text())
    function(value)
    save(path, value)


def negative(name, mutate, refresh=False):
    with tempfile.TemporaryDirectory(prefix='cg-cross-provider-corruption-') as temporary:
        target = Path(temporary) / 'live'
        shutil.copytree(ROOT / 'live', target)
        mutate(target)
        if refresh:
            edit(target / 'manifest.json', lambda m: m.update(artifact_sha256={
                p.name: digest(p) for p in target.glob('*.json') if p.name != 'manifest.json'}))
        try:
            evaluate(target)
        except (AssertionError, KeyError, ValueError, IndexError):
            return {'case': name, 'rejected': True}
    raise AssertionError('Corrupt capture accepted: ' + name)


def verify(snapshots):
    protocol = json.loads((ROOT / 'protocol.json').read_text())
    for name, expected in protocol['code_sha256'].items():
        assert digest(REPO / name) == expected, name
    assert digest(REPO / 'target/release/cognigraph-server') == protocol['binary_sha256']
    result = evaluate(ROOT / 'live')
    assert (json.dumps(result, ensure_ascii=False, indent=2) + '\n').encode() == (ROOT / 'results.json').read_bytes()
    calls = json.loads((ROOT / 'live/provider-calls.json').read_text())
    assert sum(c['reserved_usd'] for c in calls) <= protocol['budget_usd']
    for call in calls:
        assert call['provider_url'] == ENDPOINTS[call['provider']][1]
        assert call['sent_to_provider'] and call['mode'] == 'live'
        if call['provider'] == 'deepseek':
            usage = json.loads(call['raw_response']).get('usage')
            if usage and 'prompt_cache_miss_tokens' in usage:
                assert usage['prompt_cache_hit_tokens'] + usage['prompt_cache_miss_tokens'] == usage['prompt_tokens']
    probes = [
        negative('artifact tamper', lambda p: (p / 'luna-json-r1-b1.json').write_text('{}')),
        negative('unfinished run', lambda p: edit(p / 'manifest.json', lambda m: m.update(finished=False))),
        negative('missing attempt', lambda p: edit(p / 'manifest.json', lambda m: m['attempts'].pop())),
        negative('invalid evidence span despite refreshed manifest', lambda p: edit(
            p / 'luna-json-r1-b4.json', lambda b: b['facts'][0].update(trigger_end=999999)), True),
    ]
    def wrong_model(path):
        def mutate(calls):
            call = next(c for c in calls if c['context']['arm'] == 'luna-json' and c['context']['phase'] == 'measurement')
            raw = json.loads(call['raw_response'])
            raw['model'] = 'wrong-model'
            call['raw_response'] = json.dumps(raw)
        edit(path / 'provider-calls.json', mutate)
    probes.append(negative('wrong returned model despite refreshed manifest', wrong_model, True))
    controls = json.loads((ROOT / 'controls.json').read_text())
    assert controls['mode'] == 'mock' and len(controls['arms']) == 5
    for arm in controls['arms'].values():
        assert arm['execution']['complete'] and arm['provider_calls'] == 9
        assert all((r['after_gates']['tp'], r['after_gates']['fp'], r['after_gates']['fn']) == (32, 0, 0)
                   for r in arm['repetitions'])
    preserved = {}
    for file in snapshots:
        values = json.loads(file.read_text())
        if set(values) == {'sha256'}:
            assert digest(REPO / '.env') == values['sha256']
            preserved['local_env_unchanged'] = True
        else:
            for name, expected in values.items():
                p = REPO / name
                assert (digest(p) if p.exists() else None) == expected, name
            preserved[file.name] = len(values)
    keys = [key_from_env(name).encode() for name, _ in ENDPOINTS.values()]
    files = [p for p in ROOT.rglob('*') if p.is_file() and '__pycache__' not in p.parts]
    assert all(all(k not in p.read_bytes() for k in keys) for p in files)
    spec = importlib.util.spec_from_file_location('decision_index', REPO / 'scripts/check-decision-index.py')
    links = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(links)
    decision_index = links.check_index(REPO)
    assert not decision_index['errors']
    docs = list(ROOT.glob('*.md')) + [REPO / p for p in (
        'CHANGELOG.md', 'docs/implementation-plan.md', 'docs/issues/README.md',
        'docs/decisions/README.md', 'docs/decisions/decision_luna_baseline.md',
        'docs/decisions/decision_sideviews_provider_and_benchmark.md')]
    local_links = anchors = 0
    for doc in docs:
        for _, destination in links.LINK.findall(doc.read_text()):
            target = urlsplit(destination.strip('<>'))
            if target.scheme or target.netloc:
                continue
            p = (doc.parent / unquote(target.path)).resolve() if target.path else doc
            assert p.exists(), (doc, destination)
            local_links += 1
            if target.fragment and p.suffix == '.md':
                assert unquote(target.fragment) in links.anchors(p), (doc, destination)
                anchors += 1
    subprocess.run(['git', 'diff', '--check'], cwd=REPO, check=True)
    assert not subprocess.check_output(['git', 'diff', '--cached', '--name-only'], cwd=REPO, text=True).strip()
    return {'date': '2026-09-09', 'source': 'Post-execution local audit; not an independent model or provenance attestation',
        'frozen_code_files': len(protocol['code_sha256']), 'binary_sha256': protocol['binary_sha256'],
        'offline_result_bytes_identical': True, 'provider_calls': len(calls), 'negative_probes': probes,
        'control_requests': 45, 'preservation': preserved, 'credential_scan_files': len(files),
        'documentation': {'files': len(docs), 'local_links': local_links, 'anchors': anchors},
        'decision_index': decision_index, 'results_sha256': digest(ROOT / 'results.json'),
        'manifest_sha256': digest(ROOT / 'live/manifest.json'), 'verifier_sha256': digest(Path(__file__)),
        'rust_verification': 'No Rust edits; exact previously validated CG-26 binary/source hashes checked. Earlier fmt, strict Clippy, 961 reported tests and live gates were not rerun.'}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--snapshot', type=Path, action='append', default=[])
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    save(args.output, verify(args.snapshot))
