"""CG-55 disposable local QA proxy, forwarding real API responses unchanged.

The response delay keeps execution pending in the browser after the server has
finished. Automatic EXPLAIN validation requests are classified separately.
No credentials, request bodies, or response bodies are recorded.
"""
import hashlib
import http.client
import json
import os
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

TARGET_PORT = 38483
PROXY_PORT = 38484
DELAY_SECONDS = 10
QUERY_DELAY_SECONDS = int(os.environ.get('CG55_QUERY_DELAY_SECONDS', DELAY_SECONDS))
OUTPUT = Path(__file__).with_name('requests.json')
events = json.loads(OUTPUT.read_text())['requests'] if OUTPUT.exists() else []
lock = threading.Lock()


def save():
    OUTPUT.write_text(json.dumps({'delay_seconds': DELAY_SECONDS, 'requests': events}, indent=2) + '\n')


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *_args):
        pass

    def forward(self):
        body = self.rfile.read(int(self.headers.get('Content-Length', '0')))
        kind = None
        if self.command == 'POST' and self.path in ['/api/lua/execute', '/api/search/query']:
            kind = 'lua' if self.path.endswith('/lua/execute') else 'query'
            if kind == 'query' and json.loads(body).get('query', '').lstrip().upper().startswith('EXPLAIN'):
                kind = 'validation'
        event = None
        delay = QUERY_DELAY_SECONDS if kind == 'query' else DELAY_SECONDS if kind == 'lua' else 0
        if kind:
            event = {'kind': kind, 'received_at': time.time(), 'body_sha256': hashlib.sha256(body).hexdigest(), 'delay_seconds': delay}
            with lock:
                events.append(event)
                save()
        headers = {k: v for k, v in self.headers.items() if k.lower() not in ('host', 'connection')}
        connection = http.client.HTTPConnection('127.0.0.1', TARGET_PORT, timeout=30)
        connection.request(self.command, self.path, body, headers)
        response = connection.getresponse()
        payload = response.read()
        if event:
            with lock:
                event.update(status=response.status, backend_completed_at=time.time())
                save()
        if delay:
            time.sleep(delay)
        try:
            self.send_response(response.status)
            for key, value in response.getheaders():
                if key.lower() not in ('connection', 'transfer-encoding', 'content-length'):
                    self.send_header(key, value)
            self.send_header('Content-Length', str(len(payload)))
            self.end_headers()
            self.wfile.write(payload)
            if event:
                event['delivered_at'] = time.time()
        except (BrokenPipeError, ConnectionResetError):
            if event:
                event['client_disconnected_at'] = time.time()
        finally:
            connection.close()
            if event:
                with lock:
                    save()

    do_GET = forward
    do_POST = forward
    do_DELETE = forward
    do_PATCH = forward
    do_OPTIONS = forward


if __name__ == '__main__':
    ThreadingHTTPServer(('127.0.0.1', PROXY_PORT), Handler).serve_forever()
