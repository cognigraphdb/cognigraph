"""Loopback recorder and disposable real-server process; credentials never recorded."""
import hashlib
import json
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import threading
import time
import urllib.error
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

ROOT = Path(__file__).resolve().parent
REPO = ROOT.parents[2]


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def save(path, value):
    Path(path).write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n')


def existing_key():
    key = os.environ.get('OPENAI_API_KEY', '').strip()
    if not key:
        for line in (REPO / '.env').read_text().splitlines():
            if line.strip().startswith('OPENAI_API_KEY='):
                key = line.split('=', 1)[1].strip().strip('\"\'')
                break
    if not key:
        raise RuntimeError('No existing OpenAI key available')
    return key


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *args, **kwargs):
        return None


class Recorder:
    def __init__(self, output, protocol, mode):
        self.output, self.protocol, self.mode = output, protocol, mode
        self.key = existing_key() if mode == 'live' else None
        self.records, self.reserved, self.context = [], 0.0, None
        owner = self

        class Handler(BaseHTTPRequestHandler):
            def do_POST(self):
                body = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
                if self.path == '/api/embed':
                    self.respond(200, json.dumps({'embeddings': [[1., 0.] for _ in body['input']]}).encode())
                    return
                if self.path != '/v1/chat/completions':
                    self.respond(404, b'{}')
                    return
                owner.forward(self, body)

            def respond(self, status, raw):
                self.send_response(status)
                self.send_header('Content-Type', 'application/json')
                self.send_header('Content-Length', str(len(raw)))
                self.end_headers()
                try:
                    self.wfile.write(raw)
                except BrokenPipeError:
                    pass

            def log_message(self, *args):
                pass

        self.server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        threading.Thread(target=self.server.serve_forever, daemon=True).start()
        self.base = f'http://127.0.0.1:{self.server.server_port}'

    def forward(self, handler, incoming):
        context = self.context
        assert context is not None
        arm = next(a for a in self.protocol['arms'] if a['model'] == incoming['model'])
        outgoing = dict(incoming, reasoning_effort=arm['reasoning_effort'],
                        max_completion_tokens=self.protocol['max_completion_tokens'])
        raw_request = json.dumps(outgoing, ensure_ascii=False).encode()
        # Conservative per-request token reservation, without assuming caching.
        # The additional 4096-token margin covers protocol/schema overhead.
        reserve = ((len(raw_request) + 4096) * arm['usd_per_million_input'] +
                   self.protocol['max_completion_tokens'] * arm['usd_per_million_output']) / 1e6
        entry = {'index': len(self.records), 'context': context, 'request_from_server': incoming,
                 'request_to_provider': outgoing, 'reserved_usd': reserve,
                 'started_utc': time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime()),
                 'provider_url': 'https://api.openai.com/v1/chat/completions', 'mode': self.mode}
        self.records.append(entry)
        if self.reserved + reserve > self.protocol['budget_usd']:
            status, raw = 429, b'{"error":{"message":"Local experiment budget exhausted"}}'
            entry['sent_to_provider'] = False
        else:
            self.reserved += reserve
            entry['sent_to_provider'] = self.mode == 'live'
            started = time.monotonic()
            if self.mode == 'mock':
                facts = []
                for case in context['cases']:
                    for fact in case['gold']:
                        facts.append(dict(fact, source_type='entity', target_type='entity',
                                          evidence=case['text'], chunk_id=case['id']))
                raw = json.dumps({'model': arm['model'], 'choices': [{'finish_reason': 'stop',
                    'message': {'content': json.dumps({'facts': facts})}}],
                    'usage': {'prompt_tokens': 0, 'completion_tokens': 0, 'total_tokens': 0}}).encode()
                status = 200
            else:
                request = urllib.request.Request(entry['provider_url'], data=raw_request,
                    headers={'Content-Type': 'application/json', 'Authorization': 'Bearer ' + self.key})
                try:
                    response = urllib.request.build_opener(NoRedirect()).open(request, timeout=110)
                except urllib.error.HTTPError as error:
                    response = error
                except (OSError, urllib.error.URLError):
                    response = None
                if response is None:
                    status, raw = 502, b'{"error":{"message":"Provider transport failure; no retry"}}'
                else:
                    with response:
                        status, raw = response.status, response.read()
                        entry['request_id'] = response.headers.get('x-request-id')
            entry['latency_seconds'] = time.monotonic() - started
        # API errors must not accidentally echo a credential into the artifact.
        text = raw.decode('utf-8')
        if self.key and self.key in text:
            text = text.replace(self.key, '[REDACTED_CREDENTIAL]')
        entry.update(status=status, raw_response=text)
        save(self.output / 'provider-calls.json', self.records)
        handler.respond(status, text.encode())

    def close(self):
        self.server.shutdown()
        self.server.server_close()


class Server:
    def __init__(self, binary, model, recorder):
        self.temp = tempfile.TemporaryDirectory(prefix='cognigraph-luna-baseline-')
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
            COGNIGRAPH_COMPLETION_PROVIDER='openai', COGNIGRAPH_COMPLETION_MODEL=model,
            OPENAI_API_KEY='loopback-only-placeholder', OPENAI_BASE_URL=recorder.base + '/v1',
            COGNIGRAPH_EMBEDDING_PROVIDER='ollama', OLLAMA_BASE_URL=recorder.base,
            COGNIGRAPH_REQUEST_TIMEOUT_SECS='125', RUST_LOG='error')
        self.log = (self.directory / 'server.log').open('w')
        self.process = subprocess.Popen([str(binary)], cwd=self.directory, env=env,
                                        stdout=self.log, stderr=self.log)
        try:
            for _ in range(300):
                if self.process.poll() is not None:
                    raise RuntimeError('Disposable server startup failed')
                try:
                    if self.call('/health')[0] == 200:
                        break
                except (urllib.error.URLError, OSError):
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

    def call(self, path, body=None):
        headers = {'Content-Type': 'application/json'}
        if self.token:
            headers['Authorization'] = 'Bearer ' + self.token
        request = urllib.request.Request(self.base + path, headers=headers,
            data=None if body is None else json.dumps(body, ensure_ascii=False).encode())
        start = time.monotonic()
        try:
            response = urllib.request.urlopen(request, timeout=135)
        except urllib.error.HTTPError as error:
            response = error
        with response:
            raw = response.read().decode()
            try:
                result = json.loads(raw)
            except json.JSONDecodeError:
                result = raw
            return response.status, result, time.monotonic() - start

    def rows(self, collection):
        status, result, _ = self.call('/api/documents?collection=' + collection + '&limit=10000')
        assert status == 200, (collection, status)
        return result['results']

    def close(self):
        self.process.terminate()
        try:
            self.process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            self.process.kill()
            self.process.wait()
        self.log.close()
        self.temp.cleanup()
