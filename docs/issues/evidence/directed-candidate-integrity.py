"""Post-capture integrity probes on disposable copies of candidate evidence."""
import argparse
import json
from pathlib import Path
import shutil
import sys
import tempfile

REPO = Path(__file__).resolve().parents[3]
PACKAGE = REPO / 'fixtures/semantic-neurons/luna-directed-v2-2026-09-09'
sys.path.insert(0, str(PACKAGE))
from common import digest, save
from score import score


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    report = score(PACKAGE / 'live')
    assert report['evidence_occurrences_checked'] > 0
    checks = []
    for name in ['digest', 'missing-attempt', 'stored-quote', 'document-input', 'effort-override']:
        with tempfile.TemporaryDirectory(prefix='cognigraph-directed-integrity-') as temp:
            directory = Path(temp) / 'live'
            shutil.copytree(PACKAGE / 'live', directory)
            manifest = json.loads((directory / 'manifest.json').read_text())
            if name == 'missing-attempt':
                manifest['attempts'].pop()
            elif name == 'effort-override':
                path = directory / 'provider-calls.json'
                calls = json.loads(path.read_text())
                for side in ['request_from_server', 'request_to_provider']:
                    calls[0][side]['reasoning_effort'] = 'medium'
                save(path, calls)
                manifest['artifact_sha256'][path.name] = digest(path)
            else:
                path = next(directory / p for p in manifest['attempts']
                    if name != 'stored-quote' or json.loads((directory / p).read_text())['facts'])
                observation = json.loads(path.read_text())
                if name == 'stored-quote':
                    observation['facts'][0]['trigger'] = 'fabricated quote'
                elif name == 'document-input':
                    observation['request']['chunks'][0]['text'] += ' changed input'
                else:
                    observation['latency_seconds'] += 1
                save(path, observation)
                if name != 'digest':
                    manifest['artifact_sha256'][path.name] = digest(path)
            save(directory / 'manifest.json', manifest)
            try:
                score(directory)
            except AssertionError:
                checks.append({'probe': name, 'detected': True})
            else:
                raise AssertionError('Tampering was not detected: ' + name)
    save(args.output, {'status': 'PASS', 'checks': checks, 'harness_sha256': digest(__file__),
        'live_manifest_sha256': digest(PACKAGE / 'live/manifest.json'),
        'policy': 'All mutations occurred on temporary copies; original evidence unchanged.'})
    print(json.dumps({'status': 'PASS', 'probes': len(checks)}))


if __name__ == '__main__':
    main()
