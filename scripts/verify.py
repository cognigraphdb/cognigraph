#!/usr/bin/env python3
"""Shared local/CI checks. No dependency updates or automatic fixes."""
import argparse
from pathlib import Path
import subprocess
import sys
import docker_images

ROOT = Path(__file__).resolve().parents[1]


def commands(suite):
    if suite == 'ci':
        return [
            (ROOT, ['cargo', 'fmt', '--all', '--', '--check']),
            (ROOT, [sys.executable, '-m', 'unittest', 'discover', '-s', 'scripts/tests']),
            (ROOT, [sys.executable, 'scripts/check-server-modularity.py']),
            (ROOT, [sys.executable, 'scripts/check-editions.py']),
            (ROOT, [sys.executable, 'scripts/check-docs.py']),
            (ROOT, [sys.executable, 'scripts/check-decision-index.py']),
            (ROOT, [sys.executable, 'scripts/issue.py', 'check']),
            (ROOT, ['cargo', 'clippy', '--all-targets', '--', '-D', 'warnings']),
            (ROOT, ['cargo', 'test', '--all']),
            (ROOT, ['cargo', 'clippy', '--all-targets', '--features', 'enterprise', '--', '-D', 'warnings']),
            (ROOT, ['cargo', 'test', '--all', '--features', 'enterprise']),
        ]
    if suite == 'docker':
        return [(ROOT, command) for command in docker_images.build_commands()] + [
            (ROOT, [sys.executable, 'scripts/docker_images.py', 'check'])]
    if suite == 'ui':
        return [(ROOT / 'ui', ['bun', 'install', '--frozen-lockfile']),
                (ROOT / 'ui', ['bun', 'run', 'check']),
                (ROOT / 'ui', ['bun', 'test']),
                (ROOT / 'ui', ['bun', 'run', 'build'])]
    raise ValueError(f'Unknown suite: {suite}')


def run(suite):
    for directory, command in commands(suite):
        print(f'RUN [{suite}] {" ".join(command)}', flush=True)
        result = subprocess.run(command, cwd=directory, check=False)
        if result.returncode:
            raise RuntimeError(f'FAIL: {" ".join(command)} (exit {result.returncode})')
    print(f'PASS [{suite}]', flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--suite', choices=('ci', 'docker', 'ui'), default='ci')
    args = parser.parse_args()
    try:
        run(args.suite)
    except (OSError, RuntimeError) as error:
        print(error, file=sys.stderr)
        raise SystemExit(1)
