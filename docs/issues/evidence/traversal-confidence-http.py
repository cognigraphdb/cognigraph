"""CG-19 release HTTP/Lua parity regression using the shared Rust contract fixture.

Provide a loopback Arango URL and an EMPTY disposable database. Credentials are
read from ARANGO_USER/ARANGO_PASSWORD. Native stores use temporary directories.
--expect-vulnerable checks the saved pre-fix binary and records its mismatches.
No external model calls are made. The caller owns Arango database cleanup.
"""
import argparse
import base64
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
import urllib.parse
import urllib.request

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--binary', type=Path)
parser.add_argument('--arango-url', required=True)
parser.add_argument('--arango-db', required=True)
parser.add_argument('--expect-vulnerable', action='store_true')
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
assert urllib.parse.urlparse(args.arango_url).hostname in ('127.0.0.1', 'localhost', '::1')
REPO = Path(__file__).resolve().parents[3]
BINARY = (args.binary or REPO / 'target/release/cognigraph-server').resolve()
FIXTURE_PATH = REPO / 'crates/cognigraph-core/src/contract/traversal-confidence.json'
FIXTURE = json.loads(FIXTURE_PATH.read_text())
ROOT = Path(tempfile.mkdtemp(prefix='cognigraph-cg19-live-'))
MODES = ['resident-embedded', 'resident-sidecar', 'paged-sidecar', 'arango']
COLLECTION = 'cg19_http_docs'
EDGES = 'cg19_http_edges'


def arango_info():
    auth = base64.b64encode((os.environ.get('ARANGO_USER', 'root') + ':'
                            + os.environ['ARANGO_PASSWORD']).encode()).decode()
    headers = {'Authorization': 'Basic ' + auth}
    req = urllib.request.Request(args.arango_url + '/_api/version', headers=headers)
    with urllib.request.urlopen(req, timeout=10) as response:
        version = json.load(response)
    req = urllib.request.Request(args.arango_url + '/_db/' + args.arango_db
                                 + '/_api/collection', headers=headers)
    with urllib.request.urlopen(req, timeout=10) as response:
        collections = json.load(response)['result']
    assert all(row['isSystem'] for row in collections), 'Use an empty disposable Arango database'
    return version


@contextlib.contextmanager
def server(mode):
    directory = ROOT / mode
    directory.mkdir(exist_ok=True)
    with socket.socket() as sock:
        sock.bind(('127.0.0.1', 0))
        port = sock.getsockname()[1]
    token = None
    env = {key: os.environ[key] for key in ('PATH', 'HOME', 'TMPDIR') if key in os.environ}
    env.update(COGNIGRAPH_HOST='127.0.0.1', COGNIGRAPH_PORT=str(port),
        COGNIGRAPH_EMBEDDING_PROVIDER='none', COGNIGRAPH_AUTH_ENABLED='true',
        COGNIGRAPH_ADMIN_PASSWORD='synthetic-regression-password',
        COGNIGRAPH_JWT_SECRET='synthetic-loopback-secret')
    if mode == 'arango':
        env.update(COGNIGRAPH_BACKEND='arango', ARANGO_URL=args.arango_url,
            ARANGO_DB=args.arango_db, ARANGO_USER=os.environ.get('ARANGO_USER', 'root'),
            ARANGO_PASSWORD=os.environ['ARANGO_PASSWORD'], COGNIGRAPH_VECTOR_SEARCH_MODE='fallback')
    else:
        env.update(COGNIGRAPH_BACKEND='native',
            COGNIGRAPH_NATIVE_PATH=str(directory / 'db.redb'),
            COGNIGRAPH_STORAGE_MODE=mode.split('-')[0], COGNIGRAPH_VECTOR_MODE=mode.split('-')[1])

    def call(path, body=None, method=None):
        headers = {'Content-Type': 'application/json'}
        if token:
            headers['Authorization'] = 'Bearer ' + token
        req = urllib.request.Request(f'http://127.0.0.1:{port}' + path,
            data=None if body is None else json.dumps(body).encode(), method=method, headers=headers)
        try:
            with urllib.request.urlopen(req, timeout=15) as response:
                return response.status, json.load(response)
        except urllib.error.HTTPError as error:
            return error.code, json.loads(error.read())

    with (directory / 'server.log').open('a') as log:
        process = subprocess.Popen([str(BINARY)], cwd=directory, env=env, stdout=log, stderr=log)
        try:
            for _ in range(200):
                if process.poll() is not None:
                    raise RuntimeError(f'startup failed: {directory / "server.log"}')
                try:
                    if call('/health')[0] == 200:
                        break
                except (OSError, urllib.error.URLError):
                    time.sleep(0.1)
            else:
                raise RuntimeError('server startup timed out')
            token = ok(call, '/api/auth/login', {
                'username': 'admin', 'password': 'synthetic-regression-password'})['token']
            yield call
        finally:
            if process.poll() is None:
                process.send_signal(signal.SIGTERM)
                try:
                    process.wait(timeout=8)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()


def ok(call, path, body=None, method=None):
    status, result = call(path, body, method)
    assert status == 200, (path, status, result)
    return result


def seed(call):
    for key in ['a', *(edge['to'] for edge in FIXTURE['edges'])]:
        ok(call, '/api/documents', {'collection': COLLECTION, '_key': key, 'label': 'synthetic CG-19'})
    for edge in FIXTURE['edges']:
        endpoints = {'_from': COLLECTION + '/' + edge['from'], '_to': COLLECTION + '/' + edge['to']}
        created = ok(call, '/api/graph/relationships', {'collection': EDGES,
            'from': endpoints['_from'], 'to': endpoints['_to'], 'relation_type': 'synthetic_cg19'})
        # Replace through the public document route to preserve arbitrary JSON
        # confidence values; relationship creation accepts only a numeric value.
        data = {**endpoints, 'relation_type': 'synthetic_cg19'}
        if 'confidence' in edge:
            data['confidence'] = edge['confidence']
        ok(call, '/api/documents/' + EDGES + '/' + created['_key'], data, 'PUT')


def inspect(paths):
    actual = {}
    for path in paths or []:  # Lua serializes an empty sequence as null.
        assert path['depth'] == len(path['edges'] or [])
        assert len(path['vertices']) == path['depth'] + 1
        key = '/'.join(v['_key'] for v in path['vertices'])
        assert key not in actual, ('duplicate path', key)
        actual[key] = path['score']
    return actual


def check_cases(call):
    results = []
    for case in FIXTURE['cases']:
        opts = {key: case[key] for key in ('min_depth', 'max_depth', 'direction', 'min_confidence', 'path_decay')}
        opts['edge_collection'] = EDGES
        start = COLLECTION + '/' + case['start']
        response = ok(call, '/api/graph/traverse', {'start_vertex': start, **opts})
        assert response['count'] == len(response['results'])
        lua_opts = ', '.join(key + '=' + json.dumps(value) for key, value in opts.items() if value is not None)
        lua = ok(call, '/api/lua/execute', {'script': f'return graph.traverse("{start}", {{{lua_opts}}})'})
        for surface, paths in [('http', response['results']), ('lua', lua['result'])]:
            actual = inspect(paths)
            expected = case['expected']
            passed = actual.keys() == expected.keys() and all(
                math.isclose(actual[key], score, abs_tol=1e-12, rel_tol=0) for key, score in expected.items())
            results.append({'case': case['name'], 'surface': surface, 'passed': passed, 'actual': actual})
    return results


def main():
    report = {'date': time.strftime('%Y-%m-%d'), 'binary': str(BINARY),
        'binary_sha256': hashlib.sha256(BINARY.read_bytes()).hexdigest(),
        'fixture_sha256': hashlib.sha256(FIXTURE_PATH.read_bytes()).hexdigest(),
        'expect_vulnerable': args.expect_vulnerable, 'arango': arango_info(), 'runs': []}
    for mode in MODES:
        with server(mode) as call:
            seed(call)
            results = check_cases(call)
        with server(mode) as call:
            restarted = check_cases(call)
        assert results == restarted, (mode, 'restart changed results')
        failures = [row for row in results if not row['passed']]
        if args.expect_vulnerable and mode == 'arango':
            for name in ['per_edge_inclusive_threshold', 'reject_low_intermediate_below_min_depth',
                         'defaults_are_one', 'depth_zero_only']:
                assert any(row['case'] == name for row in failures), (name, 'regression not reproduced')
        else:
            assert not failures, (mode, failures)
        report['runs'].append({'mode': mode, 'restart_equal': True,
            'checks_before_restart': len(results), 'failures_before_restart': len(failures), 'results': results})
        print(f'{mode}: {len(results)} checks before and after restart; {len(failures)} expected mismatches', flush=True)
    args.output.write_text(json.dumps(report, indent=2) + '\n')
    print('PASS:', args.output, 'logs:', ROOT)


if __name__ == '__main__':
    main()
