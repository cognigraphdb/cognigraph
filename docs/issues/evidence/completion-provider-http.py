"""CG-29 release-server and benchmark regression; synthetic loopback providers only.

Build cognigraph-server and sideviews-benchmark in release mode, then run this
script. --binary PATH --expect-vulnerable checks only the original main-provider
misrouting (old side-view helpers ignore local base URLs, so are never called).
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

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--binary', type=Path)
parser.add_argument('--benchmark', type=Path)
parser.add_argument('--expect-vulnerable', action='store_true')
args = parser.parse_args()
REPO = Path(__file__).resolve().parents[3]
BINARY = (args.binary or REPO / 'target/release/cognigraph-server').resolve()
BENCHMARK = (args.benchmark or REPO / 'target/release/sideviews-benchmark').resolve()
ROOT = Path(tempfile.mkdtemp(prefix='cognigraph-cg29-live-'))
CALLS = []
PAIRS = [{'question': 'Who does Alpha supply?', 'answer': 'Alpha supplies Beta.'}]
DEFAULTS = {'openai': 'gpt-5.6-luna', 'gemini': 'gemini-3.8-flash'}
KEYS = {'OPENAI_API_KEY': 'synthetic-openai', 'GEMINI_API_KEY': 'synthetic-gemini'}
DIRECTED = {'space_type': 'cg29-synthetic',
    'taxonomy': [{'relation': 'SUPPLIES', 'description': 'supplies', 'require_in_sentence': ['supplies']}],
    'chunks': [{'id': 'cg29-chunk', 'text': 'Alpha supplies Beta.'}]}


class Mock(BaseHTTPRequestHandler):
    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers.get('Content-Length', 0))))
        if self.path == '/api/embed':
            response = {'embeddings': [[1.0, 0.0] for _ in body['input']]}
        else:
            if self.path == '/openai/chat/completions':
                provider, model = 'openai', body['model']
                schema = body['response_format']['json_schema']['schema']
                assert self.headers['Authorization'] == 'Bearer synthetic-openai'
                if model == 'gpt-5.6-luna':
                    assert body['reasoning_effort'] == 'none', body
                else:
                    assert 'reasoning_effort' not in body, body
            else:
                assert self.path.startswith('/gemini/models/') and self.path.endswith(':generateContent'), self.path
                provider = 'gemini'
                model = self.path.split('/models/', 1)[1].removesuffix(':generateContent')
                schema = body['generationConfig']['responseJsonSchema']
                assert self.headers['x-goog-api-key'] == 'synthetic-gemini'
            lane = 'sideviews' if 'pairs' in schema['properties'] else 'construction'
            assert lane == 'sideviews' or 'facts' in schema['properties'], schema
            CALLS.append({'provider': provider, 'model': model, 'lane': lane})
            content = json.dumps({'pairs': PAIRS} if lane == 'sideviews' else {'facts': []})
            response = ({'choices': [{'message': {'content': content}}]} if provider == 'openai'
                else {'candidates': [{'content': {'parts': [{'text': content}]}}]})
        data = json.dumps(response).encode()
        self.send_response(200)
        self.send_header('Content-Type', 'application/json')
        self.end_headers()
        self.wfile.write(data)

    def log_message(self, *unused):
        pass


mock = ThreadingHTTPServer(('127.0.0.1', 0), Mock)
threading.Thread(target=mock.serve_forever, daemon=True).start()


def environment(extra):
    env = {key: os.environ[key] for key in ('PATH', 'HOME', 'TMPDIR') if key in os.environ}
    base = f'http://127.0.0.1:{mock.server_port}'
    env.update(OPENAI_BASE_URL=base + '/openai/', GEMINI_BASE_URL=base + '/gemini/',
        OLLAMA_BASE_URL=base)
    env.update(extra)
    if args.expect_vulnerable:
        # The old constructor does not normalize trailing slashes.
        env['OPENAI_BASE_URL'] = base + '/openai'
    return env


@contextlib.contextmanager
def server(name, extra, expected_error=None):
    directory = ROOT / name
    directory.mkdir()
    with socket.socket() as sock:
        sock.bind(('127.0.0.1', 0))
        port = sock.getsockname()[1]
    env = environment(extra)
    env.update(COGNIGRAPH_HOST='127.0.0.1', COGNIGRAPH_PORT=str(port),
        COGNIGRAPH_NATIVE_PATH=str(directory / 'db.redb'),
        COGNIGRAPH_EMBEDDING_PROVIDER='ollama', COGNIGRAPH_AUTH_ENABLED='true',
        COGNIGRAPH_ADMIN_PASSWORD='synthetic-password',
        COGNIGRAPH_JWT_SECRET='synthetic-loopback-secret', COGNIGRAPH_QUERY_CACHE_ENABLED='false')
    token = None

    def call(path, body=None, headers=None):
        request_headers = {'Content-Type': 'application/json', **(headers or {})}
        if token:
            request_headers['Authorization'] = 'Bearer ' + token
        request = urllib.request.Request(f'http://127.0.0.1:{port}' + path,
            data=None if body is None else json.dumps(body).encode(), headers=request_headers)
        try:
            with urllib.request.urlopen(request, timeout=20) as response:
                return response.status, json.load(response)
        except urllib.error.HTTPError as error:
            return error.code, json.loads(error.read())

    log_path = directory / 'server.log'
    with log_path.open('w') as log:
        process = subprocess.Popen([str(BINARY)], cwd=directory, env=env, stdout=log, stderr=log)
        try:
            if expected_error:
                assert process.wait(timeout=15) != 0, 'invalid configuration started successfully'
                message = log_path.read_text()
                assert expected_error in message, message
                assert not (directory / 'db.redb').exists(), 'invalid config opened persistent store'
                yield {'rejected_before_store_open': True, 'diagnostic': expected_error}
                return
            for _ in range(200):
                if process.poll() is not None:
                    raise RuntimeError(log_path.read_text())
                try:
                    if call('/health')[0] == 200:
                        break
                except (OSError, urllib.error.URLError):
                    time.sleep(0.1)
            else:
                raise RuntimeError('startup timeout')
            token = ok(call, '/api/auth/login', {'username': 'admin', 'password': 'synthetic-password'})['token']
            yield call
        finally:
            if process.poll() is None:
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


def expect_call(start, provider, model, lane):
    expected = [{'provider': provider, 'model': model, 'lane': lane}]
    assert CALLS[start:] == expected, (CALLS[start:], expected)
    return expected[0]


def exercise(name, env, main, side=None):
    with server(name, env) as call:
        start = len(CALLS)
        status, result = call('/api/construct/directed', DIRECTED)
        if main is None:
            assert status == 500 and 'No completion provider configured' in result['error'], (status, result)
            status, result = call('/api/sideviews/generate', {'collection': 'sources'}, {'Idempotency-Key': name})
            assert status == 400 and 'requires a configured' in result['error'], (status, result)
            assert len(CALLS) == start
            return {'startup': 'healthy', 'construction': 'disabled', 'sideviews': 'disabled', 'provider_calls': 0}
        assert status == 200 and result['facts_grounded'] == 0, (status, result)
        observed = {'construction': expect_call(start, *main, 'construction')}
        if args.expect_vulnerable:
            return observed
        ok(call, '/api/documents', {'collection': 'sources', '_key': 'cg29-source', 'text': 'Alpha supplies Beta.'})
        start = len(CALLS)
        status, result = call('/api/sideviews/generate', {'collection': 'sources', 'count': 1}, {'Idempotency-Key': name})
        assert status == 202, (status, result)
        job_id = result['job']['id']
        for _ in range(200):
            job = ok(call, '/api/jobs/' + job_id)
            if job['status'] == 'succeeded':
                break
            assert job['status'] in ('queued', 'running'), job
            time.sleep(0.1)
        else:
            raise RuntimeError('side-view job timeout')
        assert job['result']['side_views_written'] == 1, job
        observed['sideviews'] = expect_call(start, *(side or main), 'sideviews')
        observed['side_views_written'] = 1
        return observed


def benchmark(name, env, models, expected):
    directory = ROOT / name
    directory.mkdir()
    docs = directory / 'docs.json'
    docs.write_text(json.dumps([{'id': 'cg29-source', 'text': 'Alpha supplies Beta.'}]))
    output = directory / 'pairs.json'
    command = [str(BENCHMARK), '--docs', str(docs), '--n', '1', '--out', str(output)]
    for model in models:
        command.extend(['--model', model])
    start = len(CALLS)
    result = subprocess.run(command, cwd=directory, env=environment(env), text=True, capture_output=True, timeout=30)
    (directory / 'benchmark.log').write_text(result.stdout + result.stderr)
    assert result.returncode == 0, result.stderr
    expected_calls = [{'provider': provider, 'model': model, 'lane': 'sideviews'} for provider, model in expected]
    assert CALLS[start:] == expected_calls, (CALLS[start:], expected_calls)
    dump = json.loads(output.read_text())
    assert len(dump) == len(expected) and all(value == {'cg29-source': PAIRS} for value in dump.values()), dump
    return {'calls': expected_calls, 'generated_pairs_verified': True}


evidence = {'issue': 'CG-29', 'date': '2026-09-08', 'binary_profile': 'release',
    'binary_sha256': hashlib.sha256(BINARY.read_bytes()).hexdigest(), 'provider': 'synthetic loopback HTTP only',
    'expected': 'vulnerable baseline' if args.expect_vulnerable else 'shared provider selection',
    'defaults': DEFAULTS if not args.expect_vulnerable else None,
    'luna_reasoning_effort_verified': 'none' if not args.expect_vulnerable else None,
    'server': {}, 'rejected_startup': {}, 'harness': {}}
try:
    if args.expect_vulnerable:
        evidence['server']['explicit-gemini-both-keys'] = exercise('baseline',
            {**KEYS, 'COGNIGRAPH_COMPLETION_PROVIDER': 'gemini', 'COGNIGRAPH_COMPLETION_MODEL': 'cg29-model'},
            ('openai', 'cg29-model'))
    else:
        cases = [
            ('default-both-keys', KEYS, ('openai', DEFAULTS['openai']), None),
            ('only-openai', {'OPENAI_API_KEY': KEYS['OPENAI_API_KEY']}, ('openai', DEFAULTS['openai']), None),
            ('only-gemini', {'GEMINI_API_KEY': KEYS['GEMINI_API_KEY']}, ('gemini', DEFAULTS['gemini']), None),
            ('explicit-openai', {**KEYS, 'COGNIGRAPH_COMPLETION_PROVIDER': 'openai', 'COGNIGRAPH_COMPLETION_MODEL': 'cg29-openai'}, ('openai', 'cg29-openai'), None),
            ('explicit-gemini', {**KEYS, 'COGNIGRAPH_COMPLETION_PROVIDER': 'gemini', 'COGNIGRAPH_COMPLETION_MODEL': 'cg29-gemini'}, ('gemini', 'cg29-gemini'), None),
            ('blank-openai-key', {**KEYS, 'OPENAI_API_KEY': ' '}, ('gemini', DEFAULTS['gemini']), None),
            ('sideviews-gemini-default', {**KEYS, 'COGNIGRAPH_COMPLETION_MODEL': 'cg29-main', 'COGNIGRAPH_SIDEVIEWS_PROVIDER': 'gemini'}, ('openai', 'cg29-main'), ('gemini', DEFAULTS['gemini'])),
            ('sideviews-openai-default', {**KEYS, 'COGNIGRAPH_COMPLETION_PROVIDER': 'gemini', 'COGNIGRAPH_COMPLETION_MODEL': 'cg29-main', 'COGNIGRAPH_SIDEVIEWS_PROVIDER': 'openai'}, ('gemini', 'cg29-main'), ('openai', DEFAULTS['openai'])),
            ('sideviews-same-provider-default', {**KEYS, 'COGNIGRAPH_COMPLETION_MODEL': 'cg29-main', 'COGNIGRAPH_SIDEVIEWS_PROVIDER': 'openai'}, ('openai', 'cg29-main'), ('openai', DEFAULTS['openai'])),
            ('sideviews-model-only', {**KEYS, 'COGNIGRAPH_COMPLETION_PROVIDER': 'gemini', 'COGNIGRAPH_COMPLETION_MODEL': 'cg29-main', 'COGNIGRAPH_SIDEVIEWS_MODEL': ' cg29-side '}, ('gemini', 'cg29-main'), ('gemini', 'cg29-side')),
            ('sideviews-provider-and-model', {**KEYS, 'COGNIGRAPH_SIDEVIEWS_PROVIDER': 'gemini', 'COGNIGRAPH_SIDEVIEWS_MODEL': 'cg29-side'}, ('openai', DEFAULTS['openai']), ('gemini', 'cg29-side')),
            ('no-keys', {}, None, None),
            ('blank-keys', {'OPENAI_API_KEY': '', 'GEMINI_API_KEY': ' '}, None, None),
        ]
        for name, env, main, side in cases:
            evidence['server'][name] = exercise(name, env, main, side)
        invalid = [
            ('invalid-completion', {**KEYS, 'COGNIGRAPH_COMPLETION_PROVIDER': 'invalid'}, 'COGNIGRAPH_COMPLETION_PROVIDER'),
            ('blank-completion', {**KEYS, 'COGNIGRAPH_COMPLETION_PROVIDER': ''}, 'COGNIGRAPH_COMPLETION_PROVIDER'),
            ('invalid-completion-no-keys', {'COGNIGRAPH_COMPLETION_PROVIDER': 'invalid'}, 'COGNIGRAPH_COMPLETION_PROVIDER'),
            ('invalid-sideviews', {**KEYS, 'COGNIGRAPH_SIDEVIEWS_PROVIDER': 'invalid'}, 'COGNIGRAPH_SIDEVIEWS_PROVIDER'),
            ('blank-sideviews', {**KEYS, 'COGNIGRAPH_SIDEVIEWS_PROVIDER': ''}, 'COGNIGRAPH_SIDEVIEWS_PROVIDER'),
            ('missing-openai-key', {'COGNIGRAPH_COMPLETION_PROVIDER': 'openai', 'GEMINI_API_KEY': KEYS['GEMINI_API_KEY']}, 'OPENAI_API_KEY required'),
            ('missing-gemini-key', {'COGNIGRAPH_COMPLETION_PROVIDER': 'gemini', 'OPENAI_API_KEY': KEYS['OPENAI_API_KEY']}, 'GEMINI_API_KEY required'),
            ('blank-selected-key', {**KEYS, 'COGNIGRAPH_COMPLETION_PROVIDER': 'openai', 'OPENAI_API_KEY': ' '}, 'OPENAI_API_KEY required'),
            ('explicit-no-keys', {'COGNIGRAPH_COMPLETION_PROVIDER': 'gemini'}, 'GEMINI_API_KEY required'),
            ('missing-sideviews-key', {'OPENAI_API_KEY': KEYS['OPENAI_API_KEY'], 'COGNIGRAPH_SIDEVIEWS_PROVIDER': 'gemini'}, 'GEMINI_API_KEY required'),
            ('invalid-openai-base', {**KEYS, 'OPENAI_BASE_URL': 'invalid'}, 'OPENAI_BASE_URL must be'),
            ('invalid-gemini-base', {**KEYS, 'COGNIGRAPH_COMPLETION_PROVIDER': 'gemini', 'GEMINI_BASE_URL': 'file:///synthetic'}, 'GEMINI_BASE_URL must be'),
            ('invalid-sideviews-base', {**KEYS, 'COGNIGRAPH_SIDEVIEWS_PROVIDER': 'gemini', 'GEMINI_BASE_URL': 'http://127.0.0.1/v1?key=synthetic'}, 'GEMINI_BASE_URL must be'),
        ]
        for name, env, error in invalid:
            start = len(CALLS)
            with server(name, env, error) as observed:
                evidence['rejected_startup'][name] = observed
            assert len(CALLS) == start
        evidence['benchmark_sha256'] = hashlib.sha256(BENCHMARK.read_bytes()).hexdigest()
        evidence['harness']['inherited-gemini'] = benchmark('benchmark-inherited',
            {**KEYS, 'COGNIGRAPH_COMPLETION_PROVIDER': 'gemini', 'COGNIGRAPH_COMPLETION_MODEL': 'cg29-main'}, [], [('gemini', 'cg29-main')])
        evidence['harness']['explicit-openai-default'] = benchmark('benchmark-explicit',
            {**KEYS, 'COGNIGRAPH_COMPLETION_PROVIDER': 'gemini', 'COGNIGRAPH_COMPLETION_MODEL': 'cg29-main', 'COGNIGRAPH_SIDEVIEWS_PROVIDER': 'openai'}, [], [('openai', DEFAULTS['openai'])])
        evidence['harness']['named-pairs'] = benchmark('benchmark-named',
            {**KEYS, 'COGNIGRAPH_COMPLETION_MODEL': 'must-not-leak'}, ['openai', 'gemini', 'openai:cg29-benchmark'],
            [('openai', DEFAULTS['openai']), ('gemini', DEFAULTS['gemini']), ('openai', 'cg29-benchmark')])
    evidence['result'] = 'PASS'
    (ROOT / 'results.json').write_text(json.dumps(evidence, indent=2) + '\n')
    print(json.dumps({'result': 'PASS', 'evidence': str(ROOT / 'results.json'),
        'server_cases': len(evidence['server']), 'rejected_startups': len(evidence['rejected_startup']),
        'harness_cases': len(evidence['harness'])}))
finally:
    mock.shutdown()
    mock.server_close()
