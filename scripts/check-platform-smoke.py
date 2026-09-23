#!/usr/bin/env python3
"""Build both release editions natively and smoke the bare binaries on this host.

CG-87: proves that each edition starts, reports `/health/database` readiness,
accepts an authenticated write, keeps it across a restart and answers the CLI
on the host's own platform (Apple Silicon macOS in CI, any developer machine
locally). No model provider, owned temporary stores, and existing environment
files are not loaded. The report lands under target/ci/platform/.
"""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('edition_probe', ROOT / 'scripts/check-editions-live.py')
probe = importlib.util.module_from_spec(spec)
spec.loader.exec_module(probe)
ARCHITECTURES = {'aarch64': 'arm64', 'arm64': 'arm64', 'x86_64': 'amd64', 'amd64': 'amd64'}
# Cargo bin target names (the CLI package is cognigraph-cli; its binary is cognigraph).
BINARIES = {'cognigraph-server': 'server', 'cognigraph': 'cli'}


def host():
    machine = platform.machine().lower()
    return f'{platform.system().lower()}/{ARCHITECTURES.get(machine, machine)}'


def toolchain():
    return subprocess.check_output(['rustc', '--version'], text=True).strip()


def version():
    return tomllib.loads((ROOT / 'Cargo.toml').read_text())['workspace']['package']['version']


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def build(edition, directory):
    """Build the edition's server and CLI with the release lockfile; copy them aside."""
    command = ['cargo', 'build', '--locked', '--release', '--no-default-features',
               '-p', 'cognigraph-server', '-p', 'cognigraph-cli', '--message-format=json-render-diagnostics']
    if edition == 'enterprise':
        command += ['--features', 'enterprise']
    result = subprocess.run(command, cwd=ROOT, stdout=subprocess.PIPE, text=True, check=True)
    artifacts = [json.loads(line) for line in result.stdout.splitlines() if line.startswith('{')]
    directory.mkdir(parents=True, exist_ok=True)
    binaries = {}
    for target, role in BINARIES.items():
        paths = [item['executable'] for item in artifacts
                 if item.get('reason') == 'compiler-artifact' and item.get('executable')
                 and item.get('target', {}).get('name') == target
                 and 'bin' in item['target'].get('kind', ['bin'])]
        if len(paths) != 1:
            raise RuntimeError(f'Cargo did not report exactly one {target} executable for {edition}')
        binary = directory / target
        shutil.copy2(paths[0], binary)
        os.chmod(binary, 0o755)
        binaries[role] = binary
    return binaries


def smoke(binaries, edition, directory):
    """Start, write, restart, read back over HTTP and the CLI; every step asserts."""
    expected = version()
    config = {'COGNIGRAPH_NATIVE_PATH': str(directory / f'{edition}.redb'),
              'COGNIGRAPH_EMBEDDING_PROVIDER': 'none'}
    checks = 0

    def expect(condition, label):
        nonlocal checks
        assert condition, (edition, label)
        checks += 1

    query = 'FOR d IN platform_probe RETURN d.text'
    with probe.server(binaries['server'], directory, config) as base:
        health = probe.http(base, '/health')
        expect(health['edition'] == edition, 'edition identity')
        expect(health['version'] == expected, 'workspace version identity')
        expect(probe.http(base, '/health/database')['database'] == 'connected', 'database readiness')
        probe.http(base, '/api/documents?collection=platform_probe', status=401)
        expect(True, 'unauthenticated rejection')
        token = probe.login(base)
        probe.http(base, '/api/documents', {'collection': 'platform_probe', '_key': 'one',
                   'text': 'synthetic platform probe'}, token)
        expect(True, 'authenticated insert')
        spec = probe.http(base, '/openapi.yaml')
        expect(('/api/neurons' in spec['paths']) == (edition == 'enterprise'), 'edition API surface')
    # Clean shutdown asserted by the context manager; reopen the same store.
    with probe.server(binaries['server'], directory, config) as base:
        token = probe.login(base)
        rows = probe.http(base, '/api/search/query', {'query': query}, token)['results']
        expect(rows == ['synthetic platform probe'], 'persisted row after restart')
        cli = json.loads(probe.cli(binaries['cli'], base, ['query', query], token))
        expect(cli['results'] == rows, 'CLI query agrees with HTTP')
    return {'edition': edition, 'checks': checks,
            'binary_sha256': {name: digest(path) for name, path in binaries.items()}}


def validate_report(path):
    report = json.loads(path.read_text())
    editions = [item.get('edition') for item in report.get('editions', [])]
    if editions != ['community', 'enterprise'] \
            or not all(item.get('checks', 0) > 0 for item in report['editions']) \
            or not report.get('platform') or not report.get('version'):
        raise RuntimeError('Platform report must cover both editions with passing checks')
    return report


def run(output):
    report = {'issue': 'CG-87', 'platform': host(), 'version': version(), 'toolchain': toolchain(),
              'editions': []}
    print(f'Platform smoke on {report["platform"]} with {report["toolchain"]}', flush=True)
    for edition in ('community', 'enterprise'):
        print(f'BUILD + SMOKE {edition}', flush=True)
        with tempfile.TemporaryDirectory(prefix=f'cg-platform-{edition}-') as temporary:
            directory = Path(temporary)
            binaries = build(edition, directory / 'bin')
            result = smoke(binaries, edition, directory)
        report['editions'].append(result)
        print(f'PASS {edition}: {result["checks"]} checks', flush=True)
    output.mkdir(parents=True, exist_ok=True)
    path = output / (report['platform'].replace('/', '-') + '.json')
    path.write_text(json.dumps(report, indent=2) + '\n')
    validate_report(path)
    print(f'PASS platform smoke [{report["platform"]}]: both editions; report {path}', flush=True)
    return path


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, default=ROOT / 'target/ci/platform')
    args = parser.parse_args()
    try:
        run(args.output)
    except (AssertionError, OSError, RuntimeError, subprocess.CalledProcessError) as error:
        print(f'FAIL: {error!r}', file=sys.stderr)
        raise SystemExit(1)
