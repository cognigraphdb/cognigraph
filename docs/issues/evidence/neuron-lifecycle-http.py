"""CG-21 release HTTP races using disposable stores and a gated loopback judge.

No external model calls. The saved pre-fix binary deterministically reproduces
stale human decisions and accepted hint/blocker conflicts during review.
"""
import argparse
import concurrent.futures
import contextlib
import hashlib
import json
import os
from pathlib import Path
import signal
import socket
import subprocess
import tempfile
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import urllib.error
import urllib.request

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--binary', type=Path)
parser.add_argument('--expect-vulnerable', action='store_true')
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
REPO = Path(__file__).resolve().parents[3]
BINARY = (args.binary or REPO / 'target/release/cognigraph-server').resolve()
ROOT = Path(tempfile.mkdtemp(prefix='cognigraph-cg21-live-'))
MODES = ['resident-embedded', 'resident-sidecar', 'paged-sidecar']


class Gate:
    def __init__(self, verdict='accept', count=1):
        self.verdict, self.count, self.entered = verdict, count, 0
        self.condition, self.release = threading.Condition(), threading.Event()

    def arrive(self):
        with self.condition:
            self.entered += 1
            self.condition.notify_all()
        assert self.release.wait(15), 'test failed to release judge'

    def wait(self):
        with self.condition:
            assert self.condition.wait_for(lambda: self.entered >= self.count, 15), 'judge did not enter'


gate = None
provider_calls = []


class Judge(BaseHTTPRequestHandler):
    def do_POST(self):
        request = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
        screen = 'tainted' in request['response_format']['json_schema']['schema']['properties']
        provider_calls.append({'model': request['model'], 'stage': 'screen' if screen else 'quality'})
        if screen:
            result = {'tainted': False, 'confidence': 1.0, 'reasoning': 'synthetic clean material'}
        else:
            selected = gate
            assert selected is not None
            selected.arrive()
            result = {'verdict': selected.verdict, 'confidence': 0.99,
                      'reasoning': 'Synthetic loopback judgment', 'concerns': []}
        data = json.dumps({'choices': [{'message': {'content': json.dumps(result)}}]}).encode()
        self.send_response(200)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Content-Length', str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def log_message(self, *unused):
        pass


mock = ThreadingHTTPServer(('127.0.0.1', 0), Judge)
threading.Thread(target=mock.serve_forever, daemon=True).start()


@contextlib.contextmanager
def server(mode):
    directory = ROOT / mode
    directory.mkdir(exist_ok=True)
    with socket.socket() as sock:
        sock.bind(('127.0.0.1', 0))
        port = sock.getsockname()[1]
    env = {k: os.environ[k] for k in ('PATH', 'HOME', 'TMPDIR') if k in os.environ}
    env.update(COGNIGRAPH_HOST='127.0.0.1', COGNIGRAPH_PORT=str(port),
        COGNIGRAPH_BACKEND='native', COGNIGRAPH_NATIVE_PATH=str(directory / 'db.redb'),
        COGNIGRAPH_STORAGE_MODE=mode.split('-')[0], COGNIGRAPH_VECTOR_MODE=mode.split('-')[1],
        COGNIGRAPH_EMBEDDING_PROVIDER='none', COGNIGRAPH_AUTH_ENABLED='true',
        COGNIGRAPH_ADMIN_PASSWORD='synthetic-regression-password',
        COGNIGRAPH_JWT_SECRET='synthetic-loopback-secret',
        COGNIGRAPH_COMPLETION_PROVIDER='openai', OPENAI_API_KEY='synthetic-cg21-key',
        OPENAI_BASE_URL=f'http://127.0.0.1:{mock.server_port}/openai',
        COGNIGRAPH_COMPLETION_MODEL='cg21-loopback-judge', COGNIGRAPH_REQUEST_TIMEOUT_SECS='30')
    token = None

    def call(path, body=None, method=None):
        headers = {'Content-Type': 'application/json'}
        if token:
            headers['Authorization'] = 'Bearer ' + token
        request = urllib.request.Request(f'http://127.0.0.1:{port}' + path,
            data=None if body is None else json.dumps(body).encode(), headers=headers, method=method)
        try:
            with urllib.request.urlopen(request, timeout=20) as response:
                return response.status, json.load(response)
        except urllib.error.HTTPError as error:
            return error.code, json.loads(error.read())

    with (directory / 'server.log').open('a') as log:
        process = subprocess.Popen([str(BINARY)], cwd=directory, env=env, stdout=log, stderr=log)
        try:
            for _ in range(200):
                if process.poll() is not None:
                    raise RuntimeError(f'startup failed: {directory / "server.log"}')
                try:
                    if call('/health')[0] == 200:
                        break
                except (OSError, urllib.error.URLError):
                    time.sleep(.1)
            else:
                raise RuntimeError('startup timed out')
            token = ok(call, '/api/auth/login', {'username': 'admin', 'password': 'synthetic-regression-password'})['token']
            yield call
        finally:
            if process.poll() is None:
                process.send_signal(signal.SIGTERM)
                try:
                    process.wait(timeout=8)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()


def ok(call, path, body=None):
    status, result = call(path, body)
    assert status == 200, (path, status, result)
    return result


def seed(call, space):
    ok(call, '/api/admin/import', {'collections': {
        'space_types': {'type': 'document', 'documents': {space: {
            '_key': space, 'id': space,
            'entities': [{'name': 'Nimbus', 'type': 'org'}, {'name': 'DataCloud', 'type': 'platform'}],
            'relation_rules': [{'source': 'Nimbus', 'relation': 'HOSTS', 'target': 'DataCloud', 'when_any': []}]}}},
        'review_policies': {'type': 'document', 'documents': {space: {
            '_key': space, 'injection_suite_passed': True, 'sampling_rate': 1.0,
            'auto_accept': {'kinds': ['relation_hint'], 'min_confidence': .9,
                            'qualified_judges': ['cg21-loopback-judge']}}}}}})


def create(call, space, suffix, kind='relation_hint'):
    key = space + '-' + suffix
    ok(call, '/api/neurons', {'space_type': space, 'id': key, 'type': kind, 'confidence': .9,
        'evidence': ['Nimbus hosts DataCloud'], 'source': 'Nimbus', 'relation': 'HOSTS',
        'target': 'DataCloud', 'triggers': ['nimbus hosts datacloud']})
    return key


def human(call, key, action):
    return call('/api/neurons/' + key + '/' + action, {'note': 'synthetic human ' + action})


def document(call, key):
    return ok(call, '/api/documents/neurons/' + key)


def review(call, space):
    return ok(call, '/api/construct/review', {'space_type': space})


def run_mode(mode):
    global gate
    results, final_docs = [], {}
    with server(mode) as call, concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
        for verdict in ['accept', 'needs_human']:
            for action in ['reject', 'retire', 'accept']:
                space = 'cg21-' + verdict.replace('_', '-') + '-' + action
                seed(call, space)
                key = create(call, space, 'hint')
                gate = Gate(verdict)
                pending = pool.submit(review, call, space)
                try:
                    gate.wait()
                    status, result = human(call, key, action)
                    assert status == 200, result
                    before = document(call, key)
                finally:
                    gate.release.set()
                reviewed = pending.result(timeout=20)
                after = document(call, key)
                unchanged = after == before
                results.append({'case': space, 'human_status': before['status'], 'final_status': after['status'],
                    'human_document_preserved': unchanged, 'review': reviewed})
                assert unchanged != args.expect_vulnerable, results[-1]
                if not args.expect_vulnerable:
                    assert len(reviewed['skipped']) == 1 and reviewed['reviewed'] == 0
                    assert reviewed['pending_remaining'] == 0
                final_docs[key] = after

        space = 'cg21-blocker-during-review'
        seed(call, space)
        key = create(call, space, 'hint')
        gate = Gate()
        pending = pool.submit(review, call, space)
        try:
            gate.wait()
            blocker = create(call, space, 'blocker', 'relation_blocker')
            assert human(call, blocker, 'accept')[0] == 200
        finally:
            gate.release.set()
        reviewed = pending.result(timeout=20)
        after, blocked = document(call, key), document(call, blocker)
        results.append({'case': space, 'hint_status': after['status'], 'blocker_status': blocked['status'], 'review': reviewed})
        assert after['status'] == ('accepted' if args.expect_vulnerable else 'proposed'), results[-1]
        if not args.expect_vulnerable:
            assert len(reviewed['queued']) == 1 and not reviewed['auto_accepted']
            assert 'accepted hint and blocker' in reviewed['queued'][0]['lane_b_reason']
        final_docs.update({key: after, blocker: blocked})

        for verdict in ['accept', 'needs_human']:
            space = 'cg21-overlap-' + verdict.replace('_', '-')
            seed(call, space)
            key = create(call, space, 'hint')
            gate = Gate(verdict, count=2)
            one = pool.submit(review, call, space)
            two = pool.submit(review, call, space)
            try:
                gate.wait()
            finally:
                gate.release.set()
            reviews = [one.result(timeout=20), two.result(timeout=20)]
            published = sum(r['reviewed'] for r in reviews)
            assert published == (2 if args.expect_vulnerable else 1), reviews
            results.append({'case': space, 'published': published, 'reviews': reviews})
            final_docs[key] = document(call, key)

        for collection in ['neurons', 'space_types', 'review_policies']:
            space = 'cg21-source-' + collection.replace('_', '-')
            seed(call, space)
            key = create(call, space, 'hint')
            before = document(call, key)
            gate = Gate()
            pending = pool.submit(review, call, space)
            try:
                gate.wait()
                source_key = key if collection == 'neurons' else space
                changed = ok(call, '/api/documents/' + collection + '/' + source_key)
                field, value = {'neurons': ('rationale', 'changed proposal'),
                    'space_types': ('description', 'changed ontology'),
                    'review_policies': ('sampling_rate', .5)}[collection]
                changed[field] = value
                ok(call, '/api/admin/import', {'collections': {collection: {
                    'type': 'document', 'documents': {source_key: changed}}}})
                before = document(call, key)
            finally:
                gate.release.set()
            reviewed = pending.result(timeout=20)
            after = document(call, key)
            assert (before == after) != args.expect_vulnerable, (collection, reviewed, before, after)
            if not args.expect_vulnerable:
                assert len(reviewed['skipped']) == 1 and reviewed['pending_remaining'] == 1, reviewed
            results.append({'case': space, 'source_collection': collection,
                'current_proposal_preserved': before == after, 'review': reviewed})
            final_docs[key] = after

        space = 'cg21-clean-control'
        seed(call, space)
        key = create(call, space, 'hint')
        gate = Gate()
        gate.release.set()
        clean = review(call, space)
        assert len(clean['auto_accepted']) == 1 and clean['auto_accepted'][0]['lane'] == 'A', clean
        results.append({'case': space, 'review': clean})
        final_docs[key] = document(call, key)

        # HTTP scheduling is intentionally stress coverage; the Rust backend
        # barrier is the deterministic proof of the two-human interleaving.
        conflicts = 0
        for n in range(20):
            space = f'cg21-human-pair-{n:02}'
            seed(call, space)
            hint = create(call, space, 'hint')
            blocker = create(call, space, 'blocker', 'relation_blocker')
            start = threading.Barrier(2)
            def accept(key):
                start.wait(timeout=5)
                return human(call, key, 'accept')
            one, two = pool.submit(accept, hint), pool.submit(accept, blocker)
            responses = [one.result(timeout=20), two.result(timeout=20)]
            statuses = sorted(r[0] for r in responses)
            if statuses == [200, 200]:
                conflicts += 1
            else:
                assert statuses == [200, 400], responses
            if not args.expect_vulnerable:
                assert statuses == [200, 400], responses
            results.append({'case': space, 'statuses': statuses})
            final_docs.update({hint: document(call, hint), blocker: document(call, blocker)})
    with server(mode) as call:
        for key, expected in final_docs.items():
            assert document(call, key) == expected, (mode, key, 'restart changed neuron')
    return {'mode': mode, 'scenarios': results, 'restart_documents_verified': len(final_docs),
            'simultaneous_human_conflicts': conflicts}


try:
    results = [run_mode(mode) for mode in MODES]
    output = {'binary_sha256': hashlib.sha256(BINARY.read_bytes()).hexdigest(),
        'expect_vulnerable': args.expect_vulnerable, 'root': str(ROOT), 'results': results,
        'provider_calls': provider_calls, 'server_restarts': 3}
    args.output.write_text(json.dumps(output, indent=2) + '\n')
    print(json.dumps({'root': str(ROOT), 'modes': len(results),
        'scenarios': sum(len(r['scenarios']) for r in results),
        'restart_documents': sum(r['restart_documents_verified'] for r in results),
        'simultaneous_human_conflicts': sum(r['simultaneous_human_conflicts'] for r in results),
        'provider_calls': len(provider_calls), 'output': str(args.output)}))
finally:
    if gate:
        gate.release.set()
    mock.shutdown()
    mock.server_close()
