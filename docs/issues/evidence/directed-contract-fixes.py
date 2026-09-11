"""CG-39/40 release HTTP checks, with OpenAI and Gemini loopback providers.

No external calls or existing database access. Original reproduction is preserved.
Run with PYTHONDONTWRITEBYTECODE=1 after building the release server.
"""
import argparse
import json
import os
from pathlib import Path
import socket
import subprocess
import sys
import tempfile
import threading
import time
import unicodedata
import urllib.error
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

REPO = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPO / 'fixtures/semantic-neurons/luna-baseline-2026-09-09'))
from transport import Server as BaseServer, digest, save


class Recorder:
    def __init__(self):
        self.response = {'facts': []}
        self.records, self.errors = [], []
        owner = self

        class Handler(BaseHTTPRequestHandler):
            def do_POST(self):
                try:
                    body = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
                    if self.path == '/api/embed':
                        response = {'embeddings': [[1., 0.] for _ in body['input']]}
                    elif self.path == '/openai/chat/completions':
                        assert self.headers['Authorization'] == 'Bearer synthetic-key'
                        owner.records.append({'provider': 'openai', 'request': body, 'response': owner.response})
                        response = {'choices': [{'message': {'content': json.dumps(owner.response)}}]}
                    else:
                        assert self.path.startswith('/gemini/models/') and self.path.endswith(':generateContent')
                        assert self.headers['x-goog-api-key'] == 'synthetic-key'
                        owner.records.append({'provider': 'gemini', 'request': body, 'response': owner.response})
                        response = {'candidates': [{'content': {'parts': [{'text': json.dumps(owner.response)}]}}]}
                    status = 200
                except Exception as error:
                    owner.errors.append(type(error).__name__ + ': ' + str(error))
                    status, response = 500, {'error': 'Loopback assertion failed'}
                raw = json.dumps(response).encode()
                self.send_response(status)
                self.send_header('Content-Type', 'application/json')
                self.send_header('Content-Length', str(len(raw)))
                self.end_headers()
                self.wfile.write(raw)

            def log_message(self, *unused):
                pass

        self.server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        self.base = f'http://127.0.0.1:{self.server.server_port}'
        threading.Thread(target=self.server.serve_forever, daemon=True).start()

    def close(self):
        self.server.shutdown()
        self.server.server_close()


class Server(BaseServer):
    def __init__(self, binary, provider, recorder):
        self.temp = tempfile.TemporaryDirectory(prefix='cognigraph-directed-fixes-')
        self.directory = Path(self.temp.name)
        with socket.socket() as sock:
            sock.bind(('127.0.0.1', 0))
            port = sock.getsockname()[1]
        self.base, self.token = f'http://127.0.0.1:{port}', None
        env = {k: os.environ[k] for k in ('PATH', 'HOME', 'TMPDIR') if k in os.environ}
        env.update(COGNIGRAPH_HOST='127.0.0.1', COGNIGRAPH_PORT=str(port),
            COGNIGRAPH_BACKEND='native', COGNIGRAPH_NATIVE_PATH=str(self.directory / 'db.redb'),
            COGNIGRAPH_AUTH_ENABLED='true', COGNIGRAPH_ADMIN_PASSWORD='synthetic-baseline-password',
            COGNIGRAPH_JWT_SECRET='synthetic-baseline-jwt-secret',
            COGNIGRAPH_COMPLETION_PROVIDER=provider, OPENAI_API_KEY='synthetic-key',
            GEMINI_API_KEY='synthetic-key', OPENAI_BASE_URL=recorder.base + '/openai',
            GEMINI_BASE_URL=recorder.base + '/gemini', COGNIGRAPH_EMBEDDING_PROVIDER='ollama',
            OLLAMA_BASE_URL=recorder.base, RUST_LOG='error')
        self.log = (self.directory / 'server.log').open('w')
        self.process = subprocess.Popen([str(binary)], cwd=self.directory, env=env,
                                        stdout=self.log, stderr=self.log)
        try:
            for _ in range(300):
                if self.process.poll() is not None:
                    raise RuntimeError('Disposable server failed to start')
                try:
                    if self.call('/health')[0] == 200:
                        break
                except (OSError, urllib.error.URLError):
                    time.sleep(.05)
            else:
                raise RuntimeError('Disposable server startup timed out')
            status, result, _ = self.call('/api/auth/login',
                {'username': 'admin', 'password': 'synthetic-baseline-password'})
            assert status == 200
            self.token = result['token']
        except BaseException:
            self.close()
            raise


def check_schema(record, request):
    wire = record['request']
    if record['provider'] == 'openai':
        assert wire['model'] == 'gpt-5.6-luna' and wire['reasoning_effort'] == 'low'
        assert wire['response_format']['type'] == 'json_schema'
        assert wire['response_format']['json_schema']['strict'] is True
        schema = wire['response_format']['json_schema']['schema']
    else:
        assert wire['generationConfig']['responseMimeType'] == 'application/json'
        schema = wire['generationConfig']['responseJsonSchema']
    item = schema['properties']['facts']['items']
    assert item['properties']['chunk_id'] == {'type': 'string', 'enum': sorted({c['id'] for c in request['chunks']})}
    assert item['properties']['relation'] == {'type': 'string', 'enum': sorted({r['relation'] for r in request['taxonomy']})}
    for obj in [schema, item]:
        assert obj['additionalProperties'] is False
        assert set(obj['required']) == set(obj['properties'])
    return schema


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    binary = REPO / 'target/release/cognigraph-server'
    recorder = Recorder()
    evidence = {'binary_sha256': digest(binary), 'harness_sha256': digest(__file__),
        'storage': 'Disposable Native database per provider; synthetic fixtures only',
        'external_provider_calls': 0, 'cases': []}
    taxonomy = [{'relation': 'OWNS', 'description': 'The source owns the target.', 'require_in_sentence': ['owns']}]
    try:
        for provider in ['openai', 'gemini']:
            server = Server(binary, provider, recorder)
            try:
                def run(name, request, output, count, expected_status=200):
                    recorder.response = output
                    previous = len(recorder.records)
                    status, result, _ = server.call('/api/construct/directed', request)
                    assert not recorder.errors, recorder.errors
                    assert len(recorder.records) == previous + 1
                    schema = check_schema(recorder.records[-1], request)
                    assert status == expected_status, (name, status, result)
                    facts = [f for f in server.rows('facts') if f['space_id'] == request['space_type']]
                    assert len(facts) == count, (name, result, facts)
                    if status == 200:
                        assert result['facts_grounded'] == count
                        assert result['extracted_by'].endswith('@directed-policy-v2')
                    canonical = {c['id']: unicodedata.normalize('NFC', c['text']).encode() for c in request['chunks']}
                    for fact in facts:
                        assert canonical[fact['evidence_chunk_id']][fact['trigger_start']:fact['trigger_end']].decode() == fact['trigger']
                        assert fact['reviewed_by'].endswith('@directed-policy-v2')
                    spaces = server.rows('space_types')
                    space = next(s for s in spaces if s['_key'] == request['space_type'])
                    assert space['drafted_by'] == 'directed-policy-v2'
                    evidence['cases'].append({'name': name, 'provider': provider, 'request': request,
                        'http_status': status, 'result': result, 'facts': facts, 'schema': schema})
                    return facts

                cases = [
                    ('inside-source', 'Joanne owns Acme.', 'Ann', 'Acme', 0),
                    ('inside-target', 'Acme owns Joanne.', 'Acme', 'Ann', 0),
                    ('prefix-source', 'South African owns Acme.', 'South Africa', 'Acme', 0),
                    ('prefix-target', 'Acme owns South African.', 'Acme', 'South Africa', 0),
                    ('complete-control', 'Ann owns Acme.', 'Ann', 'Acme', 1),
                    ('later-mention', 'Joanne owns AcmeCorp; Ann owns Acme.', 'Ann', 'Acme', 1),
                    ('punctuation', '(O’Neill) owns Smith-Jones.', 'O’Neill', 'Smith-Jones', 1),
                    ('unicode-mark', 'Ann\u20dd owns Acme.', 'Ann', 'Acme', 0),
                    ('unicode-connector', 'Acme owns Ann\u203f.', 'Acme', 'Ann', 0),
                    ('unicode-nfc', 'E\u0301lodie owns Zürich.', 'Élodie', 'Zürich', 1),
                    ('whitespace', 'South   Africa owns New\nYork.', 'South Africa', 'New York', 1),
                ]
                for name, text, source, target, count in cases:
                    request = {'space_type': name, 'taxonomy': taxonomy, 'chunks': [{'id': name, 'text': text}]}
                    fact = {'source': source, 'source_type': 'entity', 'target': target, 'target_type': 'entity',
                        'relation': 'OWNS', 'evidence': text, 'chunk_id': name}
                    run(name, request, {'facts': [fact]}, count)
                ids = [' doc::é] "x" ', ' doc::e\u0301] "x" ']
                request = {'space_type': 'opaque', 'taxonomy': taxonomy + [dict(taxonomy[0], relation='拥有')],
                    'chunks': [{'id': i, 'text': 'Ann owns Acme.'} for i in ids]}
                good = {'source': 'Ann', 'source_type': 'entity', 'target': 'Acme', 'target_type': 'entity',
                    'relation': 'OWNS', 'evidence': 'Ann owns Acme.', 'chunk_id': ids[0]}
                proposals = [good, dict(good, chunk_id=ids[1], relation='拥有')]
                proposals += [dict(good, chunk_id=i) for i in ['chunk ' + ids[0], ids[0].strip(), 'missing']]
                proposals += [dict(good, relation=r) for r in ['owns', 'OWNS ', '拥有 ']]
                before = run('exact-ids-and-rejection', request, {'facts': proposals}, 2)
                assert len(evidence['cases'][-1]['result']['skips']) == 6
                for name, output in [('missing-facts', {}), ('null-facts', {'facts': None}),
                    ('malformed-item', {'facts': [good, dict(good, chunk_id=42)]}),
                    ('extra-field', {'facts': [], 'extra': True})]:
                    after = run(name, request, output, 2, 500)
                    assert after == before, 'Malformed output changed stored projection'
                run('explicit-empty-clears', request, {'facts': []}, 0)
            finally:
                server.close()
    finally:
        recorder.close()
    evidence.update(status='PASS', captured_requests=recorder.records)
    save(args.output, evidence)
    print(json.dumps({'status': 'PASS', 'cases': len(evidence['cases']), 'providers': ['openai', 'gemini']}))


if __name__ == '__main__':
    main()
