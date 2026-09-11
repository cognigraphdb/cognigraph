"""CG-59 fault fixture forwarding real requests to Native on 38493.

UI proxy 38494. /tmp/cg59-qa/mode selects pass, slow (text delayed 8s),
fail (text connection closed), or list-fail (listing connection closed).
No authorization headers, credentials or response bodies are recorded.
"""
import json
import socket
import threading
import time
import urllib.error
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import urlsplit

ORIGIN = 'http://127.0.0.1:38493'
MODE = Path('/tmp/cg59-qa/mode')
OUTPUT = Path(__file__).with_name('requests.json')
TRACE = []
LOCK = threading.Lock()


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *_args):
        pass

    def forward(self):
        started = time.monotonic()
        parsed = urlsplit(self.path)
        is_text = parsed.path == '/api/search/text'
        is_list = parsed.path == '/api/documents'
        configured = MODE.read_text().strip() if MODE.exists() else 'pass'
        mode = configured if is_text or (is_list and configured == 'list-fail') else 'pass'
        body = self.rfile.read(int(self.headers.get('Content-Length', 0))) or None
        status = 'connection_closed'
        try:
            if mode in {'fail', 'list-fail'}:
                self.connection.shutdown(socket.SHUT_RDWR)
                self.connection.close()
                return
            if mode == 'slow':
                time.sleep(8)
            headers = {k: v for k, v in self.headers.items()
                       if k.lower() not in {'host', 'connection', 'content-length', 'accept-encoding'}}
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
                              'status': status, 'search': json.loads(body) if is_text and body else None, 'elapsed_ms': round((time.monotonic() - started) * 1000)})
                OUTPUT.write_text(json.dumps(TRACE, indent=2) + '\n')

    do_GET = do_POST = do_PATCH = do_DELETE = forward


print('CG-59 proxy ready on 127.0.0.1:38494', flush=True)
ThreadingHTTPServer(('127.0.0.1', 38494), Handler).serve_forever()
