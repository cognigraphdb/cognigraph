"""CG-35 authenticated Native startup and no-op collection ensure regression.

Uses disposable stores and synthetic documents/vectors, with no providers.
--expect-rebuilds reproduces the old unnecessary derivative rewrites.
--seed-binary verifies compatibility with files produced by a saved release.
"""
import argparse
import contextlib
import hashlib
import json
import os
from pathlib import Path
import signal
import socket
import subprocess
import tempfile
import time
import urllib.request

REPO = Path(__file__).resolve().parents[3]
COLLECTION = 'cg35_vectors'
MODES = ['resident-embedded', 'resident-sidecar', 'paged-sidecar']


def sha(data):
    return hashlib.sha256(data).hexdigest()


def files(directory):
    result = {}
    for path in directory.rglob('*.vectors'):
        data = path.read_bytes()
        assert data[:8] == b'CGVEC2\x00\x00'
        stat = path.stat()
        result[str(path.relative_to(directory))] = {
            'kind': 'vector', 'sha256': sha(data), 'payload_sha256': sha(data[36:]),
            'generation': int.from_bytes(data[8:12], 'little'), 'revision': data[20:36].hex(),
            'mtime_ns': stat.st_mtime_ns, 'inode': stat.st_ino}
    for path in directory.rglob('REVISION'):
        meta = path.parent / 'meta.json'
        stat = meta.stat()
        result[str(path.parent.relative_to(directory))] = {
            'kind': 'text', 'revision': path.read_text(), 'sha256': sha(meta.read_bytes()),
            'identity_sha256': sha((path.parent / 'IDENTITY').read_bytes()),
            'mtime_ns': stat.st_mtime_ns, 'inode': stat.st_ino}
    return result


@contextlib.contextmanager
def server(directory, mode, binary):
    directory.mkdir(exist_ok=True)
    with socket.socket() as sock:
        sock.bind(('127.0.0.1', 0))
        port = sock.getsockname()[1]
    token = None
    env = {key: os.environ[key] for key in ('PATH', 'HOME', 'TMPDIR') if key in os.environ}
    env.update(COGNIGRAPH_HOST='127.0.0.1', COGNIGRAPH_PORT=str(port),
        COGNIGRAPH_BACKEND='native', COGNIGRAPH_NATIVE_PATH=str(directory / 'db.redb'),
        COGNIGRAPH_STORAGE_MODE=mode.split('-')[0], COGNIGRAPH_VECTOR_MODE=mode.split('-')[1],
        COGNIGRAPH_EMBEDDING_PROVIDER='none', COGNIGRAPH_AUTH_ENABLED='true',
        COGNIGRAPH_ADMIN_PASSWORD='synthetic-cg35-password',
        COGNIGRAPH_JWT_SECRET='synthetic-loopback-cg35-secret')

    def call(path, body=None, method=None):
        headers = {'Content-Type': 'application/json'}
        if token:
            headers['Authorization'] = 'Bearer ' + token
        req = urllib.request.Request(f'http://127.0.0.1:{port}' + path,
            data=None if body is None else json.dumps(body).encode(), headers=headers, method=method)
        with urllib.request.urlopen(req, timeout=20) as response:
            assert response.status == 200
            return json.load(response)

    with (directory / 'server.log').open('a') as log:
        process = subprocess.Popen([str(binary)], cwd=directory, env=env, stdout=log, stderr=log)
        try:
            for _ in range(200):
                if process.poll() is not None:
                    raise RuntimeError(f'startup failed: {directory / "server.log"}')
                try:
                    call('/health')
                    break
                except OSError:
                    time.sleep(.1)
            else:
                raise RuntimeError('server startup timed out')
            token = call('/api/auth/login', {'username': 'admin', 'password': 'synthetic-cg35-password'})['token']
            yield call
        finally:
            if process.poll() is None:
                process.send_signal(signal.SIGTERM)
            try:
                process.wait(timeout=8)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()


def run(root, mode, binary, seed_binary, expect_rebuilds):
    directory = root / mode
    events = []

    def check(call, phase, changed=False):
        rows = call('/api/search/query', {'query':
            f'FOR d IN {COLLECTION} SORT d._key RETURN {{key:d._key,text:d.text}}'})['results']
        expected = [{'key': 'a', 'text': 'updated' if changed else 'needle'},
            {'key': 'b', 'text': 'needle' if changed else 'straw'}]
        assert rows == expected, (phase, rows)
        events.append({'kind': 'rows', 'phase': phase, 'rows': rows})
        winner = 'b' if changed else 'a'
        for surface in ('http', 'lua'):
            if surface == 'http':
                hits = call('/api/search/vector', {'collection': COLLECTION, 'vector': [1, 0],
                    'threshold': .9, 'limit': 1, 'model_name': 'synthetic-cg35'})['results']
            else:
                hits = call('/api/lua/execute', {'script':
                    f'return graph.similarity("{COLLECTION}", {{1,0}}, {{threshold=0.9,limit=1,model_name="synthetic-cg35"}})'})['result']
            assert len(hits) == 1 and hits[0]['document']['_key'] == winner and hits[0]['score'] == 1, (phase, hits)
            events.append({'kind': 'vector_result', 'phase': phase, 'surface': surface, 'key': winner, 'score': 1})
        hits = call('/api/search/text', {'collection': COLLECTION, 'query': 'needle', 'fields': ['text'], 'limit': 10})['results']
        assert len(hits) == 1 and hits[0]['document']['_key'] == winner, (phase, hits)
        events.append({'kind': 'text_result', 'phase': phase, 'key': winner})

    def compare(before, after, phase, vector_reuse, text_reuse):
        assert before.keys() == after.keys() and before, (phase, before, after)
        assert sum(v['kind'] == 'text' for v in before.values()) == 1
        assert sum(v['kind'] == 'vector' for v in before.values()) == (mode != 'resident-embedded')
        for name, old in before.items():
            current = after[name]
            expected_reuse = vector_reuse if old['kind'] == 'vector' else text_reuse
            reused = current == old
            assert reused == expected_reuse, (phase, name, expected_reuse, old, current)
            if not expected_reuse:
                assert current['revision'] != old['revision'], (phase, name, 'revision did not change')
            events.append({'kind': 'derivative', 'phase': phase, 'file': name,
                'derivative_kind': old['kind'], 'expected_reuse': expected_reuse, 'reused': reused,
                'before': old, 'after': current})

    with server(directory, mode, seed_binary) as call:
        for key, text, vector in [('a', 'needle', [1, 0]), ('b', 'straw', [0, 1])]:
            call('/api/documents', {'collection': COLLECTION, '_key': key, 'text': text,
                'embedding': vector, 'model_name': 'synthetic-cg35'})
        call('/api/collections', {'name': 'cg35_edges', 'collection_type': 'edge'})
        check(call, 'seed')
    seeded = files(directory)
    with server(directory, mode, binary) as call:
        check(call, 'startup')
        warm = files(directory)
        compare(seeded, warm, 'startup_noop', not expect_rebuilds, not expect_rebuilds)
        for _ in range(4):
            for name in (COLLECTION, 'cg35_edges'):
                for kind in ('document', 'edge'):
                    call('/api/collections', {'name': name, 'collection_type': kind})
        catalog = call('/api/collections')['collections']
        assert {c['name']: c['collection_type'] for c in catalog} == {COLLECTION: 'document', 'cg35_edges': 'edge'}
        events.append({'kind': 'catalog', 'phase': 'explicit_noop', 'collections': catalog, 'ensure_calls': 16})
        check(call, 'explicit_noop')
        ensured = files(directory)
        compare(warm, ensured, 'explicit_noop', not expect_rebuilds, not expect_rebuilds)
        call('/api/collections', {'name': 'cg35_new'})
        check(call, 'real_creation')
        created = files(directory)
        compare(ensured, created, 'real_creation', False, False)
        call('/api/documents/' + COLLECTION + '/a', {'text': 'updated', 'embedding': [0, 1]}, 'PATCH')
        call('/api/documents/' + COLLECTION + '/b', {'text': 'needle', 'embedding': [1, 0]}, 'PATCH')
        check(call, 'real_mutation', True)
    mutated = files(directory)
    with server(directory, mode, binary) as call:
        check(call, 'data_restart', True)
        restarted = files(directory)
        # Unflushed vector deltas require one rebuild. Text was persisted at
        # the current revision by the preceding search and can reopen directly.
        compare(mutated, restarted, 'data_restart', False, not expect_rebuilds)
    with server(directory, mode, binary) as call:
        check(call, 'unchanged_restart', True)
        compare(restarted, files(directory), 'unchanged_restart', not expect_rebuilds, not expect_rebuilds)
    return {'mode': mode, 'authenticated_restarts': 3, 'events': events, 'checks': len(events)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=REPO / 'target/release/cognigraph-server')
    parser.add_argument('--seed-binary', type=Path)
    parser.add_argument('--expect-rebuilds', action='store_true')
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    binary = args.binary.resolve()
    seed_binary = (args.seed_binary or binary).resolve()
    root = Path(tempfile.mkdtemp(prefix='cognigraph-cg35-live-'))
    print('Temporary evidence:', root, flush=True)
    report = {'issue': 'CG-35', 'revision': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=REPO, text=True).strip(),
        'binary_sha256': sha(binary.read_bytes()), 'seed_binary_sha256': sha(seed_binary.read_bytes()),
        'harness_sha256': sha(Path(__file__).read_bytes()), 'provider': 'none', 'expect_rebuilds': args.expect_rebuilds,
        'runs': [run(root, mode, binary, seed_binary, args.expect_rebuilds) for mode in MODES]}
    args.output.write_text(json.dumps(report, indent=2) + '\n')
    print('Passed', sum(r['checks'] for r in report['runs']), 'checks', flush=True)


if __name__ == '__main__':
    main()
