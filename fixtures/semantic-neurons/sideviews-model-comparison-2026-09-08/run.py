"""Run the real Rust side-view harness through a loopback usage-recording proxy.

Only the checked public passages are sent to the official provider endpoints.
The Rust binary loads existing credentials via dotenvy; this recorder never
persists request headers. Preflight stops on any unavailable provider/model.
Use --preflight-only for two calls; otherwise follow with 30 measured calls.
Outputs go to a new directory, never over an earlier measurement.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import random
import re
import subprocess
import threading
import time
import urllib.error
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--preflight-only', action='store_true')
parser.add_argument('--out', type=Path, required=True)
args = parser.parse_args()
ROOT = Path(__file__).resolve().parents[3]
BINARY = ROOT / 'target/release/sideviews-benchmark'
DOCS = ROOT / 'crates/cognigraph-construct/fixtures/sideviews/benchmark-docs.json'
MODELS = [('openai', 'gpt-5.6-luna'), ('gemini', 'gemini-3.8-flash')]
OUT = args.out.resolve()
OUT.mkdir(parents=True, exist_ok=False)
RECORDS = []
SECRETS = set()
CURRENT = {}
LIMIT = 2 if args.preflight_only else 32


def digest(data):
    return hashlib.sha256(data).hexdigest()


def clean(text):
    for secret in SECRETS:
        text = text.replace(secret, '[REDACTED]')
    return re.sub(r'\b(?:sk-[A-Za-z0-9_-]{8,}|AIza[A-Za-z0-9_-]{15,})', '[REDACTED]', text)


class Recorder(BaseHTTPRequestHandler):
    def do_POST(self):
        raw = self.rfile.read(int(self.headers.get('Content-Length', '0')))
        body = json.loads(raw)
        if self.path == '/openai/chat/completions':
            provider = 'openai'
            url = 'https://api.openai.com/v1/chat/completions'
            auth = self.headers.get('Authorization', '')
            key = auth.removeprefix('Bearer ')
            headers = {'Authorization': auth}
            model = body['model']
            schema = body['response_format']['json_schema']['schema']
        else:
            assert self.path.startswith('/gemini/models/') and self.path.endswith(':generateContent')
            provider = 'gemini'
            url = 'https://generativelanguage.googleapis.com/v1beta/' + self.path.removeprefix('/gemini/')
            key = self.headers.get('x-goog-api-key', '')
            headers = {'x-goog-api-key': key}
            model = self.path.split('/models/', 1)[1].removesuffix(':generateContent')
            schema = body['generationConfig']['responseJsonSchema']
        if key:
            SECRETS.add(key)
        assert (provider, model) in MODELS
        assert len(RECORDS) < LIMIT, 'call budget exceeded'
        headers['Content-Type'] = 'application/json'
        request = urllib.request.Request(url, data=raw, headers=headers)
        started = time.monotonic()
        try:
            # Never forward credentials across a redirect or to a proxy host.
            class NoRedirect(urllib.request.HTTPRedirectHandler):
                def redirect_request(self, *unused):
                    return None
            opener = urllib.request.build_opener(urllib.request.ProxyHandler({}), NoRedirect)
            with opener.open(request, timeout=110) as response:
                status, payload = response.status, response.read()
        except urllib.error.HTTPError as error:
            status, payload = error.code, error.read()
        except Exception as error:
            status, payload = 502, json.dumps({'error': {'type': type(error).__name__}}).encode()
        elapsed = round((time.monotonic() - started) * 1000)
        try:
            parsed = json.loads(payload)
        except ValueError:
            parsed = {'error': {'type': 'non_json_response'}}
        row = {**CURRENT, 'provider': provider, 'requested_model': model,
            'response_model': parsed.get('model', parsed.get('modelVersion')),
            'http_status': status, 'upstream_latency_ms': elapsed,
            'usage': parsed.get('usage', parsed.get('usageMetadata')),
            'request_sha256': digest(raw),
            'schema_sha256': digest(json.dumps(schema, sort_keys=True).encode()),
            'reasoning_effort': body.get('reasoning_effort'),
            'generation_config': body.get('generationConfig')}
        if status >= 400:
            row['error'] = parsed.get('error', {'type': 'unknown_error'})
        else:
            try:
                content = (parsed['choices'][0]['message']['content'] if provider == 'openai'
                    else parsed['candidates'][0]['content']['parts'][0]['text'])
                row['output'] = json.loads(content)
            except (KeyError, IndexError, ValueError, TypeError):
                row['parse_error'] = True
        RECORDS.append(json.loads(clean(json.dumps(row))))
        (OUT / 'requests.json').write_text(json.dumps(RECORDS, indent=2) + '\n')
        self.send_response(status)
        self.send_header('Content-Type', 'application/json')
        self.end_headers()
        self.wfile.write(payload)

    def log_message(self, *unused):
        pass


server = ThreadingHTTPServer(('127.0.0.1', 0), Recorder)
threading.Thread(target=server.serve_forever, daemon=True).start()


def run(phase, repeat, provider, model, docs):
    global CURRENT
    tag = f'{phase}-{repeat}-{provider}'
    path = OUT / (tag + '-docs.json')
    path.write_text(json.dumps(docs, indent=2) + '\n')
    env = os.environ.copy()
    env['OPENAI_BASE_URL'] = f'http://127.0.0.1:{server.server_port}/openai'
    env['GEMINI_BASE_URL'] = f'http://127.0.0.1:{server.server_port}/gemini'
    CURRENT = {'phase': phase, 'repeat': repeat, 'passage_order': [doc['id'] for doc in docs]}
    command = [str(BINARY), '--docs', str(path), '--model', f'{provider}:{model}',
        '--n', '12', '--out', str(OUT / (tag + '-pairs.json'))]
    start = len(RECORDS)
    result = subprocess.run(command, cwd=ROOT, env=env, capture_output=True, text=True, timeout=650)
    (OUT / (tag + '.log')).write_text(clean(result.stdout + result.stderr))
    rows = RECORDS[start:]
    for row, doc in zip(rows, docs):
        row['passage'] = doc['id']
    (OUT / 'requests.json').write_text(json.dumps(RECORDS, indent=2) + '\n')
    success = result.returncode == 0 and len(rows) == len(docs) and all(
        row['http_status'] == 200 and not row.get('parse_error') and isinstance(row.get('output', {}).get('pairs'), list)
        for row in rows)
    print(json.dumps({'run': tag, 'success': success, 'calls': len(rows),
        'statuses': [row['http_status'] for row in rows]}), flush=True)
    return success


manifest = {'started_utc': time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime()),
    'models': MODELS, 'binary_sha256': digest(BINARY.read_bytes()),
    'fixture_sha256': digest(DOCS.read_bytes()), 'target_pairs': 12,
    'repetitions': 0 if args.preflight_only else 3, 'maximum_calls': LIMIT,
    'source': 'checked public five-passage fixture', 'transport': 'official upstream APIs via loopback recording proxy',
    'result': 'running'}
(OUT / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
try:
    docs = json.loads(DOCS.read_text())
    preflight = [run('preflight', 0, provider, model, docs[:1]) for provider, model in MODELS]
    if not all(preflight):
        manifest['result'] = 'preflight_failed'
    elif args.preflight_only:
        manifest['result'] = 'preflight_passed'
    else:
        completed = True
        for repeat in range(1, 4):
            ordered = docs.copy()
            random.Random(2900 + repeat).shuffle(ordered)
            for provider, model in (MODELS if repeat % 2 else MODELS[::-1]):
                if not run('measurement', repeat, provider, model, ordered):
                    completed = False
                    break
            if not completed:
                break
        manifest['result'] = 'complete' if completed else 'measurement_failed'
finally:
    manifest['completed_utc'] = time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime())
    manifest['actual_calls'] = len(RECORDS)
    (OUT / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
    server.shutdown()
    server.server_close()
    print(json.dumps({'result': manifest['result'], 'calls': len(RECORDS), 'output': str(OUT)}), flush=True)
