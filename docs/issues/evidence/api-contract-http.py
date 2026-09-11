# /// script
# requires-python = ">=3.11"
# dependencies = ["pyyaml", "openapi-spec-validator", "openapi-schema-validator"]
# ///
"""CG-22: validate served OpenAPI and documented requests against release HTTP/CLI.

Disposable synthetic Native store; loopback OpenAI/Gemini/Ollama only. No .env.
Run with `uv run ... --output /tmp/cg22-api.json` after the release build.
"""
import argparse
import copy
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import socket
import subprocess
import tempfile
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import urllib.error
import urllib.request

import yaml
from openapi_schema_validator import OAS30Validator
from openapi_spec_validator import validate

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
REPO = Path(__file__).resolve().parents[3]
SERVER = REPO / 'target/release/cognigraph-server'
CLI = REPO / 'target/release/cognigraph'
FIXTURES = REPO / 'docs/examples/construction'
ROOT = Path(tempfile.mkdtemp(prefix='cognigraph-cg22-live-'))
DIRECTED = 'facts'
CALLS = []
CHECKS = []


def fixture(name):
    return json.loads((FIXTURES / name).read_text())


class Provider(BaseHTTPRequestHandler):
    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
        if self.path == '/api/embed':
            result = {'embeddings': [[1.0, 0.0] for _ in body['input']]}
            CALLS.append({'lane': 'embedding'})
        elif self.path.startswith('/v1beta/models/'):
            pairs = {'pairs': [{'question': 'Who does Alpha supply?', 'answer': 'Alpha supplies Beta.'}]}
            result = {'candidates': [{'content': {'parts': [{'text': json.dumps(pairs)}]}}]}
            CALLS.append({'lane': 'sideviews', 'model': self.path.split('/models/')[1].split(':')[0]})
        else:
            props = body['response_format']['json_schema']['schema']['properties']
            if 'facts' in props:
                if DIRECTED == 'malformed':
                    content = {'wrong': []}
                elif DIRECTED == 'empty':
                    content = {'facts': []}
                else:
                    chunk = re.search(r'\[chunk ([^\]]+)\]', body['messages'][1]['content']).group(1)
                    content = {'facts': [{'source': 'Alpha', 'source_type': 'company', 'target': 'Beta',
                        'target_type': 'company', 'relation': 'SUPPLIES', 'evidence': 'Alpha supplies Beta.', 'chunk_id': chunk}]}
                stage = 'directed'
            elif 'entities' in props:
                content = {'entities': [{'name': v, 'type': 'company', 'aliases': []} for v in ['Alpha', 'Beta']]}
                stage = 'draft-entities'
            else:
                assert 'rules' in props, props
                content = {'rules': [{'source': 'Alpha', 'target': 'Beta', 'relation': 'SUPPLIES', 'when_any': ['supplies']}]}
                stage = 'draft-rules'
            CALLS.append({'lane': 'main', 'stage': stage, 'model': body['model'], 'reasoning_effort': body.get('reasoning_effort')})
            result = {'choices': [{'message': {'content': json.dumps(content)}}]}
        encoded = json.dumps(result).encode()
        self.send_response(200)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Content-Length', str(len(encoded)))
        self.end_headers()
        self.wfile.write(encoded)

    def log_message(self, *unused):
        pass


provider = ThreadingHTTPServer(('127.0.0.1', 0), Provider)
threading.Thread(target=provider.serve_forever, daemon=True).start()
with socket.socket() as sock:
    sock.bind(('127.0.0.1', 0))
    port = sock.getsockname()[1]
base = f'http://127.0.0.1:{port}'
provider_base = f'http://127.0.0.1:{provider.server_port}'
env = {k: os.environ[k] for k in ('PATH', 'HOME', 'TMPDIR') if k in os.environ}
env.update(COGNIGRAPH_HOST='127.0.0.1', COGNIGRAPH_PORT=str(port), COGNIGRAPH_BACKEND='native',
    COGNIGRAPH_NATIVE_PATH=str(ROOT / 'db.redb'), COGNIGRAPH_AUTH_ENABLED='true',
    COGNIGRAPH_ADMIN_PASSWORD='synthetic-cg22-password', COGNIGRAPH_JWT_SECRET='synthetic-cg22-jwt-secret',
    COGNIGRAPH_COMPLETION_PROVIDER='openai', OPENAI_API_KEY='synthetic-openai-key', OPENAI_BASE_URL=provider_base+'/v1',
    COGNIGRAPH_SIDEVIEWS_PROVIDER='gemini', GEMINI_API_KEY='synthetic-gemini-key', GEMINI_BASE_URL=provider_base+'/v1beta',
    COGNIGRAPH_EMBEDDING_PROVIDER='ollama', OLLAMA_BASE_URL=provider_base)
token = None


def call(path, body=None, key=None, method=None, authenticated=True):
    headers = {'Content-Type': 'application/json'}
    if authenticated and token:
        headers['Authorization'] = 'Bearer '+token
    if key:
        headers['Idempotency-Key'] = key
    request = urllib.request.Request(base+path, headers=headers,
        data=None if body is None else json.dumps(body).encode(), method=method)
    try:
        response = urllib.request.urlopen(request, timeout=20)
    except urllib.error.HTTPError as error:
        response = error
    with response:
        raw = response.read().decode()
        try:
            value = json.loads(raw)
        except json.JSONDecodeError:
            value = raw
        return response.status, value, {key.lower(): value for key, value in response.headers.items()}


def expect(name, path, body=None, *, status=200, key=None, authenticated=True):
    actual, result, headers = call(path, body, key, authenticated=authenticated)
    assert actual == status, (name, actual, status, result)
    if 'spec' in globals() and body is not None and path in {
        '/api/jobs', '/api/construct/directed', '/api/construct/draft', '/api/sideviews/generate'}:
        assert str(actual) in spec['paths'][path]['post']['responses'], (path, actual, 'undocumented status')
    CHECKS.append({'case': name, 'path': path, 'status': actual})
    return result, headers


def validate_schema(value, name):
    OAS30Validator({'components': spec['components'], '$ref': '#/components/schemas/'+name}).validate(value)


def terminal(job):
    for _ in range(500):
        status, value, _ = call('/api/jobs/'+job['id'])
        assert status == 200
        if value['status'] in ['succeeded', 'failed', 'canceled']:
            validate_schema(value, 'Job')
            return value
        time.sleep(.02)
    raise AssertionError('job timeout')


def submit(file, key, path='/api/jobs', schema='JobSubmissionRequest'):
    body = fixture(file)
    validate_schema(body, schema)
    result, headers = expect(file, path, body, status=202, key=key)
    validate_schema(result, 'JobSubmission')
    assert headers['location'] == '/api/jobs/'+result['job']['id']
    done = terminal(result['job'])
    assert done['status'] == 'succeeded', done
    replay, _ = expect(file+' replay', path, body, key=key)
    validate_schema(replay, 'JobSubmission')
    assert replay['replayed'] and replay['job']['id'] == done['id']
    return done


def cli(*argv):
    command_env = {**env, 'COGNIGRAPH_URL': base, 'COGNIGRAPH_TOKEN': token}
    result = subprocess.run([str(CLI), *argv], cwd=ROOT, env=command_env, capture_output=True, text=True)
    assert result.returncode == 0, (argv, result.stderr)
    CHECKS.append({'case': 'CLI '+' '.join(argv[:3]), 'exit_code': result.returncode})
    return json.loads(result.stdout)


log = (ROOT/'server.log').open('w')
process = subprocess.Popen([str(SERVER)], cwd=ROOT, env=env, stdout=log, stderr=log)
try:
    for _ in range(200):
        assert process.poll() is None, ROOT/'server.log'
        try:
            if call('/health')[0] == 200:
                break
        except (urllib.error.URLError, OSError):
            time.sleep(.05)
    else:
        raise AssertionError('server startup timeout')
    token = expect('login', '/api/auth/login', {'username': 'admin', 'password': 'synthetic-cg22-password'})[0]['token']
    served = expect('served OpenAPI', '/openapi.yaml')[0]
    assert served == (REPO/'crates/cognigraph-server/openapi.yaml').read_text()
    spec = yaml.safe_load(served)
    validate(spec)
    CHECKS.append({'case': 'served OpenAPI 3.0 validation', 'valid': True})
    assert set(spec['components']['schemas']['JobKind']['enum']) == {'construct.ingest','construct.evaluate','construct.draft','sideviews.generate'}
    for file in ['ingest-job.json', 'evaluate-job.json', 'draft-job.json', 'sideviews-job.json']:
        validate_schema(fixture(file), 'JobSubmissionRequest')
    # Wrong kind/input pairs must be rejected by the tagged schema as well as HTTP.
    wrong = fixture('sideviews-job.json'); wrong['kind'] = 'construct.draft'
    assert not OAS30Validator({'components': spec['components'], '$ref':'#/components/schemas/JobSubmissionRequest'}).is_valid(wrong)
    expect('mismatched kind/input', '/api/jobs', wrong, key='wrong-kind', status=400)
    unknown = fixture('draft-job.json'); unknown['kind'] = 'construct.directed'
    expect('unsupported kind', '/api/jobs', unknown, key='unsupported', status=422)
    expect('missing bearer', '/api/jobs', fixture('draft-job.json'), status=401, authenticated=False)
    expect('missing job key', '/api/jobs', fixture('draft-job.json'), status=400)
    body = fixture('directed.json'); validate_schema(body, 'DirectedConstructRequest')
    result, _ = expect('directed example', '/api/construct/directed', body)
    assert result['facts_grounded'] == 1 and result['proposed'] == 1
    # Preserve an independent second chunk while replacing the original.
    peer = copy.deepcopy(body); peer['chunks'][0]['id'] = 'cg22-peer'
    expect('directed peer', '/api/construct/directed', peer)
    snapshot = expect('before malformed', '/api/admin/export')[0]
    DIRECTED = 'malformed'
    expect('malformed completion preserves occurrences', '/api/construct/directed', body, status=500)
    after = expect('after malformed', '/api/admin/export')[0]
    assert after['collections']['facts'] == snapshot['collections']['facts']
    DIRECTED = 'empty'
    result, _ = expect('empty replacement', '/api/construct/directed', body)
    assert result['facts_grounded'] == 0
    facts = expect('remaining independent occurrence', '/api/documents?collection=facts')[0]['results']
    assert len(facts) == 1
    DIRECTED = 'facts'
    for field in ['space_type','taxonomy','chunks']:
        bad = copy.deepcopy(body); del bad[field]
        expect('directed missing '+field, '/api/construct/directed', bad, status=422)
    for chunks in [[], [{'id':str(i),'text':'Alpha supplies Beta.'} for i in range(33)]]:
        bad = copy.deepcopy(body); bad['chunks'] = chunks
        expect('directed chunk bound '+str(len(chunks)), '/api/construct/directed', bad, status=400)
    for taxonomy in [[], [body['taxonomy'][0]]*2, [{'relation':'SUPPLIES','description':'test','require_in_sentence':[]}]]:
        bad = copy.deepcopy(body); bad['taxonomy'] = taxonomy
        expect('invalid taxonomy', '/api/construct/directed', bad, status=400)
    bad = copy.deepcopy(body); bad['async'] = True
    expect('directed unknown control', '/api/construct/directed', bad, status=422)
    big = copy.deepcopy(body); big['chunks'][0]['text'] = 'x'*(2*1024*1024)
    expect('directed body limit', '/api/construct/directed', big, status=413)
    for file, key in [('ingest-job.json','ingest'),('evaluate-job.json','evaluate'),('draft-job.json','draft')]:
        done = submit(file, key)
        cli('job','status',done['id'])
    done = submit('draft-async.json', 'wrapper-draft', '/api/construct/draft', 'ConstructDraftRequest')
    assert 'async' not in done['input'] and 'per_document' not in done['input']
    expect('async draft requires key', '/api/construct/draft', fixture('draft-async.json'), status=400)
    bad = fixture('draft-job.json'); bad['input']['async'] = True
    expect('generic draft rejects wrapper controls', '/api/jobs', bad, key='bad-control', status=400)
    bad = fixture('draft-job.json'); bad['input']['chunks'] = [{'id':str(i),'title':str(i),'text':'Alpha supplies Beta.'} for i in range(2001)]
    expect('draft document cap', '/api/jobs', bad, key='draft-cap', status=400)
    # A real CLI invocation seeds the example source; both submit surfaces work.
    cli('doc','put','cg22-notes',json.dumps({'_key':'note-1','text':'Alpha supplies Beta.'}))
    done = submit('sideviews-job.json','side-generic')
    assert done['result']['side_views_written'] == 1
    done = submit('sideviews.json','side-wrapper','/api/sideviews/generate','SideviewsGenerateJobInput')
    assert done['result']['side_views_written'] == 0, 'existing rows skip'
    for count, clamped in [(0,1),(51,50),(None,12)]:
        body = fixture('sideviews.json'); body.update(count=count,regenerate=True,text_field=' ')
        validate_schema(body,'SideviewsGenerateJobInput')
        result,_ = expect('side-view count '+str(count),'/api/sideviews/generate',body,status=202,key='count-'+str(count))
        done = terminal(result['job']); assert done['status']=='succeeded',done
        snapshot = expect('frozen side-view options','/api/admin/export')[0]
        frozen = snapshot['collections']['_cognigraph_jobs']['documents'][done['id']]['_execution']
        assert frozen['count']==clamped and frozen['text_field']=='text'
    for count in [-1,1.5]:
        body=fixture('sideviews.json');body['count']=count
        expect('invalid count '+str(count),'/api/sideviews/generate',body,status=400,key='invalid-'+str(count))
    body=fixture('sideviews.json');body['count']=1
    expect('changed idempotent input','/api/sideviews/generate',body,status=409,key='side-wrapper')
    for collection in ['_system','facts','side_views','bad/name']:
        expect('ineligible source '+collection,'/api/sideviews/generate',{'collection':collection},status=400,key=collection)
    page=cli('job','list','--kind','sideviews.generate','--status','succeeded')
    validate_schema(page,'JobList')
    assert page['count'] == 5
    assert all(c['model']=='gpt-5.6-luna' and c['reasoning_effort']=='none' for c in CALLS if c['lane']=='main')
    assert all(c['model']=='gemini-3.8-flash' for c in CALLS if c['lane']=='sideviews')
    assert any(c['lane']=='sideviews' for c in CALLS)
    sha=lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
    evidence={'issue':'CG-22','base_revision':'f74f0aa + uncommitted CG-33/CG-22','root':str(ROOT),
        'binary_sha256':sha(SERVER),'cli_sha256':sha(CLI),'openapi_sha256':hashlib.sha256(served.encode()).hexdigest(),
        'harness_sha256':sha(__file__),'fixtures':{p.name:sha(p) for p in sorted(FIXTURES.glob('*.json'))},
        'checks':CHECKS,'provider_calls':CALLS,'providers':'synthetic loopback OpenAI main, Gemini sideviews, Ollama embeddings; no .env or external calls'}
    args.output.write_text(json.dumps(evidence,indent=2)+'\n')
    print(json.dumps({'checks':len(CHECKS),'provider_calls':len(CALLS),'output':str(args.output)}))
finally:
    if process.poll() is None:
        process.send_signal(signal.SIGTERM)
        try:
            process.wait(timeout=8)
        except subprocess.TimeoutExpired:
            process.kill();process.wait()
    log.close()
    provider.shutdown();provider.server_close()
