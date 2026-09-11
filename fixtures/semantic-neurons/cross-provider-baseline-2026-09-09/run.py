"""Run a frozen preflight then two full repetitions per available model, through real HTTP."""
import argparse
from datetime import datetime, timezone
import json
from pathlib import Path

from shared import ROOT, REPO, BASELINE, Server, digest, save
from providers import Recorder


def utc():
    return datetime.now(timezone.utc).isoformat().replace('+00:00', 'Z')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--mode', choices=['live', 'mock'], required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    protocol = json.loads((ROOT / 'protocol.json').read_text())
    for name, expected in protocol['code_sha256'].items():
        assert digest(REPO / name) == expected, name
    assert digest(BASELINE / 'corpus.json') == protocol['corpus_sha256']
    binary = REPO / 'target/release/cognigraph-server'
    assert digest(binary) == protocol['binary_sha256']
    assert not args.output.exists(), 'Never overwrite a previous attempt'
    args.output.mkdir(parents=True)
    corpus = json.loads((BASELINE / 'corpus.json').read_text())
    manifest = {'mode': args.mode, 'started_utc': utc(), 'protocol_sha256': digest(ROOT / 'protocol.json'),
        'binary_sha256': digest(binary), 'finished': False, 'arms': {}, 'attempts': []}
    save(args.output / 'manifest.json', manifest)
    recorder = Recorder(args.output, protocol, args.mode)
    try:
        for arm in protocol['arms']:
            status_record = {'complete': False, 'preflight_passed': False}
            manifest['arms'][arm['id']] = status_record
            server = Server(binary, arm['model'], recorder)
            schedule = [('preflight', 0, 0, [protocol['preflight_case']])]
            schedule += [('measurement', repeat, batch // protocol['batch_size'] + 1,
                          corpus['cases'][batch:batch + protocol['batch_size']])
                         for repeat in range(1, protocol['repetitions'] + 1)
                         for batch in range(0, len(corpus['cases']), protocol['batch_size'])]
            try:
                for phase, repeat, batch, cases in schedule:
                    space = f"comparison-{arm['id']}-{repeat}"
                    recorder.context = {'phase': phase, 'arm': arm['id'], 'repetition': repeat,
                                        'batch': batch, 'cases': cases}
                    body = {'space_type': space, 'taxonomy': corpus['taxonomy'],
                            'chunks': [{k: c[k] for k in ('id', 'title', 'text')} for c in cases]}
                    status, result, seconds = server.call('/api/construct/directed', body)
                    observation = {'phase': phase, 'arm': arm['id'], 'repetition': repeat, 'batch': batch,
                        'request': body, 'status': status, 'response': result, 'latency_seconds': seconds,
                        'facts': [f for f in server.rows('facts') if f['space_id'] == space] if status == 200 else [],
                        'entities': server.rows('entities') if status == 200 else [],
                        'chunks': server.rows('chunks') if status == 200 else []}
                    name = f"{arm['id']}-r{repeat}-b{batch}.json"
                    save(args.output / name, observation)
                    manifest['attempts'].append(name)
                    print(f"{arm['id']} {phase} r{repeat} b{batch}: HTTP {status}", flush=True)
                    if status != 200:
                        status_record['failure'] = {'phase': phase, 'repetition': repeat, 'batch': batch, 'http_status': status}
                        break
                    raw = json.loads(recorder.records[-1]['raw_response'])
                    if raw.get('model') not in arm['accepted_returned_models'] or raw['choices'][0]['finish_reason'] != 'stop':
                        status_record['failure'] = {'phase': phase, 'reason': 'Unexpected model or incomplete completion'}
                        break
                    if phase == 'preflight':
                        status_record['preflight_passed'] = True
                    save(args.output / 'manifest.json', manifest)
                else:
                    status_record['complete'] = True
            finally:
                server.close()
                save(args.output / 'manifest.json', manifest)
        manifest['finished'] = True
    finally:
        recorder.close()
        manifest.update(finished_utc=utc(), reserved_usd=recorder.reserved,
            artifact_sha256={p.name: digest(p) for p in sorted(args.output.glob('*.json')) if p.name != 'manifest.json'})
        save(args.output / 'manifest.json', manifest)


if __name__ == '__main__':
    main()
