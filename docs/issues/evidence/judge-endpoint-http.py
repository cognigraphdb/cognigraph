"""CG-36 release judge routing and qualification controls with loopback providers.

An outbound proxy rejects CONNECT without forwarding it. The saved release's
ignored endpoint is reproduced without contacting OpenAI or sending a model
request outside this process. All credentials and review material are synthetic.
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

REPO = Path(__file__).resolve().parents[3]
CALLS, CONNECTS, STARTUPS = [], [], []
PARTNER_VERDICT = 'accept'
KEYS = {'OPENAI_API_KEY': 'synthetic-cg36-openai', 'GEMINI_API_KEY': 'synthetic-cg36-gemini'}
MODES = ['resident-embedded', 'resident-sidecar', 'paged-sidecar']


class Mock(BaseHTTPRequestHandler):
    def do_CONNECT(self):
        CONNECTS.append(self.path)
        self.send_error(502, 'Synthetic outbound guard; no forwarding')

    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
        if self.path == '/openai/review/chat/completions':
            provider, model = 'openai', body['model']
            schema = body['response_format']['json_schema']['schema']
            assert self.headers['Authorization'] == 'Bearer ' + KEYS['OPENAI_API_KEY']
        else:
            assert self.path.startswith('/gemini/review/models/') and self.path.endswith(':generateContent'), self.path
            provider = 'gemini'
            model = self.path.split('/models/', 1)[1].removesuffix(':generateContent')
            schema = body['generationConfig']['responseJsonSchema']
            assert self.headers['x-goog-api-key'] == KEYS['GEMINI_API_KEY']
        screen = 'tainted' in schema['properties']
        CALLS.append({'provider': provider, 'model': model, 'stage': 'screen' if screen else 'quality', 'path': self.path})
        result = ({'tainted': False, 'confidence': 1.0, 'reasoning': 'synthetic clean material'} if screen else
            {'verdict': PARTNER_VERDICT if model == 'cg36-partner' else 'accept',
             'confidence': .99, 'reasoning': 'Synthetic loopback judgment', 'concerns': []})
        content = json.dumps(result)
        response = ({'choices': [{'message': {'content': content}}]} if provider == 'openai' else
            {'candidates': [{'content': {'parts': [{'text': content}]}}]})
        data = json.dumps(response).encode()
        self.send_response(200)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Content-Length', str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def log_message(self, *unused):
        pass


@contextlib.contextmanager
def server(root, name, mode, binary, mock, extra, expected_error=None):
    directory = root / name
    directory.mkdir()
    with socket.socket() as sock:
        sock.bind(('127.0.0.1', 0))
        port = sock.getsockname()[1]
    base = f'http://127.0.0.1:{mock.server_port}'
    env = {key: os.environ[key] for key in ('PATH', 'HOME', 'TMPDIR') if key in os.environ}
    env.update(KEYS)
    env.update(COGNIGRAPH_HOST='127.0.0.1', COGNIGRAPH_PORT=str(port),
        COGNIGRAPH_NATIVE_PATH=str(directory / 'db.redb'), COGNIGRAPH_BACKEND='native',
        COGNIGRAPH_STORAGE_MODE=mode.split('-')[0], COGNIGRAPH_VECTOR_MODE=mode.split('-')[1],
        COGNIGRAPH_EMBEDDING_PROVIDER='none', COGNIGRAPH_AUTH_ENABLED='true',
        COGNIGRAPH_ADMIN_PASSWORD='synthetic-cg36-password', COGNIGRAPH_JWT_SECRET='synthetic-cg36-jwt-secret',
        OPENAI_BASE_URL=' ' + base + '/openai/review/// ', GEMINI_BASE_URL=base + '/gemini/review/',
        HTTP_PROXY=base, HTTPS_PROXY=base, ALL_PROXY=base, NO_PROXY='127.0.0.1,localhost')
    for key, value in extra.items():
        if value is None:
            env.pop(key, None)
        else:
            env[key] = value
    token = None

    def call(path, body=None):
        headers = {'Content-Type': 'application/json'}
        if token:
            headers['Authorization'] = 'Bearer ' + token
        req = urllib.request.Request(f'http://127.0.0.1:{port}' + path,
            data=None if body is None else json.dumps(body).encode(), headers=headers)
        try:
            with urllib.request.urlopen(req, timeout=20) as response:
                return response.status, json.load(response)
        except urllib.error.HTTPError as error:
            return error.code, json.loads(error.read())

    with (directory / 'server.log').open('w') as log:
        process = subprocess.Popen([str(binary)], cwd=directory, env=env, stdout=log, stderr=log)
        try:
            if expected_error:
                process.wait(timeout=10)
                text = (directory / 'server.log').read_text()
                assert process.returncode != 0 and all(s in text for s in expected_error), text
                assert 'secret-marker' not in text and not any(key in text for key in KEYS.values()), text
                assert not (directory / 'db.redb').exists(), 'invalid configuration opened storage'
                STARTUPS.append({'case': name, 'rejected': True, 'store_created': False, 'error_fields': expected_error})
                yield None
                return
            for _ in range(200):
                if process.poll() is not None:
                    raise RuntimeError(f'startup failed: {directory / "server.log"}')
                try:
                    if call('/health')[0] == 200:
                        break
                except OSError:
                    time.sleep(.1)
            else:
                raise RuntimeError('startup timed out')
            token = ok(call, '/api/auth/login', {'username': 'admin', 'password': 'synthetic-cg36-password'})['token']
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


def case_env(case):
    env = {'COGNIGRAPH_COMPLETION_PROVIDER': case['main'],
        'COGNIGRAPH_COMPLETION_MODEL': 'cg36-main-' + case['main']}
    if case.get('primary'):
        env['COGNIGRAPH_JUDGE_MODEL'] = ' ' + case['primary'] + ' '
    if case.get('partner'):
        env['COGNIGRAPH_JUDGE_PARTNER_MODEL'] = ' ' + case['partner'] + ' '
    if case.get('blank'):
        env.update(COGNIGRAPH_JUDGE_MODEL=' \t', COGNIGRAPH_JUDGE_PARTNER_MODEL=' ', OPENAI_BASE_URL='unused-secret-marker')
    return env


def review_case(root, mode, binary, mock, case, baseline):
    global PARTNER_VERDICT
    PARTNER_VERDICT = case.get('verdict', 'accept')
    name = mode + '-' + case['name']
    start, connect_start = len(CALLS), len(CONNECTS)
    primary = case.get('primary') or 'cg36-main-' + case['main']
    expected_calls = []
    def calls(provider, model):
        expected_calls.extend((provider, model, stage) for stage in ('screen', 'quality'))
    if not (baseline and case.get('primary')):
        calls('openai' if case.get('primary') else case['main'], primary)
    partner_used = case.get('gated') and case.get('partner') and not case.get('mismatch')
    if partner_used and not baseline:
        calls('openai', case['partner'])
    with server(root, name, mode, binary, mock, case_env(case)) as call:
        qualified = ['unqualified-model'] if case.get('unqualified') else [primary]
        policy = {'kinds': ['relation_hint'], 'min_confidence': .9, 'qualified_judges': qualified}
        if case.get('gated'):
            policy['agreement'] = {'judges': [primary, 'mismatched-partner' if case.get('mismatch') else 'cg36-partner'],
                'kinds': ['relation_hint'], 'min_confidence': .9, 'concordance_measured': True}
        rule = {'source': 'Nimbus', 'relation': 'HOSTS', 'target': 'DataCloud', 'when_any': []}
        if case.get('gated'):
            rule['require_in_sentence'] = ['source']
        ok(call, '/api/admin/import', {'collections': {
            'space_types': {'type': 'document', 'documents': {name: {'_key': name, 'id': name,
                'entities': [{'name': 'Nimbus', 'type': 'org'}, {'name': 'DataCloud', 'type': 'platform'}],
                'relation_rules': [rule]}}},
            'review_policies': {'type': 'document', 'documents': {name: {'_key': name,
                'injection_suite_passed': True, 'sampling_rate': 1.0, 'auto_accept': policy}}}}})
        key = name + '-hint'
        ok(call, '/api/neurons', {'space_type': name, 'id': key, 'type': 'relation_hint', 'confidence': .9,
            'evidence': ['Nimbus hosts DataCloud'], 'source': 'Nimbus', 'relation': 'HOSTS',
            'target': 'DataCloud', 'triggers': ['nimbus hosts datacloud']})
        before = ok(call, '/api/documents/neurons/' + key)
        status, result = call('/api/construct/review', {'space_type': name})
        after = ok(call, '/api/documents/neurons/' + key)
        blocked = baseline and bool(case.get('primary') or case.get('partner'))
        if blocked:
            assert status == 500 and 'judge:' in result['error'], (name, status, result)
            assert after == before, (name, 'failed provider request changed proposal')
            assert CONNECTS[connect_start:] and all(host == 'api.openai.com:443' for host in CONNECTS[connect_start:])
        else:
            assert status == 200 and not CONNECTS[connect_start:], (name, status, result, CONNECTS[connect_start:])
            assert result['judge'] == f'judge:{primary}@judge-policy-v2'
            queued = bool(case.get('unqualified') or case.get('mismatch') or case.get('missing_partner') or PARTNER_VERDICT != 'accept')
            assert len(result['queued']) == int(queued) and len(result['auto_accepted']) == int(not queued), result
            assert after['status'] == ('proposed' if queued else 'accepted'), after
            if case.get('gated'):
                assert (result['lane_a_plus'] == 'active') == bool(partner_used), result
                if partner_used:
                    assert after['partner_verdict'] == PARTNER_VERDICT
                    assert after['partner_judged_by'] == 'judge:cg36-partner@judge-policy-v2'
            if not queued:
                expected_by = f'judges:{primary}+cg36-partner@judge-policy-v2' if partner_used else f'judge:{primary}@judge-policy-v2'
                assert after['reviewed_by'] == expected_by, after
            assert result['judge_calls'] == 1 + int(bool(partner_used)), result
        actual_calls = [(x['provider'], x['model'], x['stage']) for x in CALLS[start:]]
        assert actual_calls == expected_calls, (name, actual_calls, expected_calls)
        return {'case': case['name'], 'mode': mode, 'status': status, 'blocked_default_endpoint': blocked,
            'calls': CALLS[start:], 'blocked_connects': CONNECTS[connect_start:],
            'review': result, 'stored_status': after['status'], 'unchanged_on_failure': after == before if blocked else None}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=REPO / 'target/release/cognigraph-server')
    parser.add_argument('--expect-default-endpoint', action='store_true')
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    binary = args.binary.resolve()
    root = Path(tempfile.mkdtemp(prefix='cognigraph-cg36-live-'))
    print('Temporary evidence:', root, flush=True)
    mock = ThreadingHTTPServer(('127.0.0.1', 0), Mock)
    threading.Thread(target=mock.serve_forever, daemon=True).start()
    cases = [{'name': 'fallback-openai', 'main': 'openai'}, {'name': 'fallback-gemini', 'main': 'gemini'},
        {'name': 'dedicated-primary', 'main': 'gemini', 'primary': 'cg36-primary'},
        {'name': 'dedicated-partner', 'main': 'gemini', 'partner': 'cg36-partner', 'gated': True},
        {'name': 'dedicated-pair', 'main': 'gemini', 'primary': 'cg36-primary', 'partner': 'cg36-partner', 'gated': True},
        {'name': 'blank-judges', 'main': 'gemini', 'blank': True}]
    try:
        reviews = [review_case(root, mode, binary, mock, case, args.expect_default_endpoint) for mode in MODES for case in cases]
        if not args.expect_default_endpoint:
            for control in [dict(name='unqualified', primary='cg36-primary', unqualified=True),
                dict(name='mismatched-pair', primary='cg36-primary', partner='cg36-partner', gated=True, mismatch=True),
                dict(name='missing-partner', primary='cg36-primary', gated=True, missing_partner=True),
                dict(name='disagreement', primary='cg36-primary', partner='cg36-partner', gated=True, verdict='needs_human')]:
                reviews.append(review_case(root, 'resident-sidecar', binary, mock, {'main': 'gemini', **control}, False))
        for lane in ('COGNIGRAPH_JUDGE_MODEL', 'COGNIGRAPH_JUDGE_PARTNER_MODEL'):
            for index, (field, value) in enumerate([('OPENAI_API_KEY', None), ('OPENAI_API_KEY', ' '),
                ('OPENAI_BASE_URL', 'relative-secret-marker'), ('OPENAI_BASE_URL', 'file:///secret-marker'),
                ('OPENAI_BASE_URL', 'https://example.test/v1?secret-marker=key'), ('OPENAI_BASE_URL', 'https://example.test/v1#secret-marker')]):
                name = lane.lower() + '-' + str(index)
                extra = {'COGNIGRAPH_COMPLETION_PROVIDER': 'gemini', lane: 'cg36-dedicated', field: value}
                error = None if args.expect_default_endpoint else [lane, field]
                start, connected = len(CALLS), len(CONNECTS)
                with server(root, name, 'resident-sidecar', binary, mock, extra, error):
                    if args.expect_default_endpoint:
                        STARTUPS.append({'case': name, 'rejected': False, 'store_created': (root / name / 'db.redb').exists(), 'error_fields': [lane, field]})
                assert len(CALLS) == start and len(CONNECTS) == connected, 'configuration probe made a provider call'
        report = {'issue': 'CG-36', 'revision': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=REPO, text=True).strip(),
            'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(), 'harness_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
            'expect_default_endpoint': args.expect_default_endpoint, 'reviews': reviews, 'startup_checks': STARTUPS,
            'loopback_provider_calls': len(CALLS), 'blocked_connects': CONNECTS, 'forwarded_connections': 0}
        args.output.write_text(json.dumps(report, indent=2) + '\n')
        print(json.dumps({'reviews': len(reviews), 'startup_checks': len(STARTUPS), 'loopback_provider_calls': len(CALLS), 'blocked_connects': len(CONNECTS)}), flush=True)
    finally:
        mock.shutdown()
        mock.server_close()


if __name__ == '__main__':
    main()
