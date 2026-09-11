"""CG-13 release HTTP regression. Uses disposable stores and synthetic loopback providers.

Build the release server first. --binary PATH --expect-vulnerable reproduces
bypassed cascades and publication races in the saved pre-fix release binary.
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
from urllib.parse import quote
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--binary', type=Path)
parser.add_argument('--expect-vulnerable', action='store_true')
parser.add_argument('--expect-nfc-rejection', action='store_true')
parser.add_argument('--output', type=Path)
args = parser.parse_args()
REPO = Path(__file__).resolve().parents[3]
BINARY = (args.binary or REPO / 'target/release/cognigraph-server').resolve()
ROOT = Path(tempfile.mkdtemp(prefix='cognigraph-cg13-live-'))
MODES = ['resident-embedded', 'resident-sidecar', 'paged-sidecar']
METHODS = ['direct', 'batch', 'cgql', 'lua_delete', 'lua_batch', 'lua_query', 'drop']
CALLS = []
ARMED = None
QUESTION = 'Synthetic question'
FAIL_COMPLETION = False


class Mock(BaseHTTPRequestHandler):
    def do_POST(self):
        global FAIL_COMPLETION
        body = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
        stage = 'embedding' if self.path == '/api/embed' else 'completion'
        question = QUESTION
        CALLS.append({'stage': stage})
        gate = ARMED
        if gate and gate['stage'] == stage:
            gate['entered'].set()
            assert gate['release'].wait(20), 'test failed to release provider barrier'
        if stage == 'embedding':
            response = {'embeddings': [[1.0, 0.0] for _ in body['input']]}
        else:
            assert self.path == '/openai/chat/completions', self.path
            assert self.headers['Authorization'] == 'Bearer synthetic-cg13-key'
            assert body['model'] == 'gpt-5.6-luna' and body['reasoning_effort'] == 'none', body
            if FAIL_COMPLETION:
                FAIL_COMPLETION = False
                self.send_response(400)
                self.send_header('Content-Type', 'application/json')
                self.end_headers()
                self.wfile.write(b'{"error":{"message":"synthetic provider failure"}}')
                return
            response = {'choices': [{'message': {'content': json.dumps({'pairs': [
                {'question': question, 'answer': 'Synthetic answer from the supplied passage.'}
            ]})}}]}
        self.send_response(200)
        self.send_header('Content-Type', 'application/json')
        self.end_headers()
        self.wfile.write(json.dumps(response).encode())

    def log_message(self, *unused):
        pass


mock = ThreadingHTTPServer(('127.0.0.1', 0), Mock)
threading.Thread(target=mock.serve_forever, daemon=True).start()


@contextlib.contextmanager
def server(mode):
    directory = ROOT / mode
    directory.mkdir(exist_ok=True)
    with socket.socket() as sock:
        sock.bind(('127.0.0.1', 0))
        port = sock.getsockname()[1]
    token = None
    env = {key: os.environ[key] for key in ('PATH', 'HOME', 'TMPDIR') if key in os.environ}
    base = f'http://127.0.0.1:{mock.server_port}'
    env.update(COGNIGRAPH_HOST='127.0.0.1', COGNIGRAPH_PORT=str(port),
        COGNIGRAPH_NATIVE_PATH=str(directory / 'db.redb'),
        COGNIGRAPH_STORAGE_MODE=mode.split('-')[0], COGNIGRAPH_VECTOR_MODE=mode.split('-')[1],
        COGNIGRAPH_EMBEDDING_PROVIDER='ollama', OLLAMA_BASE_URL=base,
        COGNIGRAPH_AUTH_ENABLED='true', COGNIGRAPH_ADMIN_PASSWORD='synthetic-regression-password',
        COGNIGRAPH_JWT_SECRET='synthetic-loopback-secret', COGNIGRAPH_CGQL_MUTATIONS_ENABLED='true',
        COGNIGRAPH_COMPLETION_PROVIDER='openai', OPENAI_API_KEY='synthetic-cg13-key',
        OPENAI_BASE_URL=base + '/openai', COGNIGRAPH_COMPLETION_MODEL='gpt-5.6-luna')

    def call(path, body=None, method=None, headers=None):
        request_headers = {'Content-Type': 'application/json', **(headers or {})}
        if token:
            request_headers['Authorization'] = 'Bearer ' + token
        req = urllib.request.Request(f'http://127.0.0.1:{port}' + path,
            data=None if body is None else json.dumps(body).encode(), method=method, headers=request_headers)
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
            token = ok(call, '/api/auth/login', {'username': 'admin', 'password': 'synthetic-regression-password'})['token']
            yield call
        finally:
            if ARMED:
                ARMED['release'].set()
            if process.poll() is None:
                process.send_signal(signal.SIGTERM)
                try:
                    process.wait(timeout=8)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()


def ok(call, path, body=None, method=None, headers=None):
    status, result = call(path, body, method, headers)
    assert status == 200, (path, status, result)
    return result


def seed(call, collection, text='Alpha supplies Beta.'):
    return ok(call, '/api/documents', {'collection': collection, '_key': 'a', 'text': text})


def side_rows(call, collection):
    status, value = call('/api/documents?collection=side_views')
    if status == 404:
        return []
    assert status == 200, value
    return [row for row in value['results'] if row['document_id'] == collection + '/a']


def submit(call, collection, name, regenerate=False):
    status, value = call('/api/sideviews/generate', {'collection': collection, 'count': 1, 'regenerate': regenerate},
        headers={'Idempotency-Key': name})
    assert status == 202, (status, value)
    return value['job']['id']


def terminal(call, job_id):
    for _ in range(300):
        job = ok(call, '/api/jobs/' + job_id)
        if job['status'] in ('succeeded', 'failed', 'canceled'):
            return job
        time.sleep(0.025)
    raise RuntimeError('job did not finish')


def generated(call, collection, name, regenerate=False, count=1):
    job = terminal(call, submit(call, collection, name, regenerate))
    assert job['status'] == 'succeeded' and job['result']['side_views_written'] == count, job
    return job


def remove(call, collection, method):
    if method == 'direct':
        return ok(call, f'/api/documents/{collection}/a', method='DELETE')
    if method == 'drop':
        return ok(call, f'/api/collections/{collection}', method='DELETE')
    if method == 'batch':
        value = ok(call, '/api/batch', {'ops': [{'op': 'delete', 'collection': collection, 'key': 'a'}]})
        assert value['count'] == 1 and value['results'] == [None], value
        return value
    query = f'FOR d IN {collection} REMOVE d._key IN {collection}'
    if method == 'cgql':
        return ok(call, '/api/query', {'query': query})
    script = {
        'lua_delete': f'return graph.delete_document("{collection}", "a")',
        'lua_batch': f'return graph.batch({{{{op="delete", collection="{collection}", key="a"}}}})',
        'lua_query': f'return graph.query("{query}")',
    }[method]
    return ok(call, '/api/lua/execute', {'script': script})


def retry(call, job_id, name):
    value = ok(call, f'/api/jobs/{job_id}/retry', {'mode': 'resume'}, headers={'Idempotency-Key': name})
    assert value['job']['id'] == job_id, value
    return terminal(call, job_id)


evidence = {'issue': 'CG-13', 'date': '2026-09-09', 'binary_profile': 'release',
    'binary_sha256': hashlib.sha256(BINARY.read_bytes()).hexdigest(),
    'expected': 'vulnerable baseline' if args.expect_vulnerable else 'fenced side-view lifecycle',
    'providers': 'synthetic loopback HTTP only; Luna settings unchanged', 'modes': {}}
try:
    for mode in MODES:
        result = {'delete_surfaces': {}, 'publication_races': [], 'restart': False}
        with server(mode) as call:
            for method in METHODS:
                collection = 'basic_' + method
                seed(call, collection)
                generated(call, collection, mode + collection)
                assert len(side_rows(call, collection)) == 1
                response = remove(call, collection, method)
                remaining = len(side_rows(call, collection))
                expected = int(args.expect_vulnerable and method != 'direct')
                assert remaining == expected, (mode, method, remaining)
                if method == 'direct':
                    assert response['side_views_deleted'] == 1 and response['deleted'], response
                assert call(f'/api/documents/{collection}/a')[0] == 404
                result['delete_surfaces'][method] = {'remaining': remaining, 'http': 200}

            for method in METHODS:
                for stage in ['completion', 'embedding']:
                    collection = f'race_{method}_{stage}'
                    recreate = stage == 'embedding'
                    seed(call, collection)
                    QUESTION = 'Stale ' + collection
                    ARMED = {'stage': stage, 'entered': threading.Event(), 'release': threading.Event()}
                    job_id = submit(call, collection, mode + collection)
                    assert ARMED['entered'].wait(10), (method, stage, 'provider not reached')
                    # This must finish while the provider is still blocked.
                    remove(call, collection, method)
                    if recreate:
                        seed(call, collection, 'Recreated source contains a different fact.')
                    ARMED['release'].set()
                    job = terminal(call, job_id)
                    ARMED = None
                    remaining = len(side_rows(call, collection))
                    if args.expect_vulnerable:
                        assert job['status'] == 'succeeded' and remaining == 1, job
                        result['publication_races'].append({'method': method, 'provider_wait': stage,
                            'recreated': recreate, 'job_status': job['status'], 'stale_rows': remaining})
                        continue
                    assert job['status'] == 'failed' and 'invalidated during generation' in json.dumps(job['error']), job
                    assert remaining == 0, (method, stage, remaining)
                    QUESTION = 'Fresh ' + collection
                    resumed = retry(call, job_id, mode + collection + '-retry')
                    assert resumed['status'] == 'succeeded', resumed
                    assert resumed['result']['side_views_written'] == int(recreate), resumed
                    if recreate:
                        fresh = side_rows(call, collection)
                        assert len(fresh) == 1 and fresh[0]['question'] == QUESTION, fresh
                        remove(call, collection, 'direct')
                    result['publication_races'].append({'method': method, 'provider_wait': stage,
                        'recreated': recreate, 'job_status': job['status'], 'stale_rows': 0,
                        'retry_status': resumed['status'], 'retry_written': int(recreate)})

            if not args.expect_vulnerable:
                source_key = 'café'
                ok(call, '/api/documents', {'collection':'unicode_source','_key':source_key,'text':'Synthetic source.'})
                calls_before = len(CALLS)
                status, value = call('/api/sideviews/generate', {'collection':'unicode_source','count':1},
                    headers={'Idempotency-Key':mode + '-unicode-identity'})
                if args.expect_nfc_rejection:
                    assert status == 400 and 'NFC Unicode' in value['error'] and len(CALLS) == calls_before, (status, value)
                    result['noncanonical_identity_rejected_before_provider'] = True
                else:
                    assert status == 202, (status, value)
                    job = terminal(call, value['job']['id'])
                    assert job['status'] == 'succeeded' and job['result']['side_views_written'] == 1, job
                    found = [r for r in ok(call, '/api/documents?collection=side_views')['results']
                             if r['document_id'] == 'unicode_source/' + source_key]
                    assert len(found) == 1, found
                    ok(call, '/api/documents/unicode_source/' + quote(source_key, safe=''), method='DELETE')
                    result['exact_unicode_identity_supported'] = True
                    calls_before = len(CALLS)
                ok(call, '/api/documents', {'collection':'nested/source','_key':'a','text':'Synthetic source.'})
                status, value = call('/api/sideviews/generate', {'collection':'nested/source','count':1},
                    headers={'Idempotency-Key':mode + '-ambiguous-identity'})
                assert status == 400 and 'must not contain' in value['error'] and len(CALLS) == calls_before, (status, value)
                result['ambiguous_collection_rejected_before_provider'] = True
                seed(call, 'retained')
                QUESTION = 'Original retrieval surface'
                generated(call, 'retained', mode + '-original')
                before = side_rows(call, 'retained')
                calls_before = len(CALLS)
                generated(call, 'retained', mode + '-skip', count=0)
                assert len(CALLS) == calls_before and side_rows(call, 'retained') == before
                ok(call, '/api/documents/retained/a', {'text': 'Updated source.'}, method='PATCH')
                assert side_rows(call, 'retained') == before, 'ordinary updates should keep the accepted manual refresh policy'
                FAIL_COMPLETION = True
                failed_id = submit(call, 'retained', mode + '-regenerate-failure', regenerate=True)
                failed = terminal(call, failed_id)
                assert failed['status'] == 'failed' and side_rows(call, 'retained') == before, failed
                QUESTION = 'Regenerated retrieval surface'
                resumed = retry(call, failed_id, mode + '-regenerate-retry')
                assert resumed['status'] == 'succeeded', resumed
                fresh = side_rows(call, 'retained')
                assert len(fresh) == 1 and fresh[0]['question'] == QUESTION, fresh
                stored = ok(call, '/api/admin/export')['collections']['side_views']['documents'][fresh[0]['_key']]
                assert stored['embedding'] == [1.0, 0.0], stored
                result['regeneration'] = {'skip_without_provider': True, 'update_keeps_old_rows': True,
                    'provider_failure_preserves_old_rows': True, 'retry_replaces_rows': True}
                before = side_rows(call, 'retained')
                status, value = call('/api/batch', {'ops': [
                    {'op': 'delete', 'collection': 'retained', 'key': 'a'},
                    {'op': 'update', 'collection': 'retained', 'key': 'missing', 'merge': {'v': 1}}
                ]})
                assert status == 404 and side_rows(call, 'retained') == before, (status, value)
                assert call('/api/documents/retained/a')[0] == 200
                result['failed_batch_rollback'] = True
                status, value = call('/api/batch', {'ops': [
                    {'op': 'delete', 'collection': 'retained', 'key': 'a'},
                    {'op': 'insert', 'collection': 'retained', 'doc': {'_key': 'a', 'text': 'Replacement'}}
                ]})
                assert status == 200 and value['count'] == 2 and value['results'][0] is None, (status, value)
                assert side_rows(call, 'retained') == []
                generated(call, 'retained', mode + '-final')
                result['delete_reinsert_batch'] = True

        with server(mode) as call:
            for method in METHODS:
                assert len(side_rows(call, 'basic_' + method)) == int(args.expect_vulnerable and method != 'direct')
            if not args.expect_vulnerable:
                all_rows = ok(call, '/api/documents?collection=side_views')['results']
                assert len(all_rows) == 1 and all_rows[0]['document_id'] == 'retained/a', all_rows
                stored = ok(call, '/api/admin/export')['collections']['side_views']['documents'][all_rows[0]['_key']]
                assert stored['embedding'] == [1.0, 0.0], stored
                result['restart'] = {'surviving_rows': 1, 'orphan_rows': 0, 'embedding_preserved': True}
            else:
                result['restart'] = {'bypassed_cascades_persisted': True}
        evidence['modes'][mode] = result
        print(mode + ': PASS', flush=True)
finally:
    if ARMED:
        ARMED['release'].set()
    mock.shutdown()
    mock.server_close()

evidence['completion_calls'] = sum(c['stage'] == 'completion' for c in CALLS)
evidence['embedding_calls'] = sum(c['stage'] == 'embedding' for c in CALLS)
evidence['logs_directory'] = str(ROOT)
name = 'side-view-lifecycle-' + ('baseline-' if args.expect_vulnerable else '') + 'http-2026-09-09.json'
out = args.output or Path(__file__).with_name(name)
out.write_text(json.dumps(evidence, indent=2) + chr(10))
print(out)
