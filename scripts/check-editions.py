#!/usr/bin/env python3
"""Verify Native-only dependencies and the Community/Enterprise crate boundary."""
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]
ENTERPRISE = {'cognigraph-governance', 'cognigraph-construct', 'cognigraph-artifacts'}


def main():
    metadata = json.loads(subprocess.check_output(
        ['cargo', 'metadata', '--locked', '--no-deps', '--format-version', '1'],
        cwd=ROOT, text=True))
    assert not any('arango' in package['name'].lower() for package in metadata['packages']), \
        'Native-only workspace must not contain an Arango adapter'
    for package in ('cognigraph-server', 'cognigraph-cli'):
        for enterprise in (False, True):
            command = ['cargo', 'tree', '--locked', '-p', package, '--edges', 'normal',
                       '--prefix', 'none', '--format', '{p}']
            if enterprise:
                command += ['--features', 'enterprise']
            output = subprocess.check_output(command, cwd=ROOT, text=True)
            names = {line.split()[0] for line in output.splitlines() if line.strip()}
            assert not any('arango' in name.lower() for name in names), \
                (package, enterprise, 'Arango runtime dependency is forbidden')
            included = names & ENTERPRISE
            expected = ENTERPRISE if package.endswith('server') else ENTERPRISE - {'cognigraph-construct'}
            assert included == (expected if enterprise else set()), (package, enterprise, included)
            print(f'PASS: {package} {"Enterprise" if enterprise else "Community"} dependencies')


if __name__ == '__main__':
    main()
