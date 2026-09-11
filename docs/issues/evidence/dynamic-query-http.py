"""CG-8/CG-15 release HTTP regression using disposable Native stores.

Build the release server, then run this script. --binary selects a saved
executable; --expect-vulnerable verifies the original analysis and budget defects.
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
import urllib.error
import urllib.request

parser = argparse.ArgumentParser()
parser.add_argument('--binary', type=Path)
parser.add_argument('--expect-vulnerable', action='store_true')
args = parser.parse_args()
REPO = Path(__file__).resolve().parents[3]
BINARY = (args.binary or REPO / 'target/release/cognigraph-server').resolve()
ROOT = Path(tempfile.mkdtemp(prefix='cognigraph-cg8-live-'))


@contextlib.contextmanager
def server(mode, cap=0):
    directory = ROOT / mode / "store"
    directory.mkdir(parents=True, exist_ok=True)
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        port = sock.getsockname()[1]
    env = {k: os.environ[k] for k in ("PATH", "HOME", "TMPDIR") if k in os.environ}
    env.update(
        COGNIGRAPH_HOST="127.0.0.1", COGNIGRAPH_PORT=str(port),
        COGNIGRAPH_NATIVE_PATH=str(directory / "db.redb"),
        COGNIGRAPH_STORAGE_MODE=mode, COGNIGRAPH_VECTOR_MODE="sidecar",
        COGNIGRAPH_EMBEDDING_PROVIDER="none", COGNIGRAPH_AUTH_ENABLED="true",
        COGNIGRAPH_ADMIN_PASSWORD="synthetic-regression-password",
        COGNIGRAPH_JWT_SECRET="synthetic-loopback-regression-secret",
        COGNIGRAPH_CGQL_MUTATIONS_ENABLED="true",
        COGNIGRAPH_CGQL_MAX_SOURCE_ROWS=str(cap),
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
        process = subprocess.Popen([str(BINARY)], cwd=directory, env=env, stdout=log, stderr=log)
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
    {'name':'filter','query':'FOR n IN notes FILTER DOCUMENT(n.ref).v == 2 RETURN n._key','expected':['a','b'],'units':5},
    {'name':'deferred','query':'FOR n IN notes LET d=DOCUMENT(n.ref) LET v=d.v SORT n._key LIMIT 1 RETURN v','expected':[2],'units':4},
    {'name':'dependent','query':'FOR n IN notes FILTER n._key == "a" LET d=DOCUMENT(n.ref) LET next=DOCUMENT(d.next) RETURN next.v','expected':[3],'units':3},
    {'name':'traversal','query':'FOR n IN notes FILTER n._key == "a" FOR v IN 1..1 OUTBOUND n._id links RETURN v._key','expected':['b','c'],'units':5},
    {'name':'mixed','query':'FOR n IN notes FILTER n._key == "a" FOR v IN 1..1 OUTBOUND n._id links LET d=DOCUMENT(v._id) RETURN d.v','expected':[2,3],'units':9},
    {'name':'array','query':'FOR id IN ["notes/b","notes/c"] FILTER DOCUMENT(id).v >= 2 RETURN id','expected':['notes/b','notes/c'],'units':6},
    {'name':'subquery','query':'FOR n IN notes FILTER n._key == "a" LET xs=(FOR x IN [1,2] RETURN DOCUMENT(n.ref).v) SORT n._key LIMIT 1 RETURN xs','expected':[[2,2]],'units':6},
    {'name':'collect_into','query':'FOR n IN notes COLLECT group=1 INTO ids=DOCUMENT(n.ref)._key RETURN ids','expected':[['b','b','a']],'units':5},
    {'name':'empty','query':'FOR n IN notes FILTER DOCUMENT(n.ref).v == 99 RETURN n._key','expected':[],'units':5},
    {'name':'distinct','query':'FOR n IN notes RETURN DISTINCT DOCUMENT(n.ref).v','expected':[2,1],'units':5},
]
SURFACES = ['/api/query', '/api/search/query', '/api/lua/execute']


def invoke(call, surface, query):
    if surface == '/api/lua/execute':
        script='local ok,result=pcall(function() return graph.query([==['+query+']==]) end); if ok then return {ok=true,rows=result} else return {ok=false,error=tostring(result)} end'
        status, response=call(surface, {'script':script})
        assert status == 200, (status,response)
        value=response['result']
        # Empty Lua tables serialize as null under the current Lua serializer.
        rows=value.get('rows')
        if value['ok'] and rows is None:
            rows=[]
        return status, value['ok'], rows if value['ok'] else value['error']
    status, response=call(surface, {'query':query})
    return status, status==200, response.get('results') if status==200 else response


def succeed(call, surface, query):
    status, success, result=invoke(call,surface,query)
    assert success, (query,status,result)
    return result


def snapshot(call):
    return succeed(call, '/api/query', 'FOR n IN notes SORT n._key RETURN n')


def seed(call):
    for doc in [
        {'_key':'a','v':1,'ref':'notes/b'},
        {'_key':'b','v':2,'ref':'notes/b','next':'notes/c'},
        {'_key':'c','v':3,'ref':'notes/a'},
    ]:
        status,result=call('/api/documents', {'collection':'notes',**doc})
        assert status==200, result
    for key,target in [('ab','b'),('ac','c')]:
        status,result=call('/api/documents', {'collection':'links','_key':key,'_from':'notes/a','_to':'notes/'+target})
        assert status==200, result


evidence={
    'binary_profile':'release','binary_sha256':hashlib.sha256(BINARY.read_bytes()).hexdigest(),
    'expect_vulnerable':args.expect_vulnerable,'provider':'none','cases':CASES,'modes':{},
}
for mode in ('resident','paged'):
    observations={'parity':[],'budgets':[]}
    with server(mode) as (call,directory):
        seed(call)
        before=snapshot(call)
        if args.expect_vulnerable:
            case=CASES[0]
            for surface in SURFACES:
                rows=succeed(call,surface,case['query'])
                report=succeed(call,surface,'EXPLAIN ANALYZE '+case['query'])[0]
                assert rows==case['expected'] and report['stats']['result_rows']==0, (rows,report)
                observations['parity'].append({'surface':surface,'results':rows,'analysis':report})
        else:
            for case in CASES:
                for surface in SURFACES:
                    rows=succeed(call,surface,case['query'])
                    # Lua numeric-index tables can encode empty arrays as null.
                    assert rows==case['expected'], (case['name'],surface,rows)
                    report=succeed(call,surface,'EXPLAIN ANALYZE '+case['query'])[0]
                    assert report['stats']['result_rows']==len(rows), (case['name'],report)
                    assert report['stats']['source_rows']==case['units'], (case['name'],report)
                    outer_return=[s for s in report['stages'] if s['depth']==0 and s['stage'].startswith('RETURN')][-1]
                    assert outer_return['rows']==len(rows), report
                    assert all(s['attempted_rows']>=s['rows'] and s['attempted_runs']>=s['runs'] for s in report['stages']), report
                    observations['parity'].append({'case':case['name'],'surface':surface,'results':rows,'analysis':report})
    caps=[3] if args.expect_vulnerable else sorted({cap for case in CASES for cap in [case['units']-1,case['units']]})
    for cap in caps:
        with server(mode,cap) as (call,directory):
            if args.expect_vulnerable:
                for surface in SURFACES:
                    rows=succeed(call,surface,CASES[0]['query'])
                    assert rows==CASES[0]['expected']
                    observations['budgets'].append({'surface':surface,'cap':cap,'accepted_results':rows,'correct_units':5})
            else:
                for case in CASES:
                    if cap not in (case['units']-1,case['units']):
                        continue
                    for surface in SURFACES:
                        for analyze in (False,True):
                            text=('EXPLAIN ANALYZE ' if analyze else '')+case['query']
                            status,accepted,result=invoke(call,surface,text)
                            if cap<case['units']:
                                assert not accepted and f'query exceeded the source row budget ({cap} rows)' in json.dumps(result), (case['name'],surface,cap,analyze,status,result)
                            else:
                                assert accepted, (case['name'],surface,cap,analyze,status,result)
                                if analyze:
                                    assert result[0]['stats']['source_rows']==cap and result[0]['stats']['result_rows']==len(case['expected'])
                                else:
                                    assert result==case['expected']
                            observations['budgets'].append({'case':case['name'],'surface':surface,'cap':cap,'analyze':analyze,'accepted':accepted,'status':status})
                for surface in SURFACES:
                    plan=succeed(call,surface,'EXPLAIN FOR n IN absent_collection RETURN DOCUMENT(@missing)')
                    assert 'pipeline' in plan[0] and 'analyze' not in plan[0], plan
    with server(mode) as (call,directory):
        assert snapshot(call)==before, 'read-only execution changed persisted data'
        observations['restart_preserved']=True
    evidence['modes'][mode]=observations

evidence.update(result='passed',temporary_root=str(ROOT))
print(json.dumps(evidence,indent=2))
