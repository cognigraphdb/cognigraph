"""Local CG-58 fault injection; forwards to the disposable server on 38489.

UI listens on 38490. /tmp/cg58-qa/catalog-mode selects pass, slow, fail or denied.
Only the space-list request is affected. 'denied' forwards it using the real
synthetic host-admin token to obtain an actual backend 403; the UI session
stays Admin. This is an intentional authorization mismatch fixture, not an
observation of the Admin's normal scopes. 'fail' closes the connection before
forwarding. No credentials or response bodies are written to the trace.
"""
import json
import os
import socket
import threading
import time
import urllib.error
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import parse_qs, urlsplit

ORIGIN = 'http://127.0.0.1:38489'
MODE = Path('/tmp/cg58-qa/catalog-mode')
OUTPUT = Path(__file__).with_name('requests.json')
login = urllib.request.Request(ORIGIN + '/api/auth/login', method='POST',
    headers={'Content-Type': 'application/json'},
    data=json.dumps({'username': 'host-admin', 'password': os.environ['CG58_QA_HOST_PASSWORD']}).encode())
with urllib.request.urlopen(login) as response:
    HOST_TOKEN = json.load(response)['token']
TRACE = []
LOCK = threading.Lock()


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *_args):
        pass

    def forward(self):
        started = time.monotonic()
        parsed = urlsplit(self.path)
        is_catalog = parsed.path == '/api/documents' and parse_qs(parsed.query).get('collection') == ['space_types']
        mode = MODE.read_text().strip() if is_catalog and MODE.exists() else 'pass'
        body = self.rfile.read(int(self.headers.get('Content-Length', 0))) or None
        status = 'connection_closed'
        try:
            if mode == 'fail':
                self.connection.shutdown(socket.SHUT_RDWR)
                self.connection.close()
                return
            if mode == 'slow':
                time.sleep(8)
            headers = {k: v for k, v in self.headers.items()
                       if k.lower() not in {'host', 'connection', 'content-length', 'accept-encoding'}}
            if mode == 'denied':
                headers['Authorization'] = 'Bearer ' + HOST_TOKEN
            req = urllib.request.Request(ORIGIN + self.path, method=self.command, headers=headers, data=body)
            try:
                response = urllib.request.urlopen(req, timeout=20)
            except urllib.error.HTTPError as error:
                response = error
            with response:
                payload = response.read()
                status = response.status
                self.send_response(status)
                for k, v in response.headers.items():
                    if k.lower() not in {'connection', 'transfer-encoding', 'content-length'}:
                        self.send_header(k, v)
                self.send_header('Content-Length', str(len(payload)))
                self.end_headers()
                self.wfile.write(payload)
        except (BrokenPipeError, ConnectionResetError):
            pass
        finally:
            with LOCK:
                TRACE.append({'method': self.command, 'path': self.path, 'mode': mode,
                              'status': status, 'elapsed_ms': round((time.monotonic() - started) * 1000)})
                OUTPUT.write_text(json.dumps(TRACE, indent=2) + '\n')

    do_GET = do_POST = do_PATCH = do_DELETE = forward


print('CG-58 proxy ready on 127.0.0.1:38490', flush=True)
ThreadingHTTPServer(('127.0.0.1', 38490), Handler).serve_forever()
