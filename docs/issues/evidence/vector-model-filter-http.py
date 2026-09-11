"""CG-20 live Arango indexed/fallback regression; also probes CG-34 Native parity.

Requires an EMPTY disposable Arango database with --vector-index true.
Credentials come from ARANGO_USER/ARANGO_PASSWORD. Only synthetic rows are used.
The caller owns Arango cleanup; this script stops its own servers and proxy.
"""
import argparse
import base64
import contextlib
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import math
import os
from pathlib import Path
import signal
import socket
import subprocess
import tempfile
import threading
import time
import urllib.error
import urllib.parse
import urllib.request

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--binary', type=Path)
parser.add_argument('--arango-url', required=True)
parser.add_argument('--arango-db', required=True)
parser.add_argument('--expect-vulnerable', action='store_true')
parser.add_argument('--output', required=True, type=Path)
args = parser.parse_args()
assert urllib.parse.urlparse(args.arango_url).hostname in ('127.0.0.1', 'localhost', '::1')
REPO = Path(__file__).resolve().parents[3]
BINARY = (args.binary or REPO / 'target/release/cognigraph-server').resolve()
FIXTURE_PATH = REPO / 'crates/cognigraph-arango/tests/fixtures/vector-model-filter.json'
FIXTURE = json.loads(FIXTURE_PATH.read_text())
ROOT = Path(tempfile.mkdtemp(prefix='cognigraph-cg20-live-'))
TRACES = []


def arango(path, body=None):
    auth = base64.b64encode((os.environ.get('ARANGO_USER', 'root') + ':'
                            + os.environ['ARANGO_PASSWORD']).encode()).decode()
    request = urllib.request.Request(args.arango_url + path,
        data=None if body is None else json.dumps(body).encode(),
        headers={'Content-Type': 'application/json', 'Authorization': 'Basic ' + auth})
    with urllib.request.urlopen(request, timeout=30) as response:
        return json.load(response)


class Proxy(BaseHTTPRequestHandler):
    def forward(self):
        body = self.rfile.read(int(self.headers.get('Content-Length', 0)))
        if self.command == 'POST' and self.path.endswith('/_api/cursor'):
            query = json.loads(body)
            if 'COSINE' in query.get('query', ''):
                TRACES.append(query)
        headers = {k: v for k, v in self.headers.items()
                   if k.lower() not in ('host', 'connection', 'content-length', 'accept-encoding')}
        request = urllib.request.Request(args.arango_url + self.path,
            data=body if body else None, method=self.command, headers=headers)
        try:
            response = urllib.request.urlopen(request, timeout=30)
        except urllib.error.HTTPError as error:
            response = error
        with response:
            data = response.read()
            self.send_response(response.code)
            self.send_header('Content-Type', 'application/json')
            self.send_header('Content-Length', str(len(data)))
            self.end_headers()
            self.wfile.write(data)

    do_GET = do_POST = do_PUT = do_PATCH = do_DELETE = forward

    def log_message(self, *unused):
        pass


@contextlib.contextmanager
def server(mode, proxy_port):
    directory = ROOT / mode
    directory.mkdir(exist_ok=True)
    with socket.socket() as sock:
        sock.bind(('127.0.0.1', 0))
        port = sock.getsockname()[1]
    token = None
    env = {k: os.environ[k] for k in ('PATH', 'HOME', 'TMPDIR') if k in os.environ}
    env.update(COGNIGRAPH_HOST='127.0.0.1', COGNIGRAPH_PORT=str(port),
        COGNIGRAPH_EMBEDDING_PROVIDER='none', COGNIGRAPH_AUTH_ENABLED='true',
        COGNIGRAPH_ADMIN_PASSWORD='synthetic-regression-password',
        COGNIGRAPH_JWT_SECRET='synthetic-loopback-secret')
    if mode.startswith('arango-'):
        env.update(COGNIGRAPH_BACKEND='arango', ARANGO_URL=f'http://127.0.0.1:{proxy_port}',
            ARANGO_DB=args.arango_db, ARANGO_USER=os.environ.get('ARANGO_USER', 'root'),
            ARANGO_PASSWORD=os.environ['ARANGO_PASSWORD'], COGNIGRAPH_VECTOR_SEARCH_MODE=mode.split('-')[1])
    else:
        env.update(COGNIGRAPH_BACKEND='native', COGNIGRAPH_NATIVE_PATH=str(directory / 'db.redb'),
            COGNIGRAPH_STORAGE_MODE=mode.split('-')[0], COGNIGRAPH_VECTOR_MODE=mode.split('-')[1])

    def call(path, body=None):
        headers = {'Content-Type': 'application/json'}
        if token:
            headers['Authorization'] = 'Bearer ' + token
        request = urllib.request.Request(f'http://127.0.0.1:{port}' + path,
            data=None if body is None else json.dumps(body).encode(), headers=headers)
        with urllib.request.urlopen(request, timeout=30) as response:
            assert response.status == 200
            return json.load(response)

    with (directory / 'server.log').open('a') as log:
        process = subprocess.Popen([str(BINARY)], cwd=directory, env=env, stdout=log, stderr=log)
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


def seed(call, collection, indexed):
    for row in FIXTURE['rows']:
        call('/api/documents', {'collection': collection, **row})
    if indexed:
        index = arango('/_db/' + args.arango_db + '/_api/index?collection=' + collection, {
            'type': 'vector', 'name': 'cg20_vector', 'fields': ['embedding'],
            'params': {'dimension': 2, 'metric': 'cosine', 'nLists': 1}})
        assert index.get('trainingState') == 'ready', index
        return index
    return None


def check(call, collection, cases):
    observations = []
    for case in cases:
        # None threshold is covered by the direct Rust API; HTTP and Lua
        # currently expose a numeric threshold only.
        if case['threshold'] is None:
            continue
        opts = {k: case[k] for k in ('threshold', 'limit', 'model_name')}
        for surface in ('http', 'lua'):
            before = len(TRACES)
            if surface == 'http':
                response = call('/api/search/vector', {'collection': collection,
                    'vector': FIXTURE['query_vector'], **opts})
                hits = response['results']
                assert response['count'] == len(hits)
            else:
                lua_opts = ', '.join(k + '=' + json.dumps(v) for k, v in opts.items() if v is not None)
                hits = call('/api/lua/execute', {'script':
                    f'return graph.similarity("{collection}", {{1,0}}, {{{lua_opts}}})'})['result'] or []
            keys = [hit['document']['_key'] for hit in hits]
            scores = [hit['score'] for hit in hits]
            passed = keys == case['expected_keys'] and all(math.isclose(a, b, abs_tol=1e-6, rel_tol=0)
                for a, b in zip(scores, case['expected_scores']))
            traces = TRACES[before:]
            observations.append({'case': case['name'], 'surface': surface, 'passed': passed,
                'keys': keys, 'scores': scores, 'candidate_windows': [t['bindVars']['fetch_limit'] for t in traces]})
    return observations


def main(proxy_port):
    db = '/_db/' + args.arango_db
    assert all(c['isSystem'] for c in arango(db + '/_api/collection')['result']), 'Use an empty database'
    report = {'binary_sha256': hashlib.sha256(BINARY.read_bytes()).hexdigest(),
        'fixture_sha256': hashlib.sha256(FIXTURE_PATH.read_bytes()).hexdigest(),
        'expect_vulnerable': args.expect_vulnerable, 'arango': arango('/_api/version'), 'runs': []}
    modes = ['arango-native', 'arango-fallback', 'resident-embedded', 'resident-sidecar', 'paged-sidecar']
    for mode in modes:
        collection = 'cg20_' + mode.replace('-', '_')
        cases = FIXTURE['cases'] if mode.startswith('arango-') else FIXTURE['cases'][:1]
        with server(mode, proxy_port) as call:
            index = seed(call, collection, mode.startswith('arango-'))
            results = check(call, collection, cases)
        with server(mode, proxy_port) as call:
            restarted = check(call, collection, cases)
        assert results == restarted, (mode, 'restart changed results')
        failures = [r for r in results if not r['passed']]
        if args.expect_vulnerable and mode in ('resident-sidecar', 'paged-sidecar'):
            assert len(failures) == 2 and all(not r['keys'] for r in failures), 'CG-34 not reproduced'
        elif args.expect_vulnerable and mode.startswith('arango-'):
            name = 'model_outside_global_prefix' if mode == 'arango-native' else 'duplicates_do_not_starve_other_parents'
            assert any(r['case'] == name for r in failures), (mode, 'baseline not reproduced')
        else:
            assert not failures, (mode, failures)
        report['runs'].append({'mode': mode, 'index': index, 'restart_equal': True, 'results': results})
        print(mode, 'checks per lifetime:', len(results), 'mismatches:', len(failures), flush=True)
    # Explain the actual SQL-like query captured from the release process.
    indexed = next(t for t in TRACES if 'APPROX_NEAR_COSINE' in t['query'] and t['bindVars'].get('model_name') == 'model-b')
    explained = arango(db + '/_api/explain', indexed)
    nodes = explained['plan']['nodes']
    assert any(n['type'] == 'EnumerateNearVectorNode' for n in nodes), [n['type'] for n in nodes]
    report['indexed_plan'] = {'query': indexed['query'], 'bindVars': indexed['bindVars'],
        'nodes': nodes, 'rules': explained['plan']['rules']}
    if not args.expect_vulnerable:
        assert indexed['query'].index('FILTER embed.model_name') < indexed['query'].index('LIMIT')
        for run in report['runs'][:2]:
            duplicate = next(r for r in run['results'] if r['case'] == 'duplicates_do_not_starve_other_parents')
            assert max(duplicate['candidate_windows']) > 3 * 4
    args.output.write_text(json.dumps(report, indent=2) + '\n')
    print('PASS; artifacts:', args.output, 'server logs:', ROOT)


if __name__ == '__main__':
    proxy = ThreadingHTTPServer(('127.0.0.1', 0), Proxy)
    thread = threading.Thread(target=proxy.serve_forever, daemon=True)
    thread.start()
    try:
        main(proxy.server_port)
    finally:
        proxy.shutdown()
        proxy.server_close()
        thread.join()
