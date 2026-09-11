"""Two-call Luna smoke through the release server and untouched production schemas.

Uses fictional text, an isolated Native database, and the already authorized
OpenAI key. The recorder forwards request bytes without changing any fields;
there is no benchmark prompt, format conversion, reasoning override, or retry.
"""
import argparse
import hashlib
import json
from pathlib import Path
import sys
import threading
import time
import urllib.error
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

REPO = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPO / 'fixtures/semantic-neurons/luna-baseline-2026-09-09'))
from transport import NoRedirect, Server, digest, existing_key, save


class Recorder:
    def __init__(self, output):
        self.output, self.calls, self.key = output, [], existing_key()
        owner = self

        class Handler(BaseHTTPRequestHandler):
            def do_POST(self):
                raw = self.rfile.read(int(self.headers['Content-Length']))
                body = json.loads(raw)
                if self.path == '/api/embed':
                    self.respond(200, json.dumps({'embeddings': [[1., 0.] for _ in body['input']]}).encode())
                    return
                if self.path != '/v1/chat/completions':
                    self.respond(404, b'{}')
                    return
                assert len(owner.calls) < 2, 'Two-call smoke allowance exhausted'
                assert body['model'] == 'gpt-5.6-luna'
                assert body['reasoning_effort'] == 'low'
                assert body['response_format']['type'] == 'json_schema'
                assert body['response_format']['json_schema']['strict'] is True
                record = {'request': body, 'request_bytes_sha256': hashlib.sha256(raw).hexdigest(),
                    'request_forwarded_unchanged': True, 'started_utc': time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime()),
                    'provider_url': 'https://api.openai.com/v1/chat/completions'}
                owner.calls.append(record)
                save(owner.output / 'provider-calls.json', owner.calls)
                request = urllib.request.Request(record['provider_url'], data=raw,
                    headers={'Content-Type': 'application/json', 'Authorization': 'Bearer ' + owner.key})
                started = time.monotonic()
                try:
                    response = urllib.request.build_opener(NoRedirect()).open(request, timeout=110)
                except urllib.error.HTTPError as error:
                    response = error
                except (OSError, urllib.error.URLError):
                    response = None
                if response is None:
                    status, result = 502, b'{"error":{"message":"Provider transport failed; no retry"}}'
                else:
                    with response:
                        status, result = response.status, response.read()
                        record['request_id'] = response.headers.get('x-request-id')
                result = result.decode().replace(owner.key, '[REDACTED_CREDENTIAL]').encode()
                record.update(status=status, latency_seconds=time.monotonic()-started,
                              raw_response=result.decode())
                save(owner.output / 'provider-calls.json', owner.calls)
                self.respond(status, result)

            def respond(self, status, raw):
                self.send_response(status)
                self.send_header('Content-Type', 'application/json')
                self.send_header('Content-Length', str(len(raw)))
                self.end_headers()
                self.wfile.write(raw)

            def log_message(self, *unused):
                pass

        self.http = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        threading.Thread(target=self.http.serve_forever, daemon=True).start()
        self.base = f'http://127.0.0.1:{self.http.server_port}'

    def close(self):
        self.http.shutdown()
        self.http.server_close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=REPO / 'target/release/cognigraph-server')
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    evidence = {'date': '2026-09-09', 'binary_profile': 'release',
        'binary_sha256': digest(args.binary), 'harness_sha256': digest(__file__),
        'server_helper_sha256': digest(REPO / 'fixtures/semantic-neurons/luna-baseline-2026-09-09/transport.py'),
        'storage': 'disposable Native database', 'embedding': 'synthetic loopback, two dimensions',
        'main_model_override': None, 'sideview_override': None,
        'maximum_provider_calls': 2, 'result': 'incomplete'}
    recorder = Recorder(args.output)
    server = None
    try:
        # Blank model resolves the actual default; side-view provider/model unset.
        server = Server(args.binary.resolve(), '', recorder)
        chunks = [
            {'id': 'runtime-active', 'text': 'Alpine supplies Bramble.'},
            {'id': 'runtime-passive', 'text': 'Cobalt is supplied by Dahlia.'},
            {'id': 'runtime-negative', 'text': 'Elm and Fern attended a meeting.'},
        ]
        body = {'space_type': 'luna-runtime', 'taxonomy': [{'relation': 'SUPPLIES',
            'description': 'The source supplies goods to the target. Do not infer supply from a meeting.',
            'require_in_sentence': ['supplies', 'supplied']}], 'chunks': chunks}
        status, result, elapsed = server.call('/api/construct/directed', body)
        evidence['construction'] = {'request': body, 'status': status, 'response': result,
            'latency_seconds': elapsed, 'facts': server.rows('facts'),
            'entities': server.rows('entities'), 'chunks': server.rows('chunks')}
        save(args.output / 'result.json', evidence)
        assert status == 200, ('directed HTTP status', status)
        assert result['facts_grounded'] == 2, result
        entities = {row['_key']: row['name'] for row in evidence['construction']['entities']}
        text_by_id = {chunk['id']: chunk['text'].encode() for chunk in chunks}
        observed = set()
        for fact in evidence['construction']['facts']:
            observed.add((fact['evidence_chunk_id'], entities[fact['_from'].split('/', 1)[1]],
                          fact['relation_type'], entities[fact['_to'].split('/', 1)[1]]))
            raw = text_by_id[fact['evidence_chunk_id']]
            assert raw[fact['trigger_start']:fact['trigger_end']].decode() == fact['trigger']
            assert fact['reviewed_by'] == 'directed:gpt-5.6-luna@directed-policy-v1'
            assert fact['construction_schema'] == 'occurrence-v1'
        assert observed == {('runtime-active', 'Alpine', 'SUPPLIES', 'Bramble'),
                            ('runtime-passive', 'Dahlia', 'SUPPLIES', 'Cobalt')}, observed
        evidence['construction']['evidence_and_direction_verified'] = True
        status, result, _ = server.call('/api/documents',
            {'collection': 'sources', '_key': 'runtime-source', 'text': chunks[0]['text']})
        assert status == 200, ('source HTTP status', status)
        request = urllib.request.Request(server.base + '/api/sideviews/generate',
            data=json.dumps({'collection': 'sources', 'count': 1}).encode(),
            headers={'Content-Type': 'application/json', 'Authorization': 'Bearer ' + server.token,
                     'Idempotency-Key': 'luna-runtime-sideview'})
        with urllib.request.urlopen(request, timeout=20) as response:
            result = json.load(response)
            assert response.status == 202
        job_id = result['job']['id']
        for _ in range(650):
            status, job, _ = server.call('/api/jobs/' + job_id)
            assert status == 200
            if job['status'] in ('succeeded', 'failed', 'cancelled'):
                break
            time.sleep(.2)
        evidence['sideviews'] = {'job': job, 'rows': server.rows('side_views')}
        save(args.output / 'result.json', evidence)
        assert job['status'] == 'succeeded', job
        assert job['result']['side_views_written'] == 1, job
        assert len(evidence['sideviews']['rows']) == 1
        assert len(recorder.calls) == 2
        for call in recorder.calls:
            response = json.loads(call['raw_response'])
            assert call['status'] == 200
            assert response['model'] == 'gpt-5.6-luna'
            assert response['choices'][0]['finish_reason'] == 'stop'
        evidence['result'] = 'PASS'
    finally:
        if server:
            server.close()
        recorder.close()
        evidence['provider_calls'] = len(recorder.calls)
        evidence['finished_utc'] = time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime())
        save(args.output / 'result.json', evidence)
    print(json.dumps({'result': evidence['result'], 'provider_calls': len(recorder.calls),
                      'output': str(args.output)}))


if __name__ == '__main__':
    main()
