"""Exercise the packaged backup client over HTTP, including failure preservation."""
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import importlib.util
import json
import os
from pathlib import Path
import tempfile
import threading
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('helm_backup', ROOT / 'deploy/helm/cognigraph/files/backup.py')
client = importlib.util.module_from_spec(spec)
spec.loader.exec_module(client)


class Backup(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.directory = Path(self.temp.name)
        self.password = 'synthetic-quote"-slash\\-newline\n-π'
        self.body = b'{"collections": {"qa": {"type": "document", "documents": {"one": {"text": "synthetic"}}}}}'
        self.code = 200
        owner = self

        class Handler(BaseHTTPRequestHandler):
            def log_message(self, *args):
                pass

            def do_POST(self):
                body = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
                valid = self.path == '/api/auth/login' and body == {'username': 'admin', 'password': owner.password}
                self.send_response(200 if valid else 401)
                self.end_headers()
                self.wfile.write(b'{"token": "synthetic-token"}')

            def do_GET(self):
                valid = self.path == '/api/admin/export' and self.headers.get('Authorization') == 'Bearer synthetic-token'
                self.send_response(owner.code if valid else 401)
                self.end_headers()
                self.wfile.write(owner.body)

        self.server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()
        self.url = f'http://127.0.0.1:{self.server.server_port}'
        self.previous = self.directory / 'cognigraph-old.json'
        self.previous.write_text('{"collections": {}}')
        os.utime(self.previous, (1, 1))

    def tearDown(self):
        self.server.shutdown()
        self.server.server_close()
        self.thread.join()
        self.temp.cleanup()

    def test_password_roundtrip_and_retention_after_success(self):
        unrelated = self.directory / 'unrelated.json'
        unrelated.write_text('{}')
        os.utime(unrelated, (1, 1))
        output = client.backup(self.url, self.password, self.directory, 14)
        self.assertEqual(json.loads(output.read_text()), json.loads(self.body))
        self.assertFalse(self.previous.exists())
        self.assertTrue(unrelated.exists())
        self.assertEqual(list(self.directory.glob('*.partial')), [])

    def test_invalid_exports_preserve_previous_backups(self):
        for body, code in ((b'{"collections": [', 200), (b'{"error": "failed"}', 200),
                           (b'', 200), (b'failure', 500)):
            with self.subTest(body=body, code=code):
                self.body, self.code = body, code
                with self.assertRaises(Exception):
                    client.backup(self.url, self.password, self.directory, 14)
                self.assertEqual(list(self.directory.iterdir()), [self.previous])

    def test_invalid_retention_does_not_delete(self):
        with self.assertRaises(ValueError):
            client.backup(self.url, self.password, self.directory, 0)
        self.assertTrue(self.previous.exists())
