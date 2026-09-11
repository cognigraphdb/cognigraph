"""CG-16 release HTTP regression using disposable Native stores.

Build the release server, then run this script. --binary selects a saved
executable; --expect-vulnerable verifies the original silently suppressed document errors.
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
ROOT = Path(tempfile.mkdtemp(prefix='cognigraph-cg16-live-'))


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


SURFACES = ['/api/query', '/api/search/query', '/api/lua/execute']
FORBIDDEN = [
    ('literal', 'RETURN DOCUMENT("_users/cg16-missing")', {}),
    ('bound', 'RETURN DOCUMENT(@id)', {'id':'_users/cg16-missing'}),
    ('array', 'RETURN DOCUMENT(["notes/a", @id])', {'id':'_users/cg16-missing'}),
    ('filter', 'FOR n IN notes FILTER DOCUMENT(n.ref).v == 1 RETURN n._key', {}),
    ('dependent', 'LET d=DOCUMENT("notes/a") RETURN DOCUMENT(d.ref)', {}),
    ('deferred', 'FOR n IN notes LET d=DOCUMENT(n.ref) LET v=d.v SORT n._key LIMIT 1 RETURN v', {}),
]
CONTROLS = [
    ('present', 'RETURN DOCUMENT("notes/a").v', [7]),
    ('missing', 'RETURN DOCUMENT("notes/missing") == null', [True]),
    ('missing_collection', 'RETURN DOCUMENT("absent/missing") == null', [True]),
    ('malformed', 'RETURN DOCUMENT("not-an-id") == null', [True]),
    ('non_string', 'RETURN DOCUMENT(42) == null', [True]),
    ('array_positions', 'FOR d IN DOCUMENT(["notes/a","notes/missing","not-an-id"]) RETURN d == null', [False, True, True]),
]


def invoke(call, surface, query, binds=None, caught=False):
    binds = binds or {}
    if surface == '/api/lua/execute':
        # Bind fixtures contain only this fixed synthetic id.
        assert not binds or binds == {'id':'_users/cg16-missing'}
        lua_binds = '{id="_users/cg16-missing"}' if binds else '{}'
        expression = 'graph.query([==[' + query + ']==], ' + lua_binds + ')'
        script = 'return ' + expression
        if caught:
            script = 'local ok,result=pcall(function() return ' + expression + ' end); return {ok=ok}'
        status, response = call(surface, {'script':script})
        return status, response.get('result') if status == 200 else response
    status, response = call(surface, {'query':query, 'bind_vars':binds})
    return status, response.get('results') if status == 200 else response


def snapshot(call):
    status, rows = invoke(call, '/api/query', 'FOR n IN notes SORT n._key RETURN n')
    assert status == 200, rows
    return rows


evidence = {
    'ticket':'CG-16', 'binary_profile':'release',
    'binary_sha256':hashlib.sha256(BINARY.read_bytes()).hexdigest(),
    'expect_vulnerable':args.expect_vulnerable, 'provider':'none', 'modes':{},
}
for mode in ['resident', 'paged']:
    observations = {'errors':[], 'controls':[], 'plain_explain':0, 'lua_caught':0}
    with server(mode) as (call, directory):
        status, result = call('/api/documents', {
            'collection':'notes', '_key':'a', 'v':7, 'ref':'_users/cg16-missing',
        })
        assert status == 200, result
        before = snapshot(call)
        for name, query, binds in FORBIDDEN:
            for surface in SURFACES:
                for prefix in ['', 'EXPLAIN ANALYZE ']:
                    status, result = invoke(call, surface, prefix + query, binds)
                    assert status == (200 if args.expect_vulnerable else 403), (name, surface, prefix, status, result)
                    if not args.expect_vulnerable:
                        assert 'error' in result and 'forbidden' in result['error'].lower(), result
                    observations['errors'].append({
                        'case':name, 'surface':surface, 'analyzed':bool(prefix),
                        'status':status, 'result':result,
                    })
                status, result = invoke(call, surface, 'EXPLAIN ' + query, binds)
                assert status == 200, (name, surface, status, result)
                observations['plain_explain'] += 1
            status, result = invoke(call, '/api/lua/execute', query, binds, caught=True)
            assert status == 200 and result['ok'] == args.expect_vulnerable, result
            observations['lua_caught'] += 1
        for name, query, expected in CONTROLS:
            for surface in SURFACES:
                for prefix in ['', 'EXPLAIN ANALYZE ']:
                    status, result = invoke(call, surface, prefix + query)
                    assert status == 200, (name, surface, status, result)
                    if prefix:
                        assert result[0]['stats']['result_rows'] == len(expected), result
                    else:
                        assert result == expected, (name, surface, result)
                    observations['controls'].append({
                        'case':name, 'surface':surface, 'analyzed':bool(prefix), 'status':status,
                    })
        assert snapshot(call) == before
    with server(mode) as (call, directory):
        assert snapshot(call) == before
        observations['restart_unchanged'] = True
    evidence['modes'][mode] = observations

suffix = 'baseline-' if args.expect_vulnerable else ''
output = REPO / 'docs/issues/evidence' / ('document-errors-' + suffix + 'http-2026-09-08.json')
output.write_text(json.dumps(evidence, indent=2) + '\n')
print(json.dumps({'evidence':str(output), 'temporary_root':str(ROOT), 'checks':{
    'error_queries':sum(len(v['errors']) for v in evidence['modes'].values()),
    'control_queries':sum(len(v['controls']) for v in evidence['modes'].values()),
    'plain_explain':sum(v['plain_explain'] for v in evidence['modes'].values()),
    'lua_caught':sum(v['lua_caught'] for v in evidence['modes'].values()),
    'restarts':2,
}}, indent=2))
