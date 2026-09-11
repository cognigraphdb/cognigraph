"""Negative integrity probes against copies; never mutate frozen evidence."""
import argparse
import json
from pathlib import Path
import shutil
import tempfile

from prepare import ROOT, digest, save
from score_v2 import score


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    results = []
    for probe in ['capture_digest', 'missing_attempt', 'effort_override', 'stored_evidence', 'document_attribution']:
        with tempfile.TemporaryDirectory(prefix='cognigraph-document-integrity-') as temporary:
            target = Path(temporary) / 'live'
            shutil.copytree(ROOT / 'live', target)
            manifest = json.loads((target / 'manifest.json').read_text())
            name = None
            if probe == 'capture_digest':
                with (target / 'provider-calls.json').open('a') as stream:
                    stream.write(' ')
            elif probe == 'missing_attempt':
                manifest['attempts'].pop()
            elif probe == 'effort_override':
                name = 'provider-calls.json'
                calls = json.loads((target / name).read_text())
                calls[0]['request_from_server']['reasoning_effort'] = 'high'
                calls[0]['request_to_provider']['reasoning_effort'] = 'high'
                save(target / name, calls)
            elif probe == 'stored_evidence':
                for name in manifest['attempts']:
                    observation = json.loads((target / name).read_text())
                    if observation['facts']:
                        observation['facts'][0]['trigger'] += 'tampered'
                        save(target / name, observation)
                        break
                else:
                    raise AssertionError('No stored evidence to probe')
            else:
                name = manifest['attempts'][0]
                observation = json.loads((target / name).read_text())
                observation['request']['chunks'][0]['id'] = 'another-document'
                save(target / name, observation)
            if name:
                # Deliberately repair the outer digest to exercise semantic checks.
                manifest['artifact_sha256'][name] = digest(target / name)
            save(target / 'manifest.json', manifest)
            try:
                score(target)
            except AssertionError:
                results.append({'probe': probe, 'rejected': True})
            else:
                raise AssertionError('Probe was not rejected: ' + probe)
    save(args.output, {'status': 'PASS', 'probes': results, 'harness_sha256': digest(__file__)})
    print(json.dumps({'status': 'PASS', 'probes_rejected': len(results)}))


if __name__ == '__main__':
    main()
