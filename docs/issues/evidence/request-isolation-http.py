"""CG-12: paused-provider lifecycle checks against a real release server.
Run with --binary PATH; --expect-vulnerable verifies the saved pre-fix binary.
All credentials are synthetic and all databases are disposable.
"""
import contextlib
import hashlib
import http.client
import urllib.parse
import json
import os
from pathlib import Path
import signal
import socket
import subprocess
import tempfile
import threading
import time
import urllib.error
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

ROOT = Path(tempfile.mkdtemp(prefix="cognigraph-cg12-live-"))
import argparse
parser = argparse.ArgumentParser()
parser.add_argument("--binary", type=Path)
parser.add_argument("--expect-vulnerable", action="store_true")
args = parser.parse_args()
BINARY = args.binary or Path(__file__).resolve().parents[3] / "target/release/cognigraph-server"
ENTERED = threading.Event()
RESUME = threading.Event()


class Mock(BaseHTTPRequestHandler):
    def do_POST(self):
        request = json.loads(self.rfile.read(int(self.headers.get("Content-Length", 0))))
        ENTERED.set()
        if not RESUME.wait(15):
            self.send_error(504, "test provider release timed out")
            return
        data = json.dumps({"data": [{"embedding": [1.0, 0.0]} for _ in request["input"]]}).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.end_headers()
        self.wfile.write(data)

    def log_message(self, *args):
        pass


mock = ThreadingHTTPServer(("127.0.0.1", 0), Mock)
threading.Thread(target=mock.serve_forever, daemon=True).start()


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
        COGNIGRAPH_EMBEDDING_PROVIDER="openai", COGNIGRAPH_AUTH_ENABLED="true",
        COGNIGRAPH_ADMIN_PASSWORD="synthetic-regression-password", COGNIGRAPH_HOST_ADMIN_PASSWORD="synthetic-regression-password",
        COGNIGRAPH_JWT_SECRET="synthetic-loopback-regression-secret",
        COGNIGRAPH_CGQL_MUTATIONS_ENABLED="true", OPENAI_API_KEY="synthetic-stub-only",
        OPENAI_BASE_URL=f"http://127.0.0.1:{mock.server_port}/v1",
        COGNIGRAPH_COMPLETION_MODEL="local-regression-stub",
    )
    env["COGNIGRAPH_DATA_DIR"] = str(directory / "tenants")
    env["COGNIGRAPH_QUERY_CACHE_ENABLED"] = "true"
    token = None

    def call(path, body=None, method=None, bearer=None):
        headers = {"Content-Type": "application/json"}
        auth_token = token if bearer is None else bearer
        if auth_token:
            headers["Authorization"] = "Bearer " + auth_token
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

    call.base_url = f"http://127.0.0.1:{port}"
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
                "username": "host-admin", "password": "synthetic-regression-password",
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

PASSWORD = 'synthetic-tenant-password'

def ok(call, path, body=None, method=None):
    status, result = call(path, body, method)
    assert status == 200, (path, status, result)
    return result

def tenant_session(call, name, username):
    token = ok(call, '/api/auth/login', {'username': username, 'password': PASSWORD})['token']
    tenant = lambda path, body=None, method=None: call(path, body, method, bearer=token)
    tenant.token = token
    return tenant

def create_tenant(call, name, username):
    ok(call, '/api/tenants', {'name': name})
    ok(call, f'/api/tenants/{name}/admin', {'username': username, 'password': PASSWORD})
    return tenant_session(call, name, username)

def paused_embedding(tenant):
    ENTERED.clear()
    RESUME.clear()
    result = []
    def run():
        try:
            result.append(tenant('/api/documents/embed', {'collection': 'notes', 'items': [{'_key': 'a', 'text': 'old request'}], 'upsert': True}))
        except Exception as error:
            result.append(repr(error))
    thread = threading.Thread(target=run, daemon=True)
    thread.start()
    assert ENTERED.wait(5), 'provider was not reached'
    return thread, result

def complete(pending):
    thread, result = pending
    RESUME.set()
    thread.join(10)
    assert not thread.is_alive() and len(result) == 1 and result[0][0] == 200, result
    return result[0][0]

evidence = {'issue': 'CG-12', 'binary_profile': 'release', 'provider': 'paused synthetic loopback embedding HTTP server', 'expected': 'vulnerable baseline' if args.expect_vulnerable else 'isolated requests', 'modes': {}}
try:
    for mode in ('resident', 'paged'):
        observations = {}
        with server(mode) as (call, directory):
            # First backend use follows the provider await. Deletion must not
            # let the old request lazily create a live database for an absent name.
            name = 'cg12deleted'
            tenant = create_tenant(call, name, mode + '-deleted')
            pending = paused_embedding(tenant)
            retired = ok(call, f'/api/tenants/{name}', method='DELETE')
            complete(pending)
            reopened = (directory / 'tenants' / f'{name}.redb').exists()
            assert reopened == args.expect_vulnerable, ('deleted path reopened', reopened)
            fresh = create_tenant(call, name, mode + '-deleted-new')
            status, _ = fresh('/api/documents/notes/a')
            assert status == (200 if args.expect_vulnerable else 404), status
            assert tenant('/api/documents/notes/a')[0] == 401
            observations['deleted_before_resume'] = {'request_status': 200, 'live_database_reopened_by_old_request': reopened, 'new_tenant_document_status': status, 'old_credentials_status': 401, 'quarantined_files': len(retired['quarantined'])}

            # Recreate and seed a same-key document while the provider is paused.
            name = 'cg12recreated'
            tenant = create_tenant(call, name, mode + '-recreated')
            pending = paused_embedding(tenant)
            ok(call, f'/api/tenants/{name}', method='DELETE')
            fresh = create_tenant(call, name, mode + '-recreated-new')
            ok(fresh, '/api/documents', {'collection': 'notes', '_key': 'a', 'text': 'replacement'})
            before = ok(fresh, '/api/documents/notes/a')
            complete(pending)
            after = ok(fresh, '/api/documents/notes/a')
            unchanged = after == before
            assert unchanged != args.expect_vulnerable, (before, after)
            vectors = ok(fresh, '/api/search/vector', {'collection': 'notes', 'vector': [1.0, 0.0]})
            assert vectors['count'] == int(args.expect_vulnerable), vectors
            observations['recreated_before_resume'] = {'request_status': 200, 'replacement_document_unchanged': unchanged, 'replacement_text': after['text'], 'old_vector_count': vectors['count']}

            # Suspension closes admission; already admitted work may finish.
            name = 'cg12suspended'
            tenant = create_tenant(call, name, mode + '-suspended')
            pending = paused_embedding(tenant)
            ok(call, f'/api/tenants/{name}', {'status': 'suspended'})
            assert tenant('/api/documents/notes/a')[0] == 403
            complete(pending)
            ok(call, f'/api/tenants/{name}', {'status': 'active'})
            document = ok(tenant, '/api/documents/notes/a')
            assert document['text'] == 'old request'
            vectors = ok(tenant, '/api/search/vector', {'collection': 'notes', 'vector': [1.0, 0.0]})
            assert vectors['count'] == 1 and vectors['results'][0]['document']['_key'] == 'a', vectors
            observations['suspended_during_provider'] = {'new_request_status': 403, 'admitted_request_status': 200, 'resumed_read_status': 200}

            if not args.expect_vulnerable:
                name = 'cg12users'
                tenant = create_tenant(call, name, mode + '-users')
                url = urllib.parse.urlsplit(call.base_url)
                connection = http.client.HTTPConnection(url.hostname, url.port, timeout=15)
                body = json.dumps({'username': mode + '-stale-user', 'password': PASSWORD, 'role': 'admin'}).encode()
                connection.putrequest('POST', '/api/users')
                connection.putheader('Authorization', 'Bearer ' + tenant.token)
                connection.putheader('Content-Type', 'application/json')
                connection.putheader('Content-Length', str(len(body)))
                connection.endheaders()
                connection.send(body[:1])
                # Admission opens the data handle before JSON body extraction.
                # Observe that exact admission while the rest of the body waits.
                for _ in range(100):
                    tenants = ok(call, '/api/tenants')['tenants']
                    if any(t['name'] == name and t['store_open'] for t in tenants):
                        break
                    time.sleep(0.01)
                else:
                    raise AssertionError('slow-body request was not admitted')
                ok(call, f'/api/tenants/{name}', method='DELETE')
                fresh_users = create_tenant(call, name, mode + '-users-new')
                connection.send(body[1:])
                response = connection.getresponse()
                response.read()
                assert response.status == 401, response.status
                connection.close()
                users = ok(fresh_users, '/api/users')
                assert not any(u['username'] == mode + '-stale-user' for u in users), users
                # Also exercise normal control-store writes through the fence.
                created = ok(fresh_users, '/api/users', {'username': mode + '-valid-user', 'password': PASSWORD, 'role': 'editor'})
                user_path = '/api/users/' + created['key']
                grant = ok(fresh_users, user_path + '/tokens', {'name': 'synthetic-regression'})
                tokens = ok(fresh_users, user_path + '/tokens')
                assert len(tokens) == 1, tokens
                ok(fresh_users, user_path, method='DELETE')
                assert call('/api/documents/notes/a', bearer=grant['token'])[0] == 401
                observations['user_administration'] = {'admitted_stale_request_status': 401, 'replacement_user_list_unchanged': True, 'fresh_user_and_token_crud': 'PASS', 'deleted_user_token_status': 401}
        with server(mode) as (call, directory):
            fresh = tenant_session(call, 'cg12recreated', mode + '-recreated-new')
            assert ok(fresh, '/api/documents/notes/a') == after
            fresh = tenant_session(call, 'cg12deleted', mode + '-deleted-new')
            assert fresh('/api/documents/notes/a')[0] == (200 if args.expect_vulnerable else 404)
            observations['restart_preserved_results'] = True
        evidence['modes'][mode] = observations
    evidence['result'] = 'EXPECTED DEFECT REPRODUCED' if args.expect_vulnerable else 'PASS'
    (ROOT / 'results.json').write_text(json.dumps(evidence, indent=2) + '\n')
    print(json.dumps({'result': evidence['result'], 'evidence': str(ROOT / 'results.json')}, indent=2))
finally:
    RESUME.set()
    mock.shutdown()
