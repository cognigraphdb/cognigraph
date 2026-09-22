#!/usr/bin/env python3
"""Qualify old-to-new Native stores using two explicitly supplied server binaries.

All data is synthetic and disposable. No provider or hosted service is used.
The previous binary must use the same edition as the candidate.
"""
import argparse
from contextlib import contextmanager
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('upgrade_http', ROOT / 'scripts/check-editions-live.py')
http = importlib.util.module_from_spec(spec)
spec.loader.exec_module(http)


@contextmanager
def server(binary, work, config, crash=False):
    port = http.port()
    env = {k: os.environ[k] for k in ('PATH', 'HOME', 'TMPDIR') if k in os.environ}
    env.update(COGNIGRAPH_HOST='127.0.0.1', COGNIGRAPH_PORT=str(port),
               COGNIGRAPH_AUTH_ENABLED='true', COGNIGRAPH_ADMIN_PASSWORD=http.PASSWORD,
               COGNIGRAPH_JWT_SECRET=http.SECRET, COGNIGRAPH_EMBEDDING_PROVIDER='none')
    env.update(config)
    with tempfile.TemporaryFile(mode='w+') as log:
        process = subprocess.Popen([str(binary)], cwd=work, env=env, stdout=log, stderr=log)
        origin = f'http://127.0.0.1:{port}'
        try:
            for _ in range(200):
                if process.poll() is not None:
                    log.seek(0)
                    raise RuntimeError(log.read())
                try:
                    http.http(origin, '/health/database')
                    break
                except OSError:
                    time.sleep(0.1)
            else:
                raise RuntimeError('Upgrade server readiness timed out')
            yield origin
        finally:
            if process.poll() is None:
                process.send_signal(signal.SIGKILL if crash else signal.SIGTERM)
                try:
                    process.wait(timeout=15)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
                    raise RuntimeError('Upgrade server did not stop') from None
            assert process.returncode == (-signal.SIGKILL if crash else 0), process.returncode


def verify(previous, current, mode, work):
    storage, vector = mode.split('-')
    config = {'COGNIGRAPH_NATIVE_PATH': str(work / 'store.redb'),
              'COGNIGRAPH_STORAGE_MODE': storage, 'COGNIGRAPH_VECTOR_MODE': vector}
    versions = []
    with server(previous, work, config) as origin:
        versions.append(http.http(origin, '/health'))
        admin = http.login(origin)
        http.http(origin, '/api/users', {'username': 'legacy', 'password': http.PASSWORD,
                                       'role': 'viewer'}, admin)
        http.http(origin, '/api/documents', {'collection': 'upgrade', '_key': 'old',
                  'content': 'café provenance evidence', 'nested': {'exact': [1, None, True]}}, admin)
        assert http.http(origin, '/api/search/text', {'collection': 'upgrade', 'query': 'provenance'}, admin)['count'] == 1
    with server(current, work, config) as origin:
        versions.append(http.http(origin, '/health'))
        assert versions[0]['edition'] == versions[1]['edition']
        assert versions[0]['version'] != versions[1]['version']
        admin = http.login(origin)
        viewer = http.login(origin, 'legacy')
        http.http(origin, '/api/auth/login', {'username': 'legacy', 'password': 'wrong'}, status=401)
        old = http.http(origin, '/api/documents/upgrade/old', token=viewer)
        assert old['content'] == 'café provenance evidence'
        assert old['nested'] == {'exact': [1, None, True]}
        assert http.http(origin, '/api/search/text', {'collection': 'upgrade', 'query': 'provenance'}, admin)['count'] == 1
        http.http(origin, '/api/documents/upgrade/old', {'updated': True}, admin, 'PATCH')
        http.http(origin, '/api/documents', {'collection': 'upgrade', '_key': 'new',
                  'content': 'new provenance evidence'}, admin)
        http.http(origin, '/api/users', {'username': 'current', 'password': http.PASSWORD,
                                       'role': 'viewer'}, admin)
    # Abrupt process termination after reopening an already committed store;
    # this exercises recovery, not power-loss durability or interrupted commits.
    with server(current, work, config, crash=True) as origin:
        token = http.login(origin, 'current')
        assert http.http(origin, '/api/documents/upgrade/old', token=token)['updated'] is True
    with server(current, work, config) as origin:
        token = http.login(origin, 'legacy')
        http.login(origin, 'current')
        assert http.http(origin, '/api/documents/upgrade/new', token=token)['content'] == 'new provenance evidence'
        assert http.http(origin, '/api/search/text', {'collection': 'upgrade', 'query': 'provenance'}, token)['count'] == 2
    return {'mode': mode, 'previous': versions[0], 'candidate': versions[1],
            'legacy_login': True, 'new_login': True, 'stored_json_and_text_index': True,
            'write_restart_and_process_crash_reopen': True, 'passed': True}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--previous', type=Path, required=True)
    parser.add_argument('--candidate', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if args.output.exists():
        parser.error('Choose a new evidence output path')
    previous, current = args.previous.resolve(), args.candidate.resolve()
    hashes = {name: hashlib.sha256(path.read_bytes()).hexdigest()
              for name, path in [('previous', previous), ('candidate', current)]}
    assert hashes['previous'] != hashes['candidate'], 'Supply distinct builds'
    runs = []
    with tempfile.TemporaryDirectory(prefix='cognigraph-upgrade-') as folder:
        for mode in ('resident-embedded', 'resident-sidecar', 'paged-sidecar'):
            work = Path(folder) / mode
            work.mkdir()
            runs.append(verify(previous, current, mode, work))
            print(f'PASS dependency upgrade: {mode}', flush=True)
    args.output.write_text(json.dumps({'binary_sha256': hashes, 'runs': runs,
                                      'temporary_stores_removed': True}, indent=2) + '\n')


if __name__ == '__main__':
    main()
