"""Disposable CG-56 response ordering / connection-failure proxy.

API responses are forwarded unchanged. /tmp/cg56-qa/control.json selects delay
seconds for old-labelled requests or drops graph connections before forwarding.
The trace contains only synthetic input classification and timing, never tokens.
"""
import http.client
import json
import socket
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

ROOT = Path(__file__).parent
CONTROL = Path('/tmp/cg56-qa/control.json')
events = []
lock = threading.Lock()


def record(event, **fields):
    with lock:
        event.update(fields)
        (ROOT / 'requests.json').write_text(json.dumps(events, indent=2) + '\n')


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *_args):
        pass

    def forward(self):
        body = self.rfile.read(int(self.headers.get('Content-Length', '0')))
        event = None
        control = json.loads(CONTROL.read_text()) if CONTROL.exists() else {}
        delay = 0
        if self.command == 'POST' and self.path in (
                '/api/lua/execute', '/api/search/query', '/api/search/vector',
                '/api/search/semantic', '/api/search/hybrid',
                '/api/search/graph-augmented', '/api/graph/traverse'):
            data = json.loads(body)
            validation = data.get('query', '').lstrip().upper().startswith('EXPLAIN')
            old = ('CG56 slow' in str(data) or data.get('vector') == [1, 0]
                   or data.get('start_vertex') == 'qa_docs/alpha')
            delay = control.get('delay', 0) if old and not validation else 0
            event = {'path': self.path, 'input': data, 'validation': validation,
                     'received_at': time.time(), 'delay_seconds': delay}
            with lock:
                events.append(event)
            record(event)
            if self.path == '/api/graph/traverse' and control.get('drop_graph'):
                record(event, dropped_before_forward=True)
                self.connection.shutdown(socket.SHUT_RDWR)
                self.connection.close()
                return
        connection = http.client.HTTPConnection('127.0.0.1', 38485, timeout=30)
        headers = {k: v for k, v in self.headers.items() if k.lower() not in ('host', 'connection')}
        connection.request(self.command, self.path, body, headers)
        response = connection.getresponse()
        payload = response.read()
        if event:
            record(event, status=response.status, backend_completed_at=time.time())
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
                record(event, delivered_at=time.time())
        except (BrokenPipeError, ConnectionResetError):
            if event:
                record(event, client_disconnected_at=time.time())
        finally:
            connection.close()

    do_GET = forward
    do_POST = forward
    do_PATCH = forward
    do_DELETE = forward
    do_OPTIONS = forward


if __name__ == '__main__':
    ThreadingHTTPServer(('127.0.0.1', 38486), Handler).serve_forever()
