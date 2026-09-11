"""Capture the real directed HTTP path. Live mode spends the frozen request budget."""
import argparse
import json
from pathlib import Path
import subprocess
import time

from transport import ROOT, REPO, Recorder, Server, digest, save


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--mode', required=True, choices=['mock', 'live'])
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--binary', type=Path, default=REPO / 'target/release/cognigraph-server')
    args = parser.parse_args()
    protocol = json.loads((ROOT / 'protocol.json').read_text())
    assert digest(ROOT / 'corpus.json') == protocol['corpus_sha256']
    for file, expected in protocol['code_sha256'].items():
        assert digest(REPO / file) == expected, file
    assert digest(args.binary) == protocol['binary_sha256']
    assert not args.output.exists(), 'Use a fresh output directory; never overwrite an attempt'
    args.output.mkdir(parents=True)
    corpus = json.loads((ROOT / 'corpus.json').read_text())
    manifest = {'mode': args.mode, 'started_utc': time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime()),
        'protocol_sha256': digest(ROOT / 'protocol.json'), 'binary_sha256': digest(args.binary),
        'base_revision': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=REPO, text=True).strip(),
        'worktree_note': 'Includes uncommitted CG-26 mechanical refactor; exact code and binary digests frozen',
        'attempts': [], 'complete': False}
    save(args.output / 'manifest.json', manifest)
    recorder = Recorder(args.output, protocol, args.mode)
    try:
        for arm in protocol['arms']:
            server = Server(args.binary.resolve(), arm['model'], recorder)
            try:
                for repetition in range(protocol['repetitions']):
                    space = f"baseline-{arm['id']}-{repetition + 1}"
                    for batch in range(0, len(corpus['cases']), protocol['batch_size']):
                        cases = corpus['cases'][batch:batch + protocol['batch_size']]
                        context = {'arm': arm['id'], 'repetition': repetition + 1,
                                   'batch': batch // protocol['batch_size'] + 1, 'cases': cases}
                        recorder.context = context
                        body = {'space_type': space, 'taxonomy': corpus['taxonomy'],
                                'chunks': [{k: case[k] for k in ('id', 'title', 'text')} for case in cases]}
                        status, response, seconds = server.call('/api/construct/directed', body)
                        observation = {'arm': arm['id'], 'repetition': repetition + 1,
                            'batch': context['batch'], 'request': body, 'status': status,
                            'response': response, 'latency_seconds': seconds,
                            'facts': [f for f in server.rows('facts') if f['space_id'] == space],
                            'entities': server.rows('entities'), 'chunks': server.rows('chunks')}
                        name = f"{arm['id']}-r{repetition + 1}-b{context['batch']}.json"
                        save(args.output / name, observation)
                        manifest['attempts'].append(name)
                        save(args.output / 'manifest.json', manifest)
                        print(f"{arm['id']} repeat {repetition + 1} batch {context['batch']}: HTTP {status}", flush=True)
                        if status != 200:
                            raise RuntimeError('Recorded failed attempt; stopping without retry or tuning')
            finally:
                server.close()
        manifest['complete'] = True
    finally:
        recorder.close()
        manifest['reserved_usd'] = recorder.reserved
        manifest['finished_utc'] = time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime())
        manifest['artifact_sha256'] = {p.name: digest(p) for p in sorted(args.output.glob('*.json'))
                                       if p.name != 'manifest.json'}
        save(args.output / 'manifest.json', manifest)


if __name__ == '__main__':
    main()
