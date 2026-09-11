"""CG-7 release HTTP regression using only disposable Native stores.

Build the release server, then run this script. --binary selects a saved
executable; --expect-vulnerable verifies the original silent mutation behavior.
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
ROOT = Path(tempfile.mkdtemp(prefix='cognigraph-cg7-live-'))
FIXTURE = REPO / 'crates/cognigraph-query/tests/fixtures/mutation_backend_reads.json'
CASES = json.loads(FIXTURE.read_text())


@contextlib.contextmanager
def server(mode):
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


def ok(call, path, body):
    status, result = call(path, body)
    assert status == 200, (path, status, result)
    return result


def query(call, text, binds=None):
    return ok(call, '/api/query', {'query': text, 'bind_vars': binds or {}})['results']


def snapshot(call):
    return query(call, 'FOR d IN notes SORT d._key RETURN d')


def digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True).encode()).hexdigest()


def verify_supported(call):
    fetched = query(call, 'RETURN DOCUMENT(@ref).v', {'ref': 'notes/b'})
    assert fetched == [2], fetched
    assert query(call, 'RETURN DOCUMENT(@ref)', {'ref': 'notes/absent'}) == [None]
    # Caller-fetched values are safe mutation bind variables. These separate
    # operations do not provide a cross-document transactional snapshot.
    assert query(call, 'INSERT {_key:"safe",v:@value} INTO notes RETURN NEW.v', {'value': fetched[0]}) == [2]
    assert query(call, 'UPDATE "safe" WITH {v:3} IN notes RETURN {old:OLD.v,new:NEW.v}') == [{'old':2,'new':3}]
    assert query(call, 'REPLACE "safe" WITH {v:4} IN notes RETURN {old:OLD.v,new:NEW.v}') == [{'old':3,'new':4}]
    assert query(call, 'UPSERT {_key:"safe"} INSERT {_key:"unused"} UPDATE {v:5} IN notes RETURN NEW.v') == [5]
    assert query(call, 'REMOVE "safe" IN notes RETURN OLD.v') == [5]
    assert query(call, 'UPSERT {_key:"safe"} INSERT {_key:"safe",v:6} UPDATE {v:7} IN notes RETURN NEW.v') == [6]
    assert query(call, 'REMOVE "safe" IN notes RETURN OLD.v') == [6]
    lua = ok(call, '/api/lua/execute', {'script': 'local d=graph.query([[RETURN DOCUMENT("notes/b").v]]); return graph.query([[UPDATE "a" WITH {v:@v} IN notes RETURN NEW.v]], {v=d[1]})'})
    assert lua['result'] == [2], lua
    assert query(call, 'UPDATE "a" WITH {v:1} IN notes RETURN NEW.v') == [1]


evidence = {
    'binary_profile': 'release', 'binary_sha256': hashlib.sha256(BINARY.read_bytes()).hexdigest(),
    'fixture_sha256': hashlib.sha256(FIXTURE.read_bytes()).hexdigest(),
    'expect_vulnerable': args.expect_vulnerable, 'provider': 'none', 'modes': {},
}
for mode in ('resident', 'paged'):
    observations = {}
    with server(mode) as (call, directory):
        for key, v in [('a',1), ('b',2)]:
            ok(call, '/api/documents', {'collection':'notes','_key':key,'v':v})
        before = snapshot(call)
        observations['before_sha256'] = digest(before)
        if args.expect_vulnerable:
            returned = query(call, 'UPDATE "a" WITH {copied:DOCUMENT("notes/b").v} IN notes RETURN NEW.copied')
            after = snapshot(call)
            assert returned == [None] and after[0]['copied'] is None and after != before, after
            observations.update(returned=returned, committed_document=after[0], valid_read=query(call, 'RETURN DOCUMENT("notes/b").v'))
            assert observations['valid_read'] == [2]
        else:
            checks = []
            for ref in ('notes/b', 'notes/absent'):
                for case in CASES:
                    for surface in ('http', 'lua'):
                        if surface == 'http':
                            status, response = call('/api/query', {'query':case['query'],'bind_vars':{'ref':ref}})
                            assert status == 400, (case['name'], status, response)
                        else:
                            script = 'local ok,err=pcall(function() return graph.query([==['+case['query']+']==],{ref='+json.dumps(ref)+'}) end); return {ok=ok,error=tostring(err)}'
                            status, response = call('/api/lua/execute', {'script':script})
                            assert status == 200 and response['result']['ok'] is False, (case['name'],status,response)
                        assert case['error'] in json.dumps(response), (case['name'],response)
                        assert snapshot(call) == before, (case['name'],surface,'changed state')
                        checks.append({'case':case['name'],'reference':ref,'surface':surface,'status':status,'unchanged':True})
            observations['rejections'] = checks
            verify_supported(call)
            expected = snapshot(call)
            assert [(d['_key'],d['v']) for d in expected] == [('a',1),('b',2)]
            observations.update(supported_lifecycle=True, after_sha256=digest(expected))
    with server(mode) as (call, directory):
        persisted = snapshot(call)
        assert persisted == (after if args.expect_vulnerable else expected), persisted
        observations['restart_preserved'] = True
    evidence['modes'][mode] = observations

evidence['result'] = 'passed'
evidence['temporary_root'] = str(ROOT)
print(json.dumps(evidence, indent=2))
