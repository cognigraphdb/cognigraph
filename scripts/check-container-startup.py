#!/usr/bin/env python3
"""Exercise root-owned volume provisioning and the actual packaged console.

Runs only disposable loopback containers. --browser additionally needs the
repository's frozen UI dependencies and Playwright Chromium.
"""
import argparse
import json
import os
from pathlib import Path
import re
import secrets
import subprocess
import time
from urllib.request import urlopen

from docker_images import HELPER, http, output, pid1_status, ready, require, status_field
from ui_browser import isolated_env

ROOT = Path(__file__).resolve().parents[1]


def console(base):
    with urlopen(base + '/', timeout=10) as response:
        require('text/html' in response.headers['Content-Type'], 'Console is not HTML')
        html = response.read()
    require(b'CogniGraph' in html, 'Missing console HTML')
    for route in ('/collections', '/query', '/lua'):
        with urlopen(base + route, timeout=10) as response:
            require(response.read() == html, f'Direct route failed: {route}')
    assets = re.findall(r'(?:src|href)="([^"\s]+\.(?:js|css))"', html.decode())
    require(assets and any(path.endswith('.js') for path in assets), 'No bundled script')
    for asset in assets:
        require(asset.startswith('/') and not asset.startswith('//'), 'Non-local bundle URL')
        with urlopen(base + asset, timeout=10) as response:
            require('text/html' not in response.headers['Content-Type'] and response.read(),
                    f'Missing packaged asset: {asset}')


def rejected(options, image):
    probe = output('docker', 'create', *options, image)
    try:
        output('docker', 'start', probe)
        code = subprocess.check_output(['docker', 'wait', probe], text=True, timeout=15).strip()
        logs = subprocess.run(['docker', 'logs', probe], capture_output=True, text=True, check=True)
        require(code != '0' and 'Root startup requires' in logs.stderr,
                'Unsafe root startup was not rejected')
    finally:
        output('docker', 'rm', '--force', '--volumes', probe)


def run(image, edition, browser):
    # Empty root-owned volume reproduces the platform mount, including mode/UID.
    volume = output('docker', 'volume', 'create')
    container = None
    env = os.environ.copy()
    env.update({'COGNIGRAPH_ADMIN_PASSWORD': secrets.token_urlsafe(24),
                'COGNIGRAPH_HOST_ADMIN_PASSWORD': secrets.token_urlsafe(24),
                'COGNIGRAPH_JWT_SECRET': secrets.token_urlsafe(48)})
    common = ['--user=0', '--read-only', '--cap-drop=ALL', '--cap-add=CHOWN',
              '--cap-add=SETUID', '--cap-add=SETGID', '--security-opt=no-new-privileges',
              '--mount', f'type=volume,source={volume},target=/data,volume-nocopy']
    settings = ['--env', 'RAILWAY_VOLUME_MOUNT_PATH=/data', '--env',
                'COGNIGRAPH_NATIVE_PATH=/data/native/cognigraph.redb']
    try:
        # Root without an explicit mount contract must never start the server.
        rejected(common, image)
        command = ['docker', 'create', *common, *settings, '--publish', '127.0.0.1::3000',
                   '--env', 'COGNIGRAPH_AUTH_ENABLED=true', '--env', 'COGNIGRAPH_ADMIN_PASSWORD',
                   '--env', 'COGNIGRAPH_JWT_SECRET', '--env', 'COGNIGRAPH_EMBEDDING_PROVIDER=none']
        if edition == 'enterprise':
            command += ['--env', 'COGNIGRAPH_HOST_ADMIN_PASSWORD']
        container = subprocess.check_output(command + [image], env=env, text=True).strip()
        output('docker', 'start', container)

        def origin():
            port = output('docker', 'port', container, '3000/tcp').split(':')[-1]
            base = f'http://127.0.0.1:{int(port)}'
            ready(base)
            return base

        base = origin()
        status = pid1_status(container)
        require(status_field(status, 'Uid') == ['10001'] * 4, 'PID 1 did not drop root')
        require(status_field(status, 'Gid') == ['10001'] * 4, 'PID 1 kept a root group id')
        require(status_field(status, 'Groups') == [], 'PID 1 kept supplementary groups')
        for capabilities in ('CapInh', 'CapPrm', 'CapEff', 'CapAmb'):
            require(int(status_field(status, capabilities)[0], 16) == 0,
                    f'Server retained capabilities ({capabilities})')
        require(status_field(status, 'NoNewPrivs') == ['1'], 'Privilege escalation not disabled')
        require(output('docker', 'run', '--rm', '--mount', f'type=volume,source={volume},target=/data',
                       HELPER, 'stat', '-c', '%u:%g:%a', '/data/native')
                == '10001:10001:700', 'Unexpected data directory permissions')
        console(base)
        # The image HEALTHCHECK runs the bundled CLI (no curl ships); it must pass.
        for _ in range(90):
            health = output('docker', 'inspect', '-f', '{{.State.Health.Status}}', container)
            if health != 'starting':
                break
            time.sleep(1)
        require(health == 'healthy', f'Image HEALTHCHECK reported {health}')
        http(base + '/api/documents?collection=deployment_probe', status=401)
        token = http(base + '/api/auth/login', {
            'username': 'admin', 'password': env['COGNIGRAPH_ADMIN_PASSWORD']})['token']
        http(base + '/api/documents', {'collection': 'deployment_probe', '_key': 'one',
                                      'text': 'synthetic persistent volume probe'}, token)
        output('docker', 'restart', '--timeout', '30', container)
        base = origin()
        rows = http(base + '/api/search/query', {
            'query': 'FOR d IN deployment_probe RETURN d.text'}, token)['results']
        require(rows == ['synthetic persistent volume probe'], 'Volume restart lost data')
        if browser:
            test_env = isolated_env()
            test_env.update({'CG_UI_TEST_ORIGIN': base, 'CG_UI_TEST_EDITION': edition,
                             'CG_UI_TEST_PASSWORD': env['COGNIGRAPH_ADMIN_PASSWORD'],
                             'CG_UI_TEST_HOST_PASSWORD': env['COGNIGRAPH_HOST_ADMIN_PASSWORD']})
            subprocess.run(['bun', '--no-env-file', '--bun', 'run', 'test:browser'],
                           cwd=ROOT / 'ui', env=test_env, check=True)
        output('docker', 'stop', '--timeout', '30', container)
        require(output('docker', 'inspect', '--format', '{{.State.ExitCode}}', container) == '0',
                'Server did not shut down cleanly')
        # A pre-existing symlink must fail before ownership changes or server startup.
        output('docker', 'run', '--rm', '--user=0',
               '--mount', f'type=volume,source={volume},target=/data', HELPER,
               'sh', '-c', 'mv /data/native /data/saved; ln -s /data/saved /data/native')
        rejected(common + settings, image)
        print(f'PASS container [{edition}]: console/assets, root rejection, UID/capabilities, '
              f'private volume, restart, shutdown, symlink rejection; browser={browser}', flush=True)
    finally:
        if container:
            output('docker', 'rm', '--force', '--volumes', container)
        output('docker', 'volume', 'rm', volume)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--browser', action='store_true')
    args = parser.parse_args()
    for edition, tag in [('community', 'cognigraph:ci'), ('enterprise', 'cognigraph:ci-enterprise')]:
        identity = json.loads(output('docker', 'image', 'inspect', tag))[0]['Id']
        run(identity, edition, args.browser)
