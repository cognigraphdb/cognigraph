"""CG-32: real semantic and graph-cache regression, using only disposable loopback services.
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
ROOT = Path(tempfile.mkdtemp(prefix='cognigraph-cg32-live-'))
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

MIDDLE = ';t=0.7;seeds=5;l=10;depth=2;dir=outbound;minc=;decay=0.8;neurons='
GRAPH_LEFT = {'query':'needle', 'edge_collection':'edges'+MIDDLE+'first', 'neurons_collection':'second', 'graph_facts_limit':10}
GRAPH_RIGHT = {'query':'needle', 'edge_collection':'edges', 'neurons_collection':'first'+MIDDLE+'second', 'graph_facts_limit':10}
ALL_MODELS = ['models/empty', 'models/missing', 'models/named']


def ids(response):
    return sorted(row['document_id'] for row in response['results'])


def semantic_request(model, query='needle'):
    request = {'query':query, 'collection':'models'}
    if model is not None:
        request['model_name'] = model
    return request


def expected_models(model):
    return ALL_MODELS if model is None else ['models/empty']


def exercise_semantic(call, left_model, right_model, query):
    ok(call, '/api/cache/clear', {})
    left = semantic_request(left_model)
    right = semantic_request(right_model, query)
    warm = ok(call, '/api/search/semantic', left)
    assert ids(warm) == expected_models(left_model) and 'cached' not in warm, warm
    strong = ok(call, '/api/search/semantic', {**left, 'query':'alias'})
    assert strong['cached'] is True and ids(strong) == expected_models(left_model), strong
    fresh = ok(call, '/api/search/semantic', right)
    if args.expect_vulnerable:
        expected = ALL_MODELS if query == 'related' else expected_models(left_model)
        assert ids(fresh) == expected and fresh['cached'] == ('assisted' if query == 'related' else True), fresh
    else:
        assert ids(fresh) == expected_models(right_model) and 'cached' not in fresh, fresh
    repeat = ok(call, '/api/search/semantic', right)
    assert repeat['cached'] is True and repeat['results'] == fresh['results'], repeat
    if not args.expect_vulnerable:
        assisted = ok(call, '/api/search/semantic', {**left, 'query':'related'})
        assert assisted['cached'] == 'assisted' and ids(assisted) == expected_models(left_model), assisted
        repeat = ok(call, '/api/search/semantic', {**left, 'query':'related'})
        assert repeat['cached'] is True and repeat['results'] == assisted['results'], repeat
    return {'from_filter':'omitted' if left_model is None else 'empty', 'to_filter':'omitted' if right_model is None else 'empty', 'query':query, 'changed_request_cached':fresh.get('cached',False), 'returned_ids':ids(fresh), 'exact_repeat':True, 'valid_strong_similarity':True, 'valid_assisted_checked':not args.expect_vulnerable}


def exercise_graph(call, query):
    ok(call, '/api/cache/clear', {})
    warm = ok(call, '/api/search/graph-augmented', GRAPH_LEFT)
    assert ids(warm) == ['graphdocs/left','graphdocs/root'] and len(warm['graph_facts']) == 1 and warm['graph_facts'][0]['relation'] == 'LEFT', warm
    strong = ok(call, '/api/search/graph-augmented', {**GRAPH_LEFT, 'query':'alias'})
    assert strong['cached'] is True and strong['graph_facts'] == warm['graph_facts'], strong
    right = {**GRAPH_RIGHT, 'query':query}
    fresh = ok(call, '/api/search/graph-augmented', right)
    if args.expect_vulnerable:
        expected = ['graphdocs/left','graphdocs/right','graphdocs/root'] if query == 'related' else ['graphdocs/left','graphdocs/root']
        relation = 'RIGHT' if query == 'related' else 'LEFT'
        assert ids(fresh) == expected and fresh['cached'] == ('assisted' if query == 'related' else True), fresh
    else:
        relation = 'RIGHT'
        assert ids(fresh) == ['graphdocs/right','graphdocs/root'] and 'cached' not in fresh, fresh
    assert len(fresh['graph_facts']) == 1 and fresh['graph_facts'][0]['relation'] == relation, fresh
    repeat = ok(call, '/api/search/graph-augmented', right)
    assert repeat['cached'] is True and repeat['results'] == fresh['results'] and repeat['graph_facts'] == fresh['graph_facts'], repeat
    if not args.expect_vulnerable:
        similar = {**GRAPH_LEFT,'query':'related'}
        assisted = ok(call, '/api/search/graph-augmented', similar)
        assert assisted['cached'] == 'assisted' and ids(assisted) == ['graphdocs/left','graphdocs/root'], assisted
        assert assisted['graph_facts'] == warm['graph_facts'], assisted
        repeat = ok(call, '/api/search/graph-augmented', similar)
        assert repeat['cached'] is True and repeat['results'] == assisted['results'] and repeat['graph_facts'] == assisted['graph_facts'], repeat
    return {'query':query, 'changed_request_cached':fresh.get('cached',False), 'returned_ids':ids(fresh), 'graph_fact_relation':relation, 'exact_repeat':True, 'valid_strong_similarity':True, 'valid_assisted_checked':not args.expect_vulnerable}


def seed(call):
    for doc in [
        {'_key':'named','model_name':'alpha','embedding':[1.0,0.0]},
        {'_key':'empty','model_name':'','embedding':[1.0,0.0]},
        {'_key':'missing','embedding':[1.0,0.0]},
    ]:
        ok(call, '/api/documents', {'collection':'models', **doc})
    for key in ('root','left','right'):
        ok(call, '/api/documents', {'collection':'graphdocs','_key':key})
    ok(call, '/api/documents', {'collection':'embeddings','_key':'seed','document_id':'graphdocs/root','embedding':[1.0,0.0]})
    for request,target,relation in [(GRAPH_LEFT,'graphdocs/left','LEFT'),(GRAPH_RIGHT,'graphdocs/right','RIGHT')]:
        ok(call, '/api/graph/relationships', {'collection':request['edge_collection'],'from':'graphdocs/root','to':target,'relation_type':relation,'metadata':{'evidence_chunk_id':relation}})


evidence = {'issue':'CG-32','date':'2026-09-08','binary_profile':'release','binary_sha256':hashlib.sha256(BINARY.read_bytes()).hexdigest(),'provider':'synthetic loopback HTTP; needle/alias cosine 1.0, needle/related cosine 0.8','expected':'vulnerable baseline' if args.expect_vulnerable else 'isolated request parameters','configurations':{}}
try:
    for mode in ('resident','paged'):
        for cache_backend in ('memory','persistent'):
            observations = {}
            with server(mode, cache_backend) as call:
                seed(call)
                observations['semantic'] = [exercise_semantic(call, left, right, query) for left,right in [(None,''),('',None)] for query in ('needle','alias','related')]
                observations['graph'] = [exercise_graph(call, query) for query in ('needle','alias','related')]
                calls_before_restart = len(PROVIDER_CALLS)
            with server(mode, cache_backend) as call:
                reopened = ok(call, '/api/search/graph-augmented', {**GRAPH_LEFT,'query':'related'})
                assert 'cached' not in reopened and ids(reopened) == ['graphdocs/left','graphdocs/root'], reopened
                assert reopened['graph_facts'][0]['relation'] == 'LEFT', reopened
                provider_delta = len(PROVIDER_CALLS) - calls_before_restart
                assert provider_delta == (0 if cache_backend == 'persistent' else 1), provider_delta
                observations['restart'] = {'result_cache_cold':True,'provider_calls_for_warm_embedding':provider_delta,'semantic':exercise_semantic(call,None,'','related'),'graph':exercise_graph(call,'related')}
            evidence['configurations'][mode+'/'+cache_backend] = observations
    evidence['result'] = 'EXPECTED DEFECT REPRODUCED' if args.expect_vulnerable else 'PASS'
    (ROOT/'results.json').write_text(json.dumps(evidence,indent=2)+'\n')
    print(json.dumps({'result':evidence['result'],'evidence':str(ROOT/'results.json')},indent=2))
finally:
    mock.shutdown()
