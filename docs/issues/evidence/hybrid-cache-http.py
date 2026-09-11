"""CG-31: real hybrid-cache regression, using only disposable loopback services.
Build the release server, then run this script. --binary PATH selects another
executable; --expect-vulnerable asserts the saved pre-fix collision behavior.
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
import threading
import time
import urllib.error
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

parser = argparse.ArgumentParser()
parser.add_argument('--binary', type=Path)
parser.add_argument('--expect-vulnerable', action='store_true')
args = parser.parse_args()
BINARY = args.binary or Path(__file__).resolve().parents[3] / 'target/release/cognigraph-server'
ROOT = Path(tempfile.mkdtemp(prefix='cognigraph-cg31-live-'))
PROVIDER_CALLS = []


class Mock(BaseHTTPRequestHandler):
    def do_POST(self):
        request = json.loads(self.rfile.read(int(self.headers.get('Content-Length', 0))))
        PROVIDER_CALLS.extend(request['input'])
        data = json.dumps({'data': [{'embedding': [0.8, 0.6] if q == 'related' else [1.0, 0.0]} for q in request['input']]}).encode()
        self.send_response(200)
        self.send_header('Content-Type', 'application/json')
        self.end_headers()
        self.wfile.write(data)

    def log_message(self, *args):
        pass


mock = ThreadingHTTPServer(('127.0.0.1', 0), Mock)
threading.Thread(target=mock.serve_forever, daemon=True).start()


@contextlib.contextmanager
def server(storage_mode, cache_backend):
    directory = ROOT / storage_mode / cache_backend
    directory.mkdir(parents=True, exist_ok=True)
    with socket.socket() as sock:
        sock.bind(('127.0.0.1', 0))
        port = sock.getsockname()[1]
    env = {k: os.environ[k] for k in ('PATH', 'HOME', 'TMPDIR') if k in os.environ}
    env.update(
        COGNIGRAPH_HOST='127.0.0.1', COGNIGRAPH_PORT=str(port),
        COGNIGRAPH_NATIVE_PATH=str(directory / 'db.redb'),
        COGNIGRAPH_STORAGE_MODE=storage_mode, COGNIGRAPH_VECTOR_MODE='sidecar',
        COGNIGRAPH_EMBEDDING_PROVIDER='openai', COGNIGRAPH_AUTH_ENABLED='true',
        COGNIGRAPH_ADMIN_PASSWORD='synthetic-regression-password',
        COGNIGRAPH_JWT_SECRET='synthetic-loopback-regression-secret',
        OPENAI_API_KEY='synthetic-stub-only', OPENAI_BASE_URL=f'http://127.0.0.1:{mock.server_port}/v1',
        COGNIGRAPH_QUERY_CACHE_ENABLED='true', COGNIGRAPH_QUERY_CACHE_BACKEND=cache_backend,
        COGNIGRAPH_QUERY_CACHE_PATH=str(directory / 'cache.redb'),
    )
    token = None

    def call(path, body=None):
        headers = {'Content-Type': 'application/json'}
        if token:
            headers['Authorization'] = 'Bearer ' + token
        request = urllib.request.Request(f'http://127.0.0.1:{port}' + path,
            data=None if body is None else json.dumps(body).encode(), headers=headers)
        try:
            with urllib.request.urlopen(request, timeout=20) as response:
                return response.status, json.load(response)
        except urllib.error.HTTPError as error:
            return error.code, json.loads(error.read())

    with open(directory / 'server.log', 'a') as log:
        process = subprocess.Popen([str(BINARY)], cwd=directory, env=env, stdout=log, stderr=log)
        try:
            for _ in range(200):
                if process.poll() is not None:
                    raise RuntimeError(f'startup failed; see {directory / "server.log"}')
                try:
                    if call('/health')[0] == 200:
                        break
                except (OSError, urllib.error.URLError):
                    time.sleep(0.1)
            else:
                raise RuntimeError('server startup timed out')
            token = ok(call, '/api/auth/login', {'username': 'admin', 'password': 'synthetic-regression-password'})['token']
            yield call
        finally:
            process.send_signal(signal.SIGTERM)
            try:
                process.wait(timeout=8)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()


def ok(call, path, body=None):
    status, result = call(path, body)
    assert status == 200, (path, status, result)
    return result


def request(**overrides):
    return {'query': 'needle', 'documents_collection': 'cg31-fields', 'search_fields': ['a,b'], 'limit': 10, **overrides}


CASES = [
    ('field_array', request(), request(search_fields=['a', 'b'])),
    ('escaped_field_array', request(documents_collection='cg31-escaped', search_fields=['a,"b\\c']), request(documents_collection='cg31-escaped', search_fields=['a', '"b\\c'])),
    ('collection_view', request(documents_collection='cg31-collections;view=west', search_view='east', search_fields=['text']), request(documents_collection='cg31-collections', search_view='west;view=east', search_fields=['text'])),
    ('view_field', request(documents_collection='cg31-views', search_view='index;fields=title', search_fields=['text']), request(documents_collection='cg31-views', search_view='index', search_fields=['title;fields=text'])),
]


def ids(response):
    return sorted(row['document_id'] for row in response['results'])


def exercise(call, left, right, query):
    ok(call, '/api/cache/clear', {})
    left_id = left['documents_collection'] + '/left'
    right_id = right['documents_collection'] + '/right'
    warm = ok(call, '/api/search/hybrid', left)
    assert ids(warm) == [left_id] and 'cached' not in warm, warm
    changed = {**right, 'query': query}
    fresh = ok(call, '/api/search/hybrid', changed)
    if args.expect_vulnerable:
        expected = sorted([left_id, right_id]) if query == 'related' else [left_id]
        assert ids(fresh) == expected, fresh
        assert fresh['cached'] == ('assisted' if query == 'related' else True), fresh
    else:
        assert ids(fresh) == [right_id] and 'cached' not in fresh, fresh
    repeat = ok(call, '/api/search/hybrid', changed)
    assert repeat['cached'] is True and repeat['results'] == fresh['results'], repeat
    if not args.expect_vulnerable:
        assisted = ok(call, '/api/search/hybrid', {**left, 'query': 'related'})
        assert ids(assisted) == [left_id] and assisted['cached'] == 'assisted', assisted
        repeat = ok(call, '/api/search/hybrid', {**left, 'query': 'related'})
        assert repeat['cached'] is True and repeat['results'] == assisted['results'], repeat
    return {'query': query, 'changed_request_cached': fresh.get('cached', False), 'returned_ids': ids(fresh), 'repeat_exact_hit': True, 'same_parameters_assisted_hit': not args.expect_vulnerable}


evidence = {'issue': 'CG-31', 'date': '2026-09-08', 'binary_profile': 'release', 'binary_sha256': hashlib.sha256(BINARY.read_bytes()).hexdigest(), 'provider': 'synthetic loopback HTTP; needle/alias cosine 1.0, needle/related cosine 0.8', 'expected': 'vulnerable baseline' if args.expect_vulnerable else 'isolated request parameters', 'configurations': {}}
try:
    for mode in ('resident', 'paged'):
        for cache_backend in ('memory', 'persistent'):
            observations = {}
            with server(mode, cache_backend) as call:
                ok(call, '/api/documents', {'collection': 'embeddings', '_key': 'empty-vector-leg'})
                for name, left, right in CASES:
                    for req, key in ((left, 'left'), (right, 'right')):
                        ok(call, '/api/documents', {'collection': req['documents_collection'], '_key': key, req['search_fields'][0]: 'needle alias related'})
                    observations[name] = [exercise(call, left, right, query) for query in ('needle', 'alias', 'related')]
                # Retain the last warmed query embedding across this restart.
                calls_before_restart = len(PROVIDER_CALLS)
            with server(mode, cache_backend) as call:
                last = {**CASES[-1][1], 'query': 'related'}
                reopened = ok(call, '/api/search/hybrid', last)
                assert 'cached' not in reopened and ids(reopened) == [last['documents_collection'] + '/left'], reopened
                provider_delta = len(PROVIDER_CALLS) - calls_before_restart
                assert provider_delta == (0 if cache_backend == 'persistent' else 1), provider_delta
                observations['restart'] = {'result_cache_cold': True, 'provider_calls_for_warm_embedding': provider_delta, 'collision_regression': exercise(call, CASES[0][1], CASES[0][2], 'related')}
            evidence['configurations'][mode + '/' + cache_backend] = observations
    evidence['result'] = 'EXPECTED DEFECT REPRODUCED' if args.expect_vulnerable else 'PASS'
    (ROOT / 'results.json').write_text(json.dumps(evidence, indent=2) + '\n')
    print(json.dumps({'result': evidence['result'], 'evidence': str(ROOT / 'results.json')}, indent=2))
finally:
    mock.shutdown()
