#!/usr/bin/env python3
"""Build metadata, packaged-image checks and opt-in Docker Hub publication."""
import argparse
import json
import os
from pathlib import Path
import re
import secrets
import subprocess
import sys
import time
import tomllib
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen

ROOT = Path(__file__).resolve().parents[1]
REPOSITORY = 'cognigraphdb/cognigraph'
SOURCE = f'https://github.com/{REPOSITORY}'
IMAGES = {'community': ('cognigraph:ci', 'cognigraph/cognigraph'),
          'enterprise': ('cognigraph:ci-enterprise', 'cognigraph/cognigraph-enterprise')}
VERSION_TAG_RULE = r'^[0-9]+\.[0-9]+\.[0-9]+$'


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def output(*args):
    return subprocess.check_output(args, cwd=ROOT, text=True).strip()


def metadata():
    version = tomllib.loads((ROOT / 'Cargo.toml').read_text())['workspace']['package']['version']
    require(re.fullmatch(r'(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)', version),
            'Docker releases require a stable x.y.z workspace version')
    packages = tomllib.loads((ROOT / 'Cargo.lock').read_text())['package']
    local = [p for p in packages if p['name'].startswith('cognigraph') and 'source' not in p]
    require(local and all(p['version'] == version for p in local), 'Cargo.lock version mismatch')
    require(any(re.search(rf'^- Status: v{re.escape(version)}$', p.read_text(), re.M)
                for p in (ROOT / 'docs/changelog').glob('*.md')), 'Missing versioned changelog record')
    revision = output('git', 'rev-parse', 'HEAD')
    require(re.fullmatch(r'[0-9a-f]{40}', revision), 'Invalid commit identity')
    return version, revision


def build_commands():
    version, revision = metadata()
    return [['docker', 'build', '--pull', '--no-cache-filter', 'runtime',
             '--build-arg', f'COGNIGRAPH_EDITION={edition}',
             '--build-arg', f'COGNIGRAPH_VERSION={version}',
             '--build-arg', f'COGNIGRAPH_REVISION={revision}', '-t', image, '.']
            for edition, (image, _) in IMAGES.items()]


def http(url, body=None, token=None, status=200):
    headers = {'Content-Type': 'application/json'}
    if token:
        headers['Authorization'] = f'Bearer {token}'
    request = Request(url, data=json.dumps(body).encode() if body is not None else None,
                      headers=headers)
    try:
        response = urlopen(request, timeout=10)
    except HTTPError as error:
        response = error
    with response:
        payload = response.read()
        require(response.status == status, f'{url}: expected HTTP {status}, got {response.status}')
        return json.loads(payload) if payload else None


def ready(base):
    for _ in range(60):
        try:
            http(base + '/health/database')
            return
        except (OSError, RuntimeError):
            time.sleep(0.5)
    raise RuntimeError('Packaged server did not become ready')


def smoke(image, edition, version):
    # Docker owns this anonymous /data volume; rm -v removes only this probe's data.
    password = secrets.token_urlsafe(24)
    container = output('docker', 'create', '--read-only', '--cap-drop=ALL',
                       '--mount', 'type=volume,target=/data',
                       '--security-opt=no-new-privileges', '--publish', '127.0.0.1::3000',
                       '--env', 'COGNIGRAPH_AUTH_ENABLED=true',
                       '--env', f'COGNIGRAPH_ADMIN_PASSWORD={password}',
                       '--env', f'COGNIGRAPH_JWT_SECRET={secrets.token_urlsafe(32)}',
                       '--env', 'COGNIGRAPH_EMBEDDING_PROVIDER=none', image)
    try:
        output('docker', 'start', container)
        port = output('docker', 'port', container, '3000/tcp').split(':')[-1]
        base = f'http://127.0.0.1:{int(port)}'
        ready(base)
        health = http(base + '/health')
        require((health['version'], health['edition']) == (version, edition), 'Binary identity mismatch')
        require(output('docker', 'exec', container, 'id', '-u') == '10001', 'Runtime UID mismatch')
        http(base + '/api/documents?collection=release_probe', status=401)
        token = http(base + '/api/auth/login', {'username': 'admin', 'password': password})['token']
        http(base + '/api/documents', {'collection': 'release_probe', '_key': 'one',
                                      'text': 'synthetic image publication probe'}, token)
        spec = http(base + '/openapi.yaml')
        require(('/api/neurons' in spec['paths']) == (edition == 'enterprise'), 'Edition API mismatch')
        output('docker', 'exec', container, 'test', '-s', '/usr/share/licenses/cognigraph/LICENSE')
        output('docker', 'exec', container, 'test', '-s', '/usr/share/licenses/cognigraph/LICENSE-COMMERCIAL')
        output('docker', 'restart', '--timeout', '15', container)
        # Docker may allocate a different ephemeral host port after restart.
        port = output('docker', 'port', container, '3000/tcp').split(':')[-1]
        base = f'http://127.0.0.1:{int(port)}'
        ready(base)
        rows = http(base + '/api/search/query', {
            'query': 'FOR d IN release_probe RETURN d.text'}, token)['results']
        require(rows == ['synthetic image publication probe'], 'Persisted CGQL query failed')
        cli = output('docker', 'exec', container, 'cognigraph', '--url', 'http://127.0.0.1:3000',
                     '--token', token, 'query', 'FOR d IN release_probe RETURN d.text')
        require(json.loads(cli)['results'] == rows, 'Packaged CLI query failed')
        print(f'PASS: {edition} {version}: auth, edition API, CLI, licenses, restart and CGQL', flush=True)
    finally:
        subprocess.run(['docker', 'rm', '--force', '--volumes', container], check=True,
                       stdout=subprocess.DEVNULL)


def checked_images(version, revision, platform=None):
    images = {}
    for edition, (tag, _) in IMAGES.items():
        info = json.loads(output('docker', 'image', 'inspect', tag))[0]
        labels = info['Config'].get('Labels') or {}
        require(labels.get('org.opencontainers.image.version') == version
                and labels.get('org.opencontainers.image.revision') == revision
                and labels.get('org.opencontainers.image.source') == SOURCE
                and labels.get('io.cognigraph.edition') == edition, f'{tag}: rebuild stale image metadata')
        require(info['Config']['User'] == 'cognigraph', f'{tag}: expected non-root runtime')
        require(not info['Config'].get('Volumes'),
                f'{tag}: Railway rejects image VOLUME declarations; attach storage at deployment')
        actual = f"{info['Os']}/{info['Architecture']}"
        require(platform is None or actual == platform, f'{tag}: expected {platform}, got {actual}')
        images[edition] = info['Id']
    # Resolve both identities before testing; publication uses these IDs, never mutable local tags.
    for edition, image in images.items():
        smoke(image, edition, version)
    return images


def hub_json(path, missing_ok=False):
    try:
        with urlopen(f'https://hub.docker.com/v2/namespaces/{path}', timeout=30) as response:
            return json.load(response)
    except HTTPError as error:
        with error:
            if error.code == 404 and missing_ok:
                return None
            raise RuntimeError(f'Docker Hub preflight failed for {path}: HTTP {error.code}') from error


def available_tags(version):
    for _, repository in IMAGES.values():
        namespace, name = repository.split('/')
        path = f'{namespace}/repositories/{name}'
        info = hub_json(path, missing_ok=True)
        require(info is not None, f'Create the public Docker Hub repository {repository} before publishing')
        require(info.get('namespace') == namespace and info.get('name') == name
                and info.get('is_private') is False, f'{repository}: public repository identity mismatch')
        require(info.get('immutable_tags_settings') == {'enabled': True, 'rules': [VERSION_TAG_RULE]},
                f'{repository}: require immutable x.y.z tags and a mutable latest alias')
        require(hub_json(f'{path}/tags/{version}', missing_ok=True) is None,
                f'{repository}:{version} already exists; never overwrite a release tag')


def current_candidate(revision):
    require(os.environ.get('GITHUB_ACTIONS') == 'true'
            and os.environ.get('GITHUB_EVENT_NAME') == 'workflow_dispatch'
            and os.environ.get('GITHUB_REPOSITORY') == REPOSITORY
            and os.environ.get('GITHUB_REF') == 'refs/heads/main'
            and os.environ.get('GITHUB_SHA') == revision,
            'Publication is restricted to a manual CI run of cognigraphdb/cognigraph main')
    require(not output('git', 'status', '--porcelain', '--untracked-files=all'), 'Publication requires a clean checkout')
    remote = output('git', 'ls-remote', 'origin', 'refs/heads/main').split()
    require(remote == [revision, 'refs/heads/main'], 'Main changed; publish a newly verified candidate')


def preflight(version, revision):
    current_candidate(revision)
    available_tags(version)


def tag_digest(repository, tag, missing_ok=False):
    namespace, name = repository.split('/')
    info = hub_json(f'{namespace}/repositories/{name}/tags/{tag}', missing_ok=missing_ok)
    if info is None:
        return None
    digest = info.get('digest')
    require(isinstance(digest, str) and re.fullmatch(r'sha256:[0-9a-f]{64}', digest),
            f'{repository}:{tag}: missing or invalid registry digest')
    return digest


def verify_tag(repository, tag, expected):
    # Hub metadata may lag a successful push. Only absence/mismatch is retried;
    # authentication, rate-limit and transport errors still fail closed.
    for attempt in range(6):
        if tag_digest(repository, tag, missing_ok=True) == expected:
            return
        if attempt < 5:
            time.sleep(2)
    raise RuntimeError(f'{repository}:{tag}: registry digest mismatch; inspect remote state before retrying')


def publication_record(target, digest, edition, revision):
    record = f'- `{target}@{digest}` — {edition}, commit `{revision}`\n'
    print(record, flush=True)
    if summary := os.environ.get('GITHUB_STEP_SUMMARY'):
        with open(summary, 'a') as destination:
            destination.write(record)


def push_image(image, target):
    output('docker', 'tag', image, target)
    # Docker reads credentials from its credential store. No token argument, file or log handling here.
    result = output('docker', 'push', target)
    print(result, flush=True)
    digests = re.findall(r'digest: (sha256:[0-9a-f]{64})', result)
    require(len(digests) == 1, f'{target}: push returned no unique digest; inspect remote state before retrying')
    return digests[0]


def publish(version, revision):
    preflight(version, revision)
    images = checked_images(version, revision, 'linux/amd64')
    # Both editions have passed before any upload. Recheck the remote head and both tags.
    preflight(version, revision)
    previous = {edition: tag_digest(repository, 'latest', missing_ok=True)
                for edition, (_, repository) in IMAGES.items()}
    digests = {}
    for edition, image in images.items():
        repository = IMAGES[edition][1]
        target = f'docker.io/{repository}:{version}'
        digest = push_image(image, target)
        # Preserve successful push receipts even if readback or the next upload fails.
        publication_record(target, digest, edition, revision)
        verify_tag(repository, version, digest)
        digests[edition] = digest
    # Advance aliases only after BOTH version tags have verified registry receipts.
    current_candidate(revision)
    for edition, (_, repository) in IMAGES.items():
        require(tag_digest(repository, 'latest', missing_ok=True) == previous[edition],
                f'{repository}: latest changed during publication; inspect remote state')
    for edition, image in images.items():
        current_candidate(revision)
        repository = IMAGES[edition][1]
        target = f'docker.io/{repository}:latest'
        digest = push_image(image, target)
        publication_record(target, digest, edition, revision)
        require(digest == digests[edition], f'{target}: alias differs from the verified version digest')
        verify_tag(repository, 'latest', digest)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command', choices=('check', 'preflight', 'publish'))
    parser.add_argument('--platform', default=os.environ.get('DOCKER_DEFAULT_PLATFORM'),
                        help='Require this platform during a local check (defaults to DOCKER_DEFAULT_PLATFORM)')
    args = parser.parse_args()
    try:
        version, revision = metadata()
        if args.command == 'check':
            checked_images(version, revision, args.platform)
        elif args.command == 'preflight':
            preflight(version, revision)
        else:
            publish(version, revision)
    except (OSError, RuntimeError, ValueError, KeyError, subprocess.CalledProcessError, URLError) as error:
        print(f'FAIL: {error}', file=sys.stderr)
        raise SystemExit(1)
