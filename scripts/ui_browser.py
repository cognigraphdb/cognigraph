#!/usr/bin/env python3
"""Build both server editions and run Chromium against disposable Native stores.

Requires the built ui/dist candidate and installed Playwright Chromium. No
existing server, database, credentials or provider configuration is reused.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import secrets
import shutil
import socket
import subprocess
import tempfile
import time
import urllib.error
import urllib.request

ROOT = Path(__file__).resolve().parents[1]


def isolated_env():
    # Allowlist rather than trying to enumerate every provider/config variable.
    return {key: os.environ[key] for key in (
        'PATH', 'HOME', 'USER', 'LOGNAME', 'TMPDIR', 'TMP', 'TEMP',
        'SYSTEMROOT', 'CI', 'PLAYWRIGHT_BROWSERS_PATH',
    ) if key in os.environ}


def free_port():
    with socket.socket() as sock:
        sock.bind(('127.0.0.1', 0))
        return sock.getsockname()[1]


def build_binary(edition, work):
    command = ['cargo', 'build', '--locked', '-p', 'cognigraph-server',
               '--no-default-features', '--message-format=json-render-diagnostics']
    if edition == 'enterprise':
        command += ['--features', 'enterprise']
    # Cargo reports the actual executable path, respecting CARGO_TARGET_DIR.
    result = subprocess.run(command, cwd=ROOT, check=True, stdout=subprocess.PIPE, text=True)
    artifacts = [json.loads(line) for line in result.stdout.splitlines()]
    paths = [item['executable'] for item in artifacts
             if item.get('reason') == 'compiler-artifact' and item.get('executable')
             and item['target']['name'] == 'cognigraph-server']
    if len(paths) != 1:
        raise RuntimeError('Cargo did not report exactly one server binary')
    binary = work / 'cognigraph-server'
    shutil.copy2(paths[0], binary)
    return binary


def ready(process, origin, password, edition):
    deadline = time.monotonic() + 30
    while time.monotonic() < deadline:
        if process.poll() is not None:
            raise RuntimeError(f'{edition} server exited during startup')
        try:
            request = urllib.request.Request(origin + '/api/auth/login',
                data=json.dumps({'username': 'admin', 'password': password}).encode(),
                headers={'Content-Type': 'application/json'})
            with urllib.request.urlopen(request, timeout=1) as response:
                token = json.load(response)['token']
            request = urllib.request.Request(origin + '/api/auth/session',
                headers={'Authorization': 'Bearer ' + token})
            with urllib.request.urlopen(request, timeout=1) as response:
                session = json.load(response)
            if session['edition'] != edition or not session['auth_enabled']:
                raise RuntimeError('Unexpected server edition or authentication state')
            return
        except (urllib.error.URLError, TimeoutError):
            time.sleep(0.1)
    raise RuntimeError(f'{edition} server did not become ready')


def run_edition(edition, dist):
    output = ROOT / 'ui/test-results' / edition
    output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=f'cognigraph-ui-{edition}-') as folder:
        work = Path(folder)
        binary = build_binary(edition, work)
        origin = f'http://127.0.0.1:{free_port()}'
        password = secrets.token_urlsafe(24)
        host_password = secrets.token_urlsafe(24)
        env = isolated_env()
        env.update({
            'COGNIGRAPH_HOST': '127.0.0.1', 'COGNIGRAPH_PORT': origin.rsplit(':', 1)[1],
            'COGNIGRAPH_NATIVE_PATH': str(work / 'store.redb'),
            'COGNIGRAPH_UI_DIST': str(dist), 'COGNIGRAPH_AUTH_ENABLED': 'true',
            'COGNIGRAPH_ADMIN_PASSWORD': password,
            'COGNIGRAPH_JWT_SECRET': secrets.token_urlsafe(48),
            'COGNIGRAPH_EMBEDDING_PROVIDER': 'none',
        })
        if edition == 'enterprise':
            env['COGNIGRAPH_HOST_ADMIN_PASSWORD'] = host_password
        print(f'BROWSER [{edition}] {origin}; disposable Native store; providers disabled', flush=True)
        with (output / 'server.log').open('w') as log:
            process = subprocess.Popen([str(binary)], cwd=work, env=env,
                                       stdout=log, stderr=subprocess.STDOUT)
            try:
                ready(process, origin, password, edition)
                test_env = isolated_env()
                test_env.update({'CG_UI_TEST_ORIGIN': origin, 'CG_UI_TEST_EDITION': edition,
                                 'CG_UI_TEST_PASSWORD': password,
                                 'CG_UI_TEST_HOST_PASSWORD': host_password})
                result = subprocess.run(['bun', '--no-env-file', '--bun', 'run', 'test:browser'],
                                        cwd=ROOT / 'ui', env=test_env, check=False)
                if result.returncode:
                    raise RuntimeError(f'{edition} browser regressions failed (exit {result.returncode})')
            finally:
                process.terminate()
                try:
                    process.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
                (output / 'runtime.json').write_text(json.dumps({
                    'edition': edition, 'origin': origin, 'backend': 'native',
                    'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
                    'html_sha256': hashlib.sha256((dist / 'index.html').read_bytes()).hexdigest(),
                    'server_stopped': process.poll() is not None,
                    'temporary_store_removed_on_exit': True,
                }, indent=2) + '\n')
    print(f'PASS browser [{edition}]; server stopped and temporary store removed', flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--edition', choices=['community', 'enterprise', 'both'], default='both')
    parser.add_argument('--dist', type=Path, default=ROOT / 'ui/dist',
                        help='Built candidate directory; useful for negative qualification')
    args = parser.parse_args()
    dist = args.dist.resolve()
    if not (dist / 'index.html').is_file():
        raise RuntimeError('Build the production UI first: python3 scripts/verify.py --suite ui')
    editions = ['community', 'enterprise'] if args.edition == 'both' else [args.edition]
    for edition in editions:
        run_edition(edition, dist)
    print('Not included: external providers, model benchmarks or research holdouts.', flush=True)


if __name__ == '__main__':
    try:
        main()
    except (OSError, RuntimeError, subprocess.CalledProcessError) as error:
        raise SystemExit(str(error)) from None
