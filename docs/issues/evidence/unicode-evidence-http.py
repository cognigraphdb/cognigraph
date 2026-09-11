"""CG-5 release HTTP regression using disposable Native stores.

Build the release server, then run this script. --binary selects a saved
executable; --expect-vulnerable verifies the original pre-normalization hashes and evidence spans.
No provider keys or external services are used. Temporary logs/stores are kept
at the printed evidence root for inspection.
"""
import argparse
import contextlib
import hashlib
import json
import os
from pathlib import Path
import signal
import socket
import subprocess
import tempfile
import time
import threading
import unicodedata
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import urllib.error
import urllib.request

parser = argparse.ArgumentParser()
parser.add_argument('--binary', type=Path)
parser.add_argument('--output', type=Path)
parser.add_argument('--expect-vulnerable', action='store_true')
parser.add_argument('--seed-legacy-binary', type=Path)
args = parser.parse_args()
REPO = Path(__file__).resolve().parents[3]
BINARY = (args.binary or REPO / 'target/release/cognigraph-server').resolve()
ROOT = Path(tempfile.mkdtemp(prefix='cognigraph-cg5-live-'))


@contextlib.contextmanager
def server(label, target="source", binary=None):
    mode, vectors = label.split("-")
    directory = ROOT / label / target
    directory.mkdir(parents=True, exist_ok=True)
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        port = sock.getsockname()[1]
    env = {k: os.environ[k] for k in ("PATH", "HOME", "TMPDIR") if k in os.environ}
    env.update(
        COGNIGRAPH_HOST="127.0.0.1", COGNIGRAPH_PORT=str(port),
        COGNIGRAPH_NATIVE_PATH=str(directory / "db.redb"),
        COGNIGRAPH_STORAGE_MODE=mode, COGNIGRAPH_VECTOR_MODE=vectors,
        COGNIGRAPH_EMBEDDING_PROVIDER="none", COGNIGRAPH_AUTH_ENABLED="true",
        COGNIGRAPH_ADMIN_PASSWORD="synthetic-regression-password",
        COGNIGRAPH_JWT_SECRET="synthetic-loopback-regression-secret",
        COGNIGRAPH_CGQL_MUTATIONS_ENABLED="true",
        COGNIGRAPH_COMPLETION_PROVIDER="openai", OPENAI_API_KEY="synthetic-cg5-key",
        OPENAI_BASE_URL=f"http://127.0.0.1:{mock.server_port}/openai",
        COGNIGRAPH_COMPLETION_MODEL="gpt-5.6-luna",
    )
    token = None

    def call(path, body=None, method=None):
        headers = {"Content-Type": "application/json"}
        if token:
            headers["Authorization"] = "Bearer " + token
        req = urllib.request.Request(
            f"http://127.0.0.1:{port}" + path,
            data=None if body is None else json.dumps(body).encode(),
            headers=headers, method=method,
        )
        try:
            with urllib.request.urlopen(req, timeout=20) as result:
                return result.status, json.load(result)
        except urllib.error.HTTPError as error:
            return error.code, json.loads(error.read())

    with open(directory / "server.log", "a") as log:
        process = subprocess.Popen([str(binary or BINARY)], cwd=directory, env=env, stdout=log, stderr=log)
        try:
            for _ in range(200):
                if process.poll() is not None:
                    raise RuntimeError(f"startup failed; see {directory / 'server.log'}")
                try:
                    if call("/health")[0] == 200:
                        break
                except (OSError, urllib.error.URLError):
                    time.sleep(0.1)
            else:
                raise RuntimeError("server startup timed out")
            status, login = call("/api/auth/login", {
                "username": "admin", "password": "synthetic-regression-password",
            })
            assert status == 200, login
            token = login["token"]
            yield call, directory
        finally:
            process.send_signal(signal.SIGTERM)
            try:
                process.wait(timeout=8)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()


CASES = [
    {'id':'c0', 'text':'Café. Alpha supplies Beta.', 'source':'Alpha'},
    {'id':'c1', 'text':'Café déploie Beta.', 'source':'Café'},
    {'id':'c2', 'text':'👩‍🔬 가. Alpha supplies Beta.', 'source':'Alpha'},
]
TAXONOMY = [{'relation':'SUPPLIES', 'description':'supplies', 'require_in_sentence':['supplies','ploie']}]
CONFIG = {'id':'cg5-rule', 'entities':[{'name':'Alpha'},{'name':'Beta'},{'name':'Café'}],
    'relation_rules':[
        {'source':'Alpha','relation':'SUPPLIES','target':'Beta','when_any':['supplies']},
        {'source':'Café','relation':'SUPPLIES','target':'Beta','when_any':['déploie']},
    ]}
SELECTED = []
CALLS = []


class Mock(BaseHTTPRequestHandler):
    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
        assert self.path == '/openai/chat/completions', self.path
        assert self.headers['Authorization'] == 'Bearer synthetic-cg5-key'
        prompt = body['messages'][-1]['content']
        CALLS.append({'model':body['model'], 'prompt_is_nfc':unicodedata.is_normalized('NFC', prompt),
            'prompt_sha256':hashlib.sha256(prompt.encode()).hexdigest()})
        facts = [{'source':unicodedata.normalize('NFD', c['source']), 'source_type':'party',
            'target':'Beta','target_type':'party','relation':'SUPPLIES',
            'evidence':unicodedata.normalize('NFD', c['text']), 'chunk_id':c['id']}
            for c in SELECTED if c['text'] != 'No relation remains.']
        response = {'choices':[{'message':{'content':json.dumps({'facts':facts})}}]}
        data = json.dumps(response).encode()
        self.send_response(200)
        self.send_header('Content-Type','application/json')
        self.end_headers()
        self.wfile.write(data)

    def log_message(self, *unused):
        pass


mock = ThreadingHTTPServer(('127.0.0.1', 0), Mock)
threading.Thread(target=mock.serve_forever, daemon=True).start()


def ok(call, path, body=None):
    status, result = call(path, body)
    assert status == 200, (path, status, result)
    return result


def seed_config(call):
    ok(call, '/api/admin/import', {'collections':{'space_types':{'type':'document',
        'documents':{'cg5-rule':{'_key':'cg5-rule', **CONFIG}}}}})


def ingest(call, kind, chunks):
    global SELECTED
    SELECTED = chunks
    request = {'space_type':'cg5-'+kind, 'chunks':[{'id':c['id'], 'text':c['text']} for c in chunks]}
    if kind == 'directed':
        request['taxonomy'] = TAXONOMY
    return ok(call, '/api/construct/' + ('directed' if kind=='directed' else 'ingest'), request)


def projection(call, kind):
    result = {}
    for collection in ['chunks','facts','mentions']:
        rows = ok(call, '/api/search/query', {'query':'FOR d IN '+collection+' FILTER d.space_id == @space RETURN d',
            'bind_vars':{'space':'cg5-'+kind}})['results']
        result[collection] = sorted([{k:v for k,v in row.items() if k not in ['created_at','updated_at']}
            for row in rows], key=lambda row:row['_key'])
    return result


def check_evidence(rows, vulnerable=False):
    checks = []
    ids = {c['chunk_id'] for c in rows['chunks']}
    assert all(f['evidence_chunk_id'] in ids for f in rows['facts'])
    for chunk in rows['chunks']:
        text = chunk['text']
        computed = 'sha256:' + hashlib.sha256(text.encode()).hexdigest()
        checks.append({'chunk_id':chunk['chunk_id'], 'stored_text':text, 'utf8_bytes':len(text.encode()),
            'stored_hash':chunk['content_hash'], 'computed_hash':computed,
            'hash_matches':chunk['content_hash']==computed})
        assert unicodedata.is_normalized('NFC', text)
        assert checks[-1]['hash_matches'] != vulnerable, checks[-1]
        facts = [f for f in rows['facts'] if f['evidence_chunk_id']==chunk['chunk_id']]
        assert len(facts) == (0 if text == 'No relation remains.' else 1), (chunk, facts)
        for fact in facts:
            start,end = fact['trigger_start'],fact['trigger_end']
            try:
                evidence_slice = text.encode()[start:end].decode()
            except UnicodeDecodeError:
                evidence_slice = None
            valid = end <= len(text.encode()) and start < end and evidence_slice is not None and evidence_slice.lower()==fact['trigger'].lower()
            checks[-1].setdefault('spans',[]).append({'start':start,'end':end,'trigger':fact['trigger'],
                'slice':evidence_slice, 'valid':valid, 'fact_key':fact['_key']})
            assert valid != vulnerable, checks[-1]
    return checks


evidence = {'ticket':'CG-5','profile':'release','binary_sha256':hashlib.sha256(BINARY.read_bytes()).hexdigest(),
    'legacy_binary_sha256':hashlib.sha256(args.seed_legacy_binary.read_bytes()).hexdigest() if args.seed_legacy_binary else None,
    'expect_vulnerable':args.expect_vulnerable, 'provider':'synthetic loopback OpenAI-compatible', 'modes':{}}
for label in ['resident-embedded','resident-sidecar','paged-sidecar']:
    record = {'ingestions':[], 'restarts':[], 'legacy':{}}
    if args.seed_legacy_binary:
        with server(label, binary=args.seed_legacy_binary) as (call, _):
            seed_config(call)
            for kind in ['rule','directed']:
                assert ingest(call,kind,CASES[:1])['facts_grounded']==1
                record['legacy'][kind]=check_evidence(projection(call,kind),True)
    with server(label) as (call, _):
        seed_config(call)
        selected = CASES[:1] if args.expect_vulnerable else CASES
        for kind in ['rule','directed']:
            forms = ['NFD'] if args.expect_vulnerable else ['NFD','NFC','NFD']
            baseline = None
            for form in forms:
                chunks = [{**c,'text':unicodedata.normalize(form,c['text'])} for c in selected]
                result = ingest(call,kind,chunks)
                assert result['facts_grounded']==len(chunks), result
                rows = projection(call,kind)
                assert len(rows['chunks'])==len(chunks) and len(rows['facts'])==len(chunks), rows
                checks = check_evidence(rows,args.expect_vulnerable)
                if kind in record['legacy'] and baseline is None:
                    old_keys = {s['fact_key'] for c in record['legacy'][kind] for s in c['spans']}
                    assert old_keys.isdisjoint({f['_key'] for f in rows['facts']}), 'legacy occurrence survived repair'
                if baseline is not None:
                    assert rows == baseline, 'canonical-equivalent reingest changed logical rows'
                baseline = rows
                record['ingestions'].append({'kind':kind,'form':form,'facts':result['facts_grounded'],'evidence':checks})
            if not args.expect_vulnerable:
                revised = [{**CASES[0],'text':'No relation remains.'}]
                assert ingest(call,kind,revised)['facts_grounded']==0
                rows = projection(call,kind)
                assert len(rows['facts'])==2 and all(f['evidence_chunk_id']!='c0' for f in rows['facts'])
                check_evidence(rows)
                assert ingest(call,kind,CASES)['facts_grounded']==3
                assert projection(call,kind)==baseline
                record['ingestions'].extend([{'kind':kind,'revision':'withdraw','facts':0},
                    {'kind':kind,'revision':'restore','facts':3}])
        expected = {kind:projection(call,kind) for kind in ['rule','directed']}
        snapshot = ok(call, '/api/admin/export')
    if not args.expect_vulnerable:
        with server(label,'restored') as (call,_):
            ok(call,'/api/admin/import',snapshot)
            for kind in expected:
                assert projection(call,kind)==expected[kind]
                check_evidence(projection(call,kind))
            record['snapshot_roundtrip']=True
    for target in (['source'] if args.expect_vulnerable else ['source','restored']):
        with server(label,target) as (call,_):
            for kind in expected:
                assert projection(call,kind)==expected[kind]
                check_evidence(projection(call,kind),args.expect_vulnerable)
            record['restarts'].append({'store':target,'unchanged':True})
    evidence['modes'][label]=record
mock.shutdown()
mock.server_close()
if not args.expect_vulnerable:
    # Legacy seed calls intentionally reproduce raw prompts from the saved binary.
    expected_raw = 3 if args.seed_legacy_binary else 0
    assert sum(not c['prompt_is_nfc'] for c in CALLS)==expected_raw, CALLS
    assert all(c['model']=='gpt-5.6-luna' for c in CALLS)
evidence['completion_calls']=CALLS
suffix='baseline-' if args.expect_vulnerable else ''
output=args.output or REPO/'docs/issues/evidence'/('unicode-evidence-'+suffix+'http-2026-09-09.json')
output.write_text(json.dumps(evidence,ensure_ascii=False,indent=2)+chr(10))
print(json.dumps({'evidence':str(output),'temporary_root':str(ROOT),
    'ingestions':sum(len(r['ingestions']) for r in evidence['modes'].values()),
    'legacy_repairs':sum(len(r['legacy']) for r in evidence['modes'].values()),
    'snapshot_roundtrips':sum(r.get('snapshot_roundtrip',False) for r in evidence['modes'].values()),
    'restarts':sum(len(r['restarts']) for r in evidence['modes'].values()),
    'local_completion_calls':len(CALLS)},indent=2))
