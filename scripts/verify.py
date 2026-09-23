#!/usr/bin/env python3
"""Shared local/CI checks. No dependency updates or automatic fixes."""
import argparse
import os
from pathlib import Path
import subprocess
import sys
import docker_images

ROOT = Path(__file__).resolve().parents[1]


def commands(suite):
    if suite == 'ci':
        return commands('ui') + [
            (ROOT, ['cargo', 'fmt', '--all', '--', '--check']),
            (ROOT, ['actionlint']),
            (ROOT, [sys.executable, 'scripts/check-vendored.py']),
            (ROOT, [sys.executable, 'scripts/check-public-distribution.py']),
            (ROOT, [sys.executable, 'scripts/check-arangodump-fixtures.py']),
            *commands('advisories'),
            *commands('dependencies'),
            (ROOT, [sys.executable, '-m', 'unittest', 'discover', '-s', 'scripts/tests']),
            (ROOT, [sys.executable, 'scripts/check-server-modularity.py']),
            (ROOT, [sys.executable, 'scripts/check-editions.py']),
            (ROOT, [sys.executable, 'scripts/check-docs.py']),
            (ROOT, [sys.executable, 'scripts/check-decision-index.py']),
            (ROOT, [sys.executable, 'scripts/issue.py', 'check']),
            (ROOT, ['cargo', 'clippy', '--locked', '--all-targets', '--', '-D', 'warnings']),
            (ROOT, ['cargo', 'test', '--locked', '--all']),
            (ROOT, ['cargo', 'clippy', '--locked', '--all-targets', '--features', 'enterprise', '--', '-D', 'warnings']),
            (ROOT, ['cargo', 'test', '--locked', '--all', '--features', 'enterprise']),
            *commands('helm'),
            *commands('native'),
            *commands('ui-browser'),
        ]
    if suite == 'docker':
        return [(ROOT, command) for command in docker_images.build_commands()] + [
            (ROOT, [sys.executable, 'scripts/check-image-vulnerabilities.py']),
            (ROOT, [sys.executable, 'scripts/docker_images.py', 'check']),
            (ROOT, [sys.executable, 'scripts/check-container-startup.py']),
            (ROOT, [sys.executable, 'scripts/check-helm.py', '--live']),
            (ROOT, [sys.executable, 'scripts/check-helm.py', '--live', '--enterprise'])]
    if suite == 'advisories':
        return [(ROOT, ['cargo', 'audit', '--deny', 'unsound']), (ROOT / 'ui', ['bun', 'audit'])]
    if suite == 'dependencies':
        return [(ROOT, [sys.executable, 'scripts/dependency_freshness.py'])]
    if suite == 'helm':
        return [(ROOT, [sys.executable, 'scripts/check-helm.py']),
                (ROOT, [sys.executable, 'scripts/check-helm.py', '--enterprise'])]
    if suite == 'native':
        return [(ROOT, [sys.executable, 'scripts/native_ci.py'])]
    if suite == 'ui':
        return [(ROOT / 'ui', ['bun', 'install', '--frozen-lockfile']),
                (ROOT / 'ui', ['bun', 'run', 'check']),
                (ROOT / 'ui', ['bun', 'test']),
                (ROOT / 'ui', ['bun', 'run', 'build'])]
    if suite == 'ui-browser':
        return [(ROOT, [sys.executable, 'scripts/ui_browser.py'])]
    raise ValueError(f'Unknown suite: {suite}')


def run(suite):
    if sys.flags.optimize:
        raise RuntimeError('Verification requires Python assertions; do not use -O')
    environment = os.environ.copy()
    environment.pop('PYTHONOPTIMIZE', None)
    if suite == 'ci':
        environment['COGNIGRAPH_LIVE_LLM'] = '0'
        print('CI excludes live LLM loops and opt-in embedding-provider qualification.', flush=True)
    for directory, command in commands(suite):
        print(f'RUN [{suite}] {" ".join(command)}', flush=True)
        result = subprocess.run(command, cwd=directory, env=environment, check=False)
        if result.returncode:
            raise RuntimeError(f'FAIL: {" ".join(command)} (exit {result.returncode})')
    print(f'PASS [{suite}]', flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--suite', choices=('ci', 'docker', 'ui', 'ui-browser', 'native', 'helm', 'advisories', 'dependencies'), default='ci')
    args = parser.parse_args()
    try:
        run(args.suite)
    except (OSError, RuntimeError) as error:
        print(error, file=sys.stderr)
        raise SystemExit(1)
