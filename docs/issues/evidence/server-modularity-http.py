"""CG-26: replay signed M26 authority over authenticated release HTTP and restart.

Synthetic fixture exported by the existing M26 lifecycle test using
COGNIGRAPH_M26_LIVE_FIXTURE_DIR. No model calls or external data stores.
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


def digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(',', ':')).encode()).hexdigest()


def strip_metadata(value):
    if isinstance(value, dict):
        return {k: strip_metadata(v) for k, v in value.items()
                if k not in {'_id', '_rev', 'created_at', 'updated_at'}}
    if isinstance(value, list):
        return [strip_metadata(v) for v in value]
    return value


@contextlib.contextmanager
def server(binary, directory, fixture, mode, live, arango=None):
    directory.mkdir(parents=True, exist_ok=True)
    with socket.socket() as sock:
        sock.bind(('127.0.0.1', 0))
        port = sock.getsockname()[1]
    env = {k: os.environ[k] for k in ('PATH', 'HOME', 'TMPDIR') if k in os.environ}
    env.update(COGNIGRAPH_HOST='127.0.0.1', COGNIGRAPH_PORT=str(port),
               COGNIGRAPH_BACKEND='native', COGNIGRAPH_NATIVE_PATH=str(directory / 'db.redb'),
               COGNIGRAPH_STORAGE_MODE=mode.split('-')[0],
               COGNIGRAPH_VECTOR_MODE=mode.split('-')[1],
               COGNIGRAPH_EMBEDDING_PROVIDER='none', COGNIGRAPH_AUTH_ENABLED='true',
               COGNIGRAPH_ADMIN_PASSWORD='synthetic-cg26-admin-password',
               COGNIGRAPH_JWT_SECRET='synthetic-cg26-loopback-secret',
               COGNIGRAPH_GOVERNANCE_ROOT_PUBLIC_KEY=live['governance_root_public_key'],
               COGNIGRAPH_ARTIFACT_SOURCE='local-cas',
               COGNIGRAPH_ARTIFACT_CAS_ROOT=str(fixture / 'cas'),
               COGNIGRAPH_REQUEST_TIMEOUT_SECS='180', COGNIGRAPH_SHUTDOWN_TIMEOUT_SECS='15')
    if arango:
        env.update(COGNIGRAPH_BACKEND="arango", **arango)
    tokens = {}

    def call(path, body=None, role='admin', key=None, expected=200):
        headers = {'Content-Type': 'application/json'}
        if role in tokens:
            headers['Authorization'] = 'Bearer ' + tokens[role]
        if key:
            headers['Idempotency-Key'] = key
        req = urllib.request.Request(f'http://127.0.0.1:{port}'+path, headers=headers,
                                     data=None if body is None else json.dumps(body).encode())
        try:
            with urllib.request.urlopen(req, timeout=200) as response:
                status, value = response.status, json.load(response)
        except urllib.error.HTTPError as error:
            status, value = error.code, json.loads(error.read())
        assert status == expected, (path, status, value)
        return value

    def login(role, username, password):
        tokens[role] = call('/api/auth/login', {'username': username, 'password': password},
                            role='anonymous')['token']

    with (directory / 'server.log').open('a') as log:
        process = subprocess.Popen([str(binary)], cwd=directory, env=env, stdout=log, stderr=log)
        try:
            for _ in range(300):
                if process.poll() is not None:
                    raise RuntimeError(f'startup failed; inspect {directory / "server.log"}')
                try:
                    call('/health', role='anonymous')
                    break
                except (OSError, urllib.error.URLError):
                    time.sleep(.1)
            else:
                raise RuntimeError('startup timed out')
            login('admin', 'admin', 'synthetic-cg26-admin-password')
            yield call, login
        finally:
            if process.poll() is None:
                process.send_signal(signal.SIGTERM)
                try:
                    process.wait(timeout=20)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
                    raise RuntimeError('graceful shutdown timed out')


def exercise(binary, directory, fixture, mode, live, snapshot):
    observations = []
    space = live['space_type']
    for restart in [False, True]:
        with server(binary, directory, fixture, mode, live) as (call, login):
            if not restart:
                imported = call('/api/admin/import', snapshot)
                assert imported['collections'] == len(snapshot['collections'])
            login('promoter', live['promoter_username'], live['promoter_password'])
            head = call('/api/semantic-repairs/deployments/current/'+space)['deployment_head']
            assert head['applied_deployment_decision_id'] == live['expected_active_deployment_decision_id']
            generations = call('/api/semantic-repairs/generations')
            assert generations['count'] == 2
            build = call('/api/semantic-repairs/generations', live['build_request'], role='promoter',
                         key=live['build_idempotency_key'])
            assert build['replayed'] and not build['deployed']
            assert build['semantic_repair_generation']['semantic_repair_generation_id'] == live['generation_id']
            deploy = call('/api/semantic-repairs/generations/'+live['generation_id']+'/deploy',
                          live['deploy_authorization'], role='promoter', key=live['deploy_idempotency_key'])
            assert deploy['replayed']
            # Mutation authorization remains role-specific even for existing replay keys.
            call('/api/semantic-repairs/generations', live['build_request'],
                 key=live['build_idempotency_key'], expected=403)
            recovered = call('/api/admin/promotions/recover', {})
            assert recovered['healthy'] and recovered['repaired_heads'] == 0
            status = call('/api/admin/promotions/status')
            assert status['healthy'] and not status['verified_semantic_repair']['repair_required']
            exported = call('/api/admin/export')['collections']
            facts = list(exported['facts']['documents'].values())
            active = [f for f in facts if f.get('space_id') == space]
            assert len(active) == live['expected_active_fact_count']
            assert sorted(f['relation_type'] for f in active) == ['DISTRIBUTES', 'SUPPLIES']
            assert len([f for f in facts if f.get('space_id') == 'other-space']) == 1
            selected = {k: strip_metadata(v) for k, v in exported.items()
                        if k in snapshot['collections'] and k not in {'_users', '_tokens', '_tenants'}}
            for key in ['_cognigraph_jobs', '_cognigraph_evaluation_evidence',
                        '_cognigraph_governance_keys', '_cognigraph_promotion_decisions']:
                assert selected[key] == strip_metadata(snapshot['collections'][key]), key
            observations.append({'restart': restart, 'generation_count': generations['count'],
                                 'active_facts': len(active), 'other_space_facts': 1,
                                 'build_replayed': True, 'deployment_replayed': True,
                                 'wrong_role_rejected': True, 'healthy': True, 'repaired_heads': 0,
                                 'authority_and_graph_sha256': digest(selected)})
    assert observations[0]['authority_and_graph_sha256'] == observations[1]['authority_and_graph_sha256']
    return {'mode': mode, 'observations': observations}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--fixture', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    fixture, binary = args.fixture.resolve(), args.binary.resolve()
    live = json.loads((fixture / 'live.json').read_text())
    snapshot = json.loads((fixture / 'snapshot.json').read_text())
    root = Path(tempfile.mkdtemp(prefix='cognigraph-cg26-http-'))
    results = []
    for mode in ['resident-embedded', 'resident-sidecar', 'paged-sidecar']:
        results.append(exercise(binary, root/mode, fixture, mode, live, snapshot))
        print(mode, 'passed', flush=True)
    result = {'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
              'fixture_snapshot_sha256': hashlib.sha256((fixture/'snapshot.json').read_bytes()).hexdigest(),
              'model_calls': 0, 'isolated_stores': 3, 'authenticated_starts': 6, 'restarts': 3,
              'results': results, 'logs': str(root)}
    args.output.write_text(json.dumps(result, indent=2)+'\n')
