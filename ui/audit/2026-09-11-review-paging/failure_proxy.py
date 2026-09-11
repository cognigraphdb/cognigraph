"""Local-only CG-54 presentation fixture: forward to the real QA server.

When /tmp/cg54-qa/fail-space-continuation exists, return a synthetic 503 only
for the second space-catalog page. No credentials or request bodies are logged.
This tests partial-catalog presentation and retry, not a real backend outage.
"""
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
import urllib.error
import urllib.request


class Proxy(BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def handle_request(self):
        if (Path('/tmp/cg54-qa/fail-space-continuation').exists()
                and self.path.startswith('/api/documents?collection=space_types&limit=100&offset=100')):
            body = b'{"error":"Synthetic space continuation unavailable"}'
            self.send_response(503)
            self.send_header('Content-Type', 'application/json')
        else:
            length = int(self.headers.get('Content-Length', '0'))
            request = urllib.request.Request('http://127.0.0.1:38481' + self.path,
                        data=self.rfile.read(length) if length else None, method=self.command,
                        headers={key: value for key, value in self.headers.items()
                                 if key.lower() not in {'host', 'connection', 'content-length'}})
            try:
                response = urllib.request.urlopen(request, timeout=30)
            except urllib.error.HTTPError as error:
                response = error
            with response:
                body = response.read()
                self.send_response(response.status)
                self.send_header('Content-Type', response.headers.get('Content-Type', 'application/octet-stream'))
        self.send_header('Content-Length', str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    do_GET = handle_request
    do_POST = handle_request
    do_PATCH = handle_request
    do_DELETE = handle_request


ThreadingHTTPServer(('127.0.0.1', 38482), Proxy).serve_forever()
