#!/usr/bin/env python3
"""Exercise Community/Enterprise release binaries using disposable Native stores.

Build each edition into a separate directory first, then pass --community DIR
and --enterprise DIR. No external services or model calls: embeddings use a
local deterministic HTTP fixture. Existing environment files are not loaded.
"""
import argparse
from contextlib import contextmanager
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import signal
import socket
import subprocess
import tempfile
import threading
import time
from urllib.error import HTTPError
from urllib.request import Request, urlopen

PASSWORD = 'synthetic-cg45-password'
SECRET = 'synthetic-cg45-jwt-secret'
ENV = {k: v for k, v in os.environ.items() if not k.startswith(
    ('COGNIGRAPH_', 'OPENAI_', 'GEMINI_', 'DEEPSEEK_', 'ZHIPU_', 'OLLAMA_', 'ARANGO_'))}


def port():
    with socket.socket() as sock:
        sock.bind(('127.0.0.1', 0))
        return sock.getsockname()[1]


def http(base, path, body=None, token=None, method=None, status=200):
    headers = {'Content-Type': 'application/json'}
    if token:
        headers['Authorization'] = 'Bearer ' + token
    request = Request(base + path, data=None if body is None else json.dumps(body).encode(),
                      headers=headers, method=method)
    try:
        response = urlopen(request, timeout=10)
    except HTTPError as error:
        response = error
    with response:
        result = response.read()
        assert response.status == status, (path, response.status, result.decode())
        return json.loads(result) if result else None


class Embeddings(BaseHTTPRequestHandler):
    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
        inputs = body['input']
        if isinstance(inputs, str):
            inputs = [inputs]
        data = {'data': [{'embedding': [1.0, 0.0], 'index': i} for i in range(len(inputs))],
                'usage': {'prompt_tokens': 1, 'total_tokens': 1}}
        self.send_response(200)
        self.send_header('Content-Type', 'application/json')
        self.end_headers()
        self.wfile.write(json.dumps(data).encode())

    def log_message(self, *_):
        pass


@contextmanager
def server(binary, directory, config):
    listen = port()
    env = {**ENV, 'COGNIGRAPH_PORT': str(listen), 'COGNIGRAPH_HOST': '127.0.0.1',
           'COGNIGRAPH_AUTH_ENABLED': 'true', 'COGNIGRAPH_ADMIN_PASSWORD': PASSWORD,
           'COGNIGRAPH_HOST_ADMIN_PASSWORD': PASSWORD, 'COGNIGRAPH_JWT_SECRET': SECRET,
           'COGNIGRAPH_CGQL_MUTATIONS_ENABLED': 'true', **config}
    with tempfile.TemporaryFile(mode='w+') as log:
        process = subprocess.Popen([binary], cwd=directory, env=env, stdout=log, stderr=log)
        base = f'http://127.0.0.1:{listen}'
        try:
            for _ in range(100):
                if process.poll() is not None:
                    log.seek(0)
                    raise RuntimeError(log.read())
                try:
                    http(base, '/health/database')
                    break
                except OSError:
                    time.sleep(0.1)
            else:
                raise RuntimeError('server readiness timed out')
            yield base
        finally:
            if process.poll() is None:
                process.send_signal(signal.SIGTERM)
                try:
                    process.wait(timeout=15)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
                    raise RuntimeError('server did not shut down cleanly')
            assert process.returncode == 0, process.returncode


def login(base, username='admin'):
    return http(base, '/api/auth/login', {'username': username, 'password': PASSWORD})['token']


def rejected_start(binary, directory, config):
    process = subprocess.run([binary], cwd=directory, env={**ENV, **config},
                             text=True, capture_output=True, timeout=15)
    assert process.returncode != 0
    assert 'enterprise_feature_required' in process.stderr, process.stderr


def cli(binary, base, command, token=None, success=True):
    args = [binary, '--url', base]
    if token:
        args += ['--token', token]
    result = subprocess.run(args + command, env=ENV, capture_output=True, text=True, timeout=15)
    assert (result.returncode == 0) == success, (command, result.stdout, result.stderr)
    if not success:
        assert 'enterprise_feature_required' in result.stderr, result.stderr
    return result.stdout


def check(community, enterprise):
    fixture = ThreadingHTTPServer(('127.0.0.1', 0), Embeddings)
    thread = threading.Thread(target=fixture.serve_forever, daemon=True)
    thread.start()
    try:
        with tempfile.TemporaryDirectory(prefix='cg45-live-') as directory:
            store = str(Path(directory) / 'ordinary.redb')
            config = {'COGNIGRAPH_NATIVE_PATH': store,
                      'COGNIGRAPH_EMBEDDING_PROVIDER': 'openai', 'OPENAI_API_KEY': 'synthetic',
                      'OPENAI_BASE_URL': f'http://127.0.0.1:{fixture.server_port}/v1'}
            with server(community / 'cognigraph-server', directory, config) as base:
                assert http(base, '/health')['edition'] == 'community'
                token, host = login(base), login(base, 'host-admin')
                for key in ('one', 'two'):
                    http(base, '/api/documents', {'collection': 'notes', '_key': key,
                         'content': 'synthetic cognigraph evidence', 'embedding': [1.0, 0.0]}, token)
                http(base, '/api/documents/notes/one', {'title': 'updated'}, token, 'PATCH')
                assert http(base, '/api/documents/notes/one', token=token)['title'] == 'updated'
                assert http(base, '/api/search/text', {'collection': 'notes', 'query': 'synthetic'}, token)['count'] == 2
                assert http(base, '/api/search/vector', {'collection': 'notes', 'vector': [1, 0]}, token)['count'] == 2
                http(base, '/api/graph/relationships', {'from': 'notes/one', 'to': 'notes/two', 'relation_type': 'SUPPLIES'}, token)
                assert http(base, '/api/graph/traverse', {'start_vertex': 'notes/one'}, token)['count'] >= 1
                facts = http(base, '/api/search/graph-augmented', {'query': 'synthetic', 'collection': 'notes', 'threshold': 0}, token)
                assert facts['graph_facts'][0]['relation'] == 'SUPPLIES', facts
                assert 'warnings' not in facts, facts  # CG-89: Community has no neuron semantics
                assert http(base, '/api/lua/execute', {'script': 'return graph.query("FOR n IN [1,2] RETURN n")'}, token)['result'] == [1, 2]
                http(base, '/api/query', {'query': 'INSERT { _key: "three", title: "CG45" } INTO notes'}, token)
                assert http(base, '/api/documents/notes/three', token=token)['title'] == 'CG45'
                http(base, '/api/documents/notes/three', token=token, method='DELETE')
                for path in ('/api/neurons', '/api/jobs', '/api/governance/status', '/health/jobs'):
                    http(base, path, token=token, status=404)
                for method, path, body in [('GET', '/api/tenants', None), ('POST', '/api/tenants', {'name': 'acme'}),
                                           ('DELETE', '/api/tenants/acme', None)]:
                    assert http(base, path, body, host, method, 403)['code'] == 'enterprise_feature_required'
                assert http(base, '/api/users', {'username': 'foreign', 'password': PASSWORD,
                            'role': 'viewer', 'tenant': 'acme'}, token, status=403)['code'] == 'enterprise_feature_required'
                spec = http(base, '/openapi.yaml')
                assert '/api/neurons' not in spec['paths'] and '/api/tenants' not in spec['paths']
                assert spec['info']['version'] == http(base, '/health')['version']
                before = http(base, '/api/admin/export', token=token)
                bad = {'collections': {'side_views': {'type': 'document', 'documents': {'x': {'_key': 'x'}}}}}
                assert http(base, '/api/admin/import', bad, token, status=403)['code'] == 'enterprise_feature_required'
                assert http(base, '/api/admin/export', token=token) == before
                cli(community / 'cognigraph', base, ['query', 'RETURN 45'], token)
                for command in (['tenant', 'list'], ['governance', 'status'], ['job', 'list']):
                    cli(community / 'cognigraph', 'http://127.0.0.1:1', command, success=False)
            print('PASS: Community CRUD, CGQL, Lua, text/vector/graph search, auth, rejection, snapshot safety and CLI', flush=True)
            # Enterprise opens the same ordinary file without conversion.
            with server(enterprise / 'cognigraph-server', directory, config) as base:
                token = login(base)
                assert http(base, '/health')['edition'] == 'enterprise'
                assert http(base, '/api/documents/notes/one', token=token)['title'] == 'updated'
                enterprise_spec = http(base, '/openapi.yaml')
                assert '/api/neurons' in enterprise_spec['paths']
                http(base, '/health/jobs')
                cli(enterprise / 'cognigraph', base, ['governance', 'status'], token)
                http(base, '/api/admin/import', before, token)
            with server(community / 'cognigraph-server', directory, config) as base:
                token = login(base)
                assert http(base, '/api/documents/notes/one', token=token)['title'] == 'updated'
                missing = set(enterprise_spec['paths']) - set(spec['paths'])
                for path in sorted(missing):
                    if path.startswith('/api/tenants'):
                        continue  # Explicit rejection stubs covered above.
                    method = next(iter(enterprise_spec['paths'][path])).upper()
                    http(base, path, {}, token, method, status=404)
            print(f'PASS: ordinary file/snapshot interoperability and {len(missing)} Enterprise-only spec paths', flush=True)
            for setting in ({'COGNIGRAPH_DATA_DIR': str(Path(directory) / 'forbidden')},
                            {'COGNIGRAPH_GOVERNANCE_ROOT_PUBLIC_KEY': 'synthetic'},
                            {'COGNIGRAPH_ARTIFACT_CAS_ROOT': str(Path(directory) / 'cas')}):
                rejected_start(community / 'cognigraph-server', directory, setting)
            assert not (Path(directory) / 'forbidden').exists()
            # Import generated state using Enterprise, then refuse Community startup.
            with server(enterprise / 'cognigraph-server', directory, config) as base:
                token = login(base)
                http(base, '/api/admin/import', bad, token)
                # CG-89: an accepted rank hint cannot reweight the default
                # document_relations trace; Enterprise says so, and traversing
                # facts (absent here) silences it.
                hint = {'_key': 'boost', 'id': 'boost', 'type': 'relation_rank_hint', 'status': 'accepted',
                        'confidence': 0.9, 'evidence': ['trace review'], 'relation': 'SUPPLIES', 'boost': 25.0}
                http(base, '/api/admin/import', {'collections': {'neurons': {'type': 'document', 'documents': {'boost': hint}}}}, token)
                search = {'query': 'synthetic', 'collection': 'notes', 'threshold': 0}
                warned = http(base, '/api/search/graph-augmented', search, token)
                assert warned['warnings'][0]['code'] == 'inert_rank_hints', warned
                assert warned['warnings'][0]['accepted_rank_hints'] == 1, warned
                assert 'warnings' not in http(base, '/api/search/graph-augmented', {**search, 'edge_collection': 'facts'}, token)
            rejected_start(community / 'cognigraph-server', directory, {'COGNIGRAPH_NATIVE_PATH': store})
            with server(enterprise / 'cognigraph-server', directory, config) as base:
                assert http(base, '/api/admin/export', token=login(base))['collections']['side_views']['documents']['x']['_key'] == 'x'
            # Enterprise tenant data remains isolated and usable.
            multi = {'COGNIGRAPH_DATA_DIR': str(Path(directory) / 'multi')}
            with server(enterprise / 'cognigraph-server', directory, multi) as base:
                host = login(base, 'host-admin')
                http(base, '/api/tenants', {'name': 'acme'}, host)
                http(base, '/api/tenants/acme/admin', {'username': 'acme-admin', 'password': PASSWORD}, host)
                token = login(base, 'acme-admin')
                http(base, '/api/documents', {'collection': 'notes', '_key': 'tenant-only'}, token)
                assert http(base, '/api/documents?collection=notes', token=token)['count'] == 1
                http(base, '/api/documents?collection=notes', token=login(base), status=404)
            print('PASS: Community configuration/store refusal, preserved governed data, Enterprise tenant isolation', flush=True)
    finally:
        fixture.shutdown()
        fixture.server_close()
        thread.join()


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--community', required=True, type=Path)
    parser.add_argument('--enterprise', required=True, type=Path)
    args = parser.parse_args()
    check(args.community.resolve(), args.enterprise.resolve())
