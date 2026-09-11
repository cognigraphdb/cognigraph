"""CG-34 Native model-selection lifecycle through authenticated HTTP and Lua.

Uses disposable stores and synthetic vectors. --expect-vulnerable verifies a
saved pre-fix binary. --seed-binary also proves reuse of pre-fix sidecar files.
"""
import argparse
import contextlib
import hashlib
import json
import math
import os
from pathlib import Path
import signal
import socket
import subprocess
import tempfile
import time
import urllib.error
import urllib.request

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--binary', type=Path)
parser.add_argument('--seed-binary', type=Path)
parser.add_argument('--expect-vulnerable', action='store_true')
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
REPO = Path(__file__).resolve().parents[3]
BINARY = (args.binary or REPO / 'target/release/cognigraph-server').resolve()
FIXTURE_PATH = REPO / 'crates/cognigraph-native/tests/fixtures/sidecar-model-filter.json'
FIXTURE = json.loads(FIXTURE_PATH.read_text())
ROOT = Path(tempfile.mkdtemp(prefix='cognigraph-cg34-live-'))
MODES = ['resident-embedded', 'resident-sidecar', 'paged-sidecar']
COLLECTION = 'cg34_vectors'


@contextlib.contextmanager
def server(mode, binary=BINARY):
    directory = ROOT / mode
    directory.mkdir(exist_ok=True)
    with socket.socket() as sock:
        sock.bind(('127.0.0.1', 0))
        port = sock.getsockname()[1]
    token = None
    env = {k: os.environ[k] for k in ('PATH', 'HOME', 'TMPDIR') if k in os.environ}
    env.update(COGNIGRAPH_HOST='127.0.0.1', COGNIGRAPH_PORT=str(port),
        COGNIGRAPH_BACKEND='native', COGNIGRAPH_NATIVE_PATH=str(directory / 'db.redb'),
        COGNIGRAPH_STORAGE_MODE=mode.split('-')[0], COGNIGRAPH_VECTOR_MODE=mode.split('-')[1],
        COGNIGRAPH_EMBEDDING_PROVIDER='none', COGNIGRAPH_AUTH_ENABLED='true',
        COGNIGRAPH_ADMIN_PASSWORD='synthetic-regression-password',
        COGNIGRAPH_JWT_SECRET='synthetic-loopback-secret')

    def call(path, body=None, method=None):
        headers = {'Content-Type': 'application/json'}
        if token:
            headers['Authorization'] = 'Bearer ' + token
        request = urllib.request.Request(f'http://127.0.0.1:{port}' + path,
            data=None if body is None else json.dumps(body).encode(), headers=headers, method=method)
        with urllib.request.urlopen(request, timeout=20) as response:
            assert response.status == 200
            return json.load(response)

    with (directory / 'server.log').open('a') as log:
        process = subprocess.Popen([str(binary.resolve())], cwd=directory, env=env, stdout=log, stderr=log)
        try:
            for _ in range(200):
                if process.poll() is not None:
                    raise RuntimeError(f'startup failed: {directory / "server.log"}')
                try:
                    call('/health')
                    break
                except (OSError, urllib.error.URLError):
                    time.sleep(.1)
            else:
                raise RuntimeError('startup timed out')
            token = call('/api/auth/login', {'username': 'admin', 'password': 'synthetic-regression-password'})['token']
            yield call
        finally:
            if process.poll() is None:
                process.send_signal(signal.SIGTERM)
                try:
                    process.wait(timeout=8)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()


def seed(call):
    for row in FIXTURE['rows']:
        call('/api/documents', {'collection': COLLECTION, **row})


def apply(call, action):
    kind = action['op']
    path = '/api/documents/' + COLLECTION + '/' + action.get('key', '')
    if kind == 'create':
        call('/api/documents', {'collection': COLLECTION, **action['doc']})
    elif kind == 'update':
        call(path, action['doc'], 'PATCH')
    elif kind == 'replace':
        call(path, action['doc'], 'PUT')
    elif kind == 'delete':
        call(path, method='DELETE')
    elif kind == 'batch':
        ops = []
        for op in action['operations']:
            if op['op'] == 'create':
                ops.append({'op': 'insert', 'collection': COLLECTION, 'doc': op['doc']})
            else:
                ops.append({'op': 'update', 'collection': COLLECTION, 'key': op['key'], 'merge': op['doc']})
        call('/api/batch', {'ops': ops})
    else:
        raise ValueError(kind)


def check(call, phase):
    results = []
    for case in phase['cases']:
        if case['threshold'] is None:  # Direct Rust API covers None; HTTP/Lua are numeric.
            continue
        opts = {k: case[k] for k in ('model_name', 'limit', 'threshold')}
        for surface in ('http', 'lua'):
            if surface == 'http':
                response = call('/api/search/vector', {'collection': COLLECTION, 'vector': [1, 0], **opts})
                hits = response['results']
                assert response['count'] == len(hits)
            else:
                params = ', '.join(k + '=' + json.dumps(v) for k, v in opts.items() if v is not None)
                hits = call('/api/lua/execute', {'script':
                    f'return graph.similarity("{COLLECTION}", {{1,0}}, {{{params}}})'})['result'] or []
            keys = [h['document']['_key'] for h in hits]
            scores = [h['score'] for h in hits]
            passed = keys == case['keys'] and all(math.isclose(a, b, rel_tol=0, abs_tol=1e-12)
                for a, b in zip(scores, case['scores']))
            results.append({'phase': phase['name'], 'case': case['name'], 'surface': surface,
                'keys': keys, 'scores': scores, 'passed': passed})
    return results


def sidecar_files(mode):
    result = {}
    for p in (ROOT / mode).rglob('*.vectors'):
        data = p.read_bytes()
        assert data[:8] == b'CGVEC2\x00\x00'
        result[p.name] = {'file_sha256': hashlib.sha256(data).hexdigest(),
            'vectors_sha256': hashlib.sha256(data[36:]).hexdigest(),
            'generation': int.from_bytes(data[8:12], 'little')}
    return result


def vector_contents(files):
    return {name: data['vectors_sha256'] for name, data in files.items()}


def require(results, mode):
    failures = [r for r in results if not r['passed']]
    if args.expect_vulnerable and mode.endswith('sidecar'):
        assert any(r['case'] == 'outside_global_prefix' for r in failures), 'CG-34 not reproduced'
    else:
        assert not failures, (mode, failures)


def main():
    base, final = FIXTURE['phases'][0], FIXTURE['phases'][-1]
    report = {'binary_sha256': hashlib.sha256(BINARY.read_bytes()).hexdigest(),
        'fixture_sha256': hashlib.sha256(FIXTURE_PATH.read_bytes()).hexdigest(),
        'expect_vulnerable': args.expect_vulnerable, 'runs': []}
    if args.seed_binary:
        report['seed_binary_sha256'] = hashlib.sha256(args.seed_binary.read_bytes()).hexdigest()
    for mode in MODES:
        # The seed process always executes a search, creating its base sidecar.
        with server(mode, args.seed_binary or BINARY) as call:
            seed(call)
            seeded = check(call, base)
        initial_files = sidecar_files(mode)
        if mode.endswith('sidecar'):
            assert initial_files, 'base sidecar was not created'
        with server(mode) as call:
            results = check(call, base)
            active_files = sidecar_files(mode)
            # Auth collection ensures currently write a new revision on startup
            # (CG-35). Check vector payload continuity across that rebuild;
            # direct Rust tests separately assert unchanged-revision warm reuse.
            assert vector_contents(active_files) == vector_contents(initial_files)
            if args.expect_vulnerable:
                assert results == seeded
            for phase in FIXTURE['phases'][1:]:
                for action in phase['actions']:
                    apply(call, action)
                results += check(call, phase)
                if phase['name'] != 'rebuild':
                    assert sidecar_files(mode) == active_files, 'delta write rebuilt the base early'
            rebuilt_files = sidecar_files(mode)
            if mode.endswith('sidecar'):
                assert rebuilt_files != active_files, 'delta threshold did not rebuild the base'
            document = call('/api/documents/' + COLLECTION + '/b0')
            assert document['model_name'] == 'model-b'
        with server(mode) as call:
            restarted = check(call, final)
            assert restarted == [r for r in results if r['phase'] == final['name']]
            restarted_files = sidecar_files(mode)
            assert vector_contents(restarted_files) == vector_contents(rebuilt_files)
        require(results, mode)
        require_restarted = [r for r in restarted if not r['passed']]
        if not args.expect_vulnerable or mode == 'resident-embedded':
            assert not require_restarted
        report['runs'].append({'mode': mode, 'seed_observations': seeded,
            'seed_vector_bytes_preserved': True, 'delta_kept_base_files': True,
            'restart_vector_bytes_preserved': True, 'seed_files': initial_files,
            'startup_files': active_files, 'rebuilt_files': rebuilt_files,
            'restart_files': restarted_files, 'results': results, 'after_restart': restarted})
        print(mode, 'checks:', len(results) + len(restarted),
            'mismatches:', sum(not r['passed'] for r in results + restarted), flush=True)
    args.output.write_text(json.dumps(report, indent=2) + '\n')
    print('PASS; artifact:', args.output, 'logs:', ROOT)


if __name__ == '__main__':
    main()
