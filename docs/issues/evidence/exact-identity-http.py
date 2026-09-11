"""CG-33 release verification: exact handles, durable restart, and offline repair.

Synthetic disposable data and loopback completion/embedding only. --seed-binary
adds repair of a snapshot written by the previous release. No production data.
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
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import urllib.error
import urllib.request
from urllib.parse import quote

parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--binary',type=Path)
parser.add_argument('--seed-binary',type=Path)
parser.add_argument('--expect-vulnerable',action='store_true')
parser.add_argument('--output',type=Path,required=True)
args=parser.parse_args()
REPO=Path(__file__).resolve().parents[3]
BINARY=(args.binary or REPO/'target/release/cognigraph-server').resolve()
CLI=REPO/'target/release/cognigraph'
ROOT=Path(tempfile.mkdtemp(prefix='cognigraph-cg33-live-'))
C,D='café','cafe\u0301'
MODES=['resident-embedded','resident-sidecar','paged-sidecar']
GATE=None
CALLS=[]

class Mock(BaseHTTPRequestHandler):
    def do_POST(self):
        body=json.loads(self.rfile.read(int(self.headers['Content-Length'])))
        stage='embedding' if self.path=='/api/embed' else 'completion'
        CALLS.append(stage)
        selected=GATE
        if selected and stage=='completion':
            selected['entered'].set()
            assert selected['release'].wait(20)
        if stage=='embedding':
            result={'embeddings':[[1.0,0.0] for _ in body['input']]}
        else:
            result={'choices':[{'message':{'content':json.dumps({'pairs':[{'question':'Synthetic identity question','answer':'Synthetic source answer'}]})}}]}
        encoded=json.dumps(result).encode()
        self.send_response(200)
        self.send_header('Content-Type','application/json')
        self.send_header('Content-Length',str(len(encoded)))
        self.end_headers()
        try: self.wfile.write(encoded)
        except (BrokenPipeError,ConnectionResetError): pass # expected after process kill
    def log_message(self,*unused): pass

mock=ThreadingHTTPServer(('127.0.0.1',0),Mock)
threading.Thread(target=mock.serve_forever,daemon=True).start()

@contextlib.contextmanager
def server(mode,label='fixed',binary=BINARY):
    directory=ROOT/mode/label
    directory.mkdir(parents=True,exist_ok=True)
    with socket.socket() as sock:
        sock.bind(('127.0.0.1',0)); port=sock.getsockname()[1]
    env={k:os.environ[k] for k in ('PATH','HOME','TMPDIR') if k in os.environ}
    base=f'http://127.0.0.1:{mock.server_port}'
    env.update(COGNIGRAPH_HOST='127.0.0.1',COGNIGRAPH_PORT=str(port),COGNIGRAPH_BACKEND='native',
        COGNIGRAPH_NATIVE_PATH=str(directory/'db.redb'),COGNIGRAPH_STORAGE_MODE=mode.split('-')[0],
        COGNIGRAPH_VECTOR_MODE=mode.split('-')[1],COGNIGRAPH_EMBEDDING_PROVIDER='ollama',OLLAMA_BASE_URL=base,
        COGNIGRAPH_AUTH_ENABLED='true',COGNIGRAPH_ADMIN_PASSWORD='synthetic-regression-password',
        COGNIGRAPH_JWT_SECRET='synthetic-loopback-secret',COGNIGRAPH_CGQL_MUTATIONS_ENABLED='true',
        COGNIGRAPH_COMPLETION_PROVIDER='openai',OPENAI_API_KEY='synthetic-cg33-key',
        OPENAI_BASE_URL=base+'/openai',COGNIGRAPH_COMPLETION_MODEL='gpt-5.6-luna')
    token=None
    def call(path,body=None,method=None,headers=None):
        merged={'Content-Type':'application/json',**(headers or {})}
        if token: merged['Authorization']='Bearer '+token
        request=urllib.request.Request(f'http://127.0.0.1:{port}'+path,
            data=None if body is None else json.dumps(body).encode(),headers=merged,method=method)
        try:
            with urllib.request.urlopen(request,timeout=20) as response: return response.status,json.load(response)
        except urllib.error.HTTPError as error: return error.code,json.loads(error.read())
    with (directory/'server.log').open('a') as log:
        process=subprocess.Popen([str(binary.resolve())],cwd=directory,env=env,stdout=log,stderr=log)
        try:
            for _ in range(200):
                if process.poll() is not None: raise RuntimeError(f'startup failed: {directory}')
                try:
                    if call('/health')[0]==200: break
                except (OSError,urllib.error.URLError): time.sleep(.1)
            else: raise RuntimeError('startup timeout')
            token=ok(call,'/api/auth/login',{'username':'admin','password':'synthetic-regression-password'})['token']
            yield call,process
        finally:
            if process.poll() is None:
                process.send_signal(signal.SIGTERM)
                try: process.wait(timeout=8)
                except subprocess.TimeoutExpired: process.kill(); process.wait()

def ok(call,path,body=None,method=None,headers=None):
    status,value=call(path,body,method,headers)
    assert status==200,(path,status,value)
    return value

def doc(call,collection,key): return ok(call,'/api/documents/'+quote(collection,safe='')+'/'+quote(key,safe=''))
def new(call,collection,key,**fields): return ok(call,'/api/documents',{'collection':collection,'_key':key,**fields})
def query(call,source,binds=None): return ok(call,'/api/query',{'query':source,'bind_vars':binds or {}})['results']
def terminal(call,job):
    for _ in range(500):
        current=ok(call,'/api/jobs/'+job)
        if current['status'] in ['succeeded','failed','canceled']: return current
        time.sleep(.02)
    raise RuntimeError('job not terminal')
def sides(call):
    status,value=call('/api/documents?collection=side_views')
    return [] if status==404 else value['results']

def offline_repair(mode):
    with server(mode,'legacy',args.seed_binary) as (call,_):
        new(call,'legacy_docs',C,marker='wrong-existing-target')
        new(call,'legacy_docs',D,marker='intended-target')
        new(call,'legacy_docs','end')
        edge=ok(call,'/api/graph/relationships',{'collection':'legacy_edges','from':'legacy_docs/'+D,
            'to':'legacy_docs/end','relation_type':'links'})
        assert edge['_from']=='legacy_docs/'+C
        snapshot=ok(call,'/api/admin/export')
    # Synthetic opaque control bytes verify that repair never edits protected data.
    snapshot['collections']['_cg33_custody_probe']={'type':'document','documents':{'probe':{'opaque':D}}}
    source=ROOT/mode/'legacy.json'; source.write_text(json.dumps(snapshot,ensure_ascii=False))
    original=source.read_bytes()
    def cli(*argv,success=True):
        result=subprocess.run([str(CLI),*map(str,argv)],capture_output=True,text=True)
        assert (result.returncode==0)==success,(argv,result.stdout,result.stderr)
        return json.loads(result.stdout) if success else result.stderr.strip()
    audit=cli('references','audit',source)
    assert any(f['kind']=='ambiguous_reference' and f['collection']=='legacy_edges' for f in audit['findings'])
    plan={'snapshot_sha256':audit['snapshot_sha256'],'changes':[{'collection':'legacy_edges','key':edge['_key'],
        'field':'_from','from':'legacy_docs/'+C,'to':'legacy_docs/'+D}]}
    plan_path=ROOT/mode/'repair-plan.json'; plan_path.write_text(json.dumps(plan))
    out=ROOT/mode/'repaired.json'
    report=cli('references','repair',source,plan_path,'--out',out)
    cli('references','repair',source,plan_path,'--out',out,success=False)
    assert source.read_bytes()==original
    repaired=json.loads(out.read_text())
    expected=json.loads(original); expected['collections']['legacy_edges']['documents'][edge['_key']]['_from']='legacy_docs/'+D
    assert repaired==expected
    bad=json.loads(json.dumps(plan)); bad['changes'][0]['collection']='_cg33_custody_probe'
    plan_path.write_text(json.dumps(bad))
    refused=ROOT/mode/'forbidden.json'
    cli('references','repair',source,plan_path,'--out',refused,success=False)
    assert not refused.exists()
    bad=json.loads(json.dumps(plan)); bad['snapshot_sha256']='0'*64
    plan_path.write_text(json.dumps(bad))
    cli('references','repair',source,plan_path,'--out',refused,success=False)
    assert not refused.exists()
    return repaired,{'edge_key':edge['_key'],'audit_findings':audit['findings_total'],'repair':report,
        'source_preserved':True,'protected_data_preserved':True,'existing_output_and_unsafe_plans_rejected':True}

def run_mode(mode):
    global GATE
    checks=[]
    def check(name,actual,expected,legacy=None):
        required=legacy if args.expect_vulnerable and legacy is not None else expected
        assert actual==required,(mode,name,actual,required)
        checks.append({'case':name,'actual':actual,'expected':expected,'correct':actual==expected})
    repaired,repair_result=(offline_repair(mode) if args.seed_binary else (None,None))
    with server(mode) as (call,process):
        if repaired:
            ok(call,'/api/admin/import',repaired)
            edge=doc(call,'legacy_edges',repair_result['edge_key'])
            check('legacy_repaired_target',edge['_from'],'legacy_docs/'+D)
            check('protected_import',ok(call,'/api/admin/export')['collections']['_cg33_custody_probe']['documents']['probe']['opaque'],D)
            paths=ok(call,'/api/graph/traverse',{'start_vertex':'legacy_docs/'+D,'edge_collection':'legacy_edges','max_depth':1})
            check('repaired_traversal',paths['count'],1)
        new(call,'ends','end')
        for ci,collection in enumerate([C,D]):
            for ki,key in enumerate([C,D]):
                marker=f'{ci}-{ki}'; handle=collection+'/'+key
                new(call,collection,key,marker=marker,text='Synthetic source '+marker,
                    document_id=handle,nested={'opaque':D,'_execution':{'collection':collection,'keys':[key]}},
                    embedding=[1.0,0.0],model_name=key)
                path='/api/documents/'+quote(collection,safe='')+'/'+quote(key,safe='')
                ok(call,path,{'untouched':D},method='PATCH')
                stored=doc(call,collection,key)
                check(marker+'-stored-reference',stored['document_id'],handle,C+'/'+C)
                check(marker+'-opaque',stored['nested']['opaque'],D,C)
                check(marker+'-literal',query(call,'RETURN DOCUMENT('+json.dumps(handle)+').marker'),[marker],['0-0'])
                check(marker+'-bind',query(call,'RETURN DOCUMENT(@id).marker',{'id':handle}),[marker])
                check(marker+'-lua',ok(call,'/api/lua/execute',{'script':'return graph.get_document('+json.dumps(collection,ensure_ascii=False)+','+json.dumps(key,ensure_ascii=False)+').marker'})['result'],marker)
                edge=ok(call,'/api/graph/relationships',{'collection':'rels','from':handle,'to':'ends/end','relation_type':marker})
                check(marker+'-edge',edge['_from'],handle,C+'/'+C)
                paths=ok(call,'/api/graph/traverse',{'start_vertex':handle,'edge_collection':'rels','max_depth':1})
                if ci or ki: check(marker+'-traversal',paths['count'],1,0)
                hits=ok(call,'/api/search/vector',{'collection':collection,'vector':[1.0,0.0],'model_name':key,'limit':2,'threshold':0})
                if ki: check(marker+'-model',len(hits['results']),1,0)
        check('literal_identity',query(call,'RETURN '+json.dumps(C)+' == '+json.dumps(D)),[False],[True])
        if not args.expect_vulnerable:
            check('explicit_text_normalization',query(call,'RETURN NORMALIZE_NFC(@text)',{'text':D}),[C])
        value=ok(call,'/api/batch',{'ops':[{'op':'insert','collection':'payloads','doc':{
            '_key':'batch','_execution':{'collection':D,'keys':[D,C]},'opaque':D}}]})
        check('batch-frozen-payload',doc(call,'payloads','batch')['_execution'],{'collection':D,'keys':[D,C]}, {'collection':C,'keys':[C,C]})
        check('cgql-mutation',query(call,'INSERT { _key: "query", opaque: '+json.dumps(D)+' } INTO payloads RETURN NEW.opaque'),[D],[C])
        if args.expect_vulnerable:
            for collection in [C,D]:
                status,body=call('/api/sideviews/generate',{'collection':collection,'count':1},headers={'Idempotency-Key':'baseline-'+str([C,D].index(collection))})
                check('baseline-sideview-restriction-'+collection,status,202,400)
            snapshot=ok(call,'/api/admin/export')
        else:
            GATE={'entered':threading.Event(),'release':threading.Event()}
            status,body=call('/api/sideviews/generate',{'collection':D,'count':1},headers={'Idempotency-Key':'cg33-restart'})
            assert status==202,body
            job_id=body['job']['id']
            assert GATE['entered'].wait(15)
            snapshot=ok(call,'/api/admin/export')
            payload=snapshot['collections']['_cognigraph_jobs']['documents'][job_id]['_execution']
            check('durable-collection',payload['collection'],D)
            check('durable-keys',sorted(payload['keys']),sorted([D,C]))
            process.kill(); process.wait(timeout=8)
            GATE['release'].set(); GATE=None
    with server(mode) as (call,_):
        if args.expect_vulnerable:
            check('baseline-snapshot-restart',ok(call,'/api/admin/export')['collections']['payloads'],snapshot['collections']['payloads'])
        else:
            completed=terminal(call,job_id)
            check('restarted-job-status',completed['status'],'succeeded')
            check('restarted-job-rows',completed['result']['side_views_written'],2)
            check('exact-parent-handles',sorted(r['document_id'] for r in sides(call)),sorted([D+'/'+D,D+'/'+C]))
            response=ok(call,'/api/documents/'+quote(D,safe='')+'/'+quote(D,safe=''),method='DELETE')
            check('exact-cascade-count',response['side_views_deleted'],1)
            check('collision-peer-survives',[r['document_id'] for r in sides(call)],[D+'/'+C])
            check('collision-peer-document',doc(call,D,C)['marker'],'1-0')
            snapshot=ok(call,'/api/admin/export')
            final_sides=snapshot['collections']['side_views']
    if not args.expect_vulnerable:
        with server(mode) as (call,_):
            check('post-cascade-restart',ok(call,'/api/admin/export')['collections']['side_views'],final_sides)
            check('completed-job-still-terminal',terminal(call,job_id)['status'],'succeeded')
    return {'mode':mode,'checks':checks,'repair':repair_result,'restart_count':1 if args.expect_vulnerable else 2}

try:
    results=[run_mode(mode) for mode in MODES]
    evidence={'binary_sha256':hashlib.sha256(BINARY.read_bytes()).hexdigest(),
        'cli_sha256':hashlib.sha256(CLI.read_bytes()).hexdigest() if args.seed_binary else None,
        'seed_binary_sha256':hashlib.sha256(args.seed_binary.read_bytes()).hexdigest() if args.seed_binary else None,
        'expect_vulnerable':args.expect_vulnerable,'root':str(ROOT),'results':results,
        'provider_calls':{'completion':CALLS.count('completion'),'embedding':CALLS.count('embedding')}}
    args.output.write_text(json.dumps(evidence,ensure_ascii=False,indent=2)+'\n')
    print(json.dumps({'root':str(ROOT),'checks':sum(len(r['checks']) for r in results),
        'mismatches':sum(not c['correct'] for r in results for c in r['checks']),
        'restarts':sum(r['restart_count'] for r in results),'repairs':sum(r['repair'] is not None for r in results),
        'output':str(args.output)}))
finally:
    if GATE: GATE['release'].set()
    mock.shutdown(); mock.server_close()
