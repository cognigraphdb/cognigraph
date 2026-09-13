"""Publish a reviewed root snapshot to an explicitly identified empty repository."""
import json
from pathlib import Path
import re
import tomllib


def run(gate, filename, remote, stdin, head):
    path = Path(filename)
    gate.require(path.is_absolute() and not path.resolve().is_relative_to(gate.ROOT.resolve()),
                 'Keep the initial-publication manifest outside the checkout')
    raw = path.read_bytes()
    manifest = json.loads(raw)
    gate.require(manifest.get('operation') == 'initial-publication'
                 and manifest.get('candidate') == head,
                 'Initial-publication manifest does not identify this candidate')
    updates = gate.parse_updates(stdin, head)
    gate.require(len(updates) == 1 and updates[0][1:] == (head, 'refs/heads/main', gate.ZERO),
                 'Initial publication permits only creation of main at the reviewed root commit')
    gate.clean()
    gate.require(gate.output('git', 'rev-list', '--count', head) == '1',
                 'Initial publication requires exactly one root commit with no inherited history')
    repository = gate.github_repository(remote)
    gate.require(repository == manifest.get('repository'), 'Manifest names another repository')

    def destination():
        metadata = json.loads(gate.output('gh', 'api', f'repos/{repository}'))
        gate.require(type(manifest.get('repository_id')) is int
                     and metadata['id'] == manifest['repository_id'],
                     'Repository identity changed; review the destination again')
        gate.require(type(manifest.get('private')) is bool
                     and metadata['private'] == manifest['private'],
                     'Repository visibility changed; review distribution scope again')
        gate.require(not gate.remote_state(remote), 'Initial-publication destination must be empty')
        gate.require(not gate.open_prs(repository), 'Process incoming PRs before initial publication')

    destination()
    previous = manifest.get('previous_version', '')
    gate.require(isinstance(previous, str)
                 and re.fullmatch(r'(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)', previous),
                 'Record the previously published stable product version')
    version = gate.version_at(head)
    gate.require(version > tuple(map(int, previous.split('.'))),
                 'Bump the product version above the previously published snapshot')
    expected = '.'.join(map(str, version))
    lock = tomllib.loads((gate.ROOT / 'Cargo.lock').read_text())
    packages = [p for p in lock['package'] if p['name'].startswith('cognigraph') and 'source' not in p]
    gate.require(packages and all(p['version'] == expected for p in packages), 'Refresh Cargo.lock versions')
    gate.require(any(f'- Status: v{expected}' in p.read_text()
                     for p in (gate.ROOT / 'docs/changelog').glob('*.md')), 'Missing initial release record')
    for suite in ('ci', 'docker'):
        gate.verify.run(suite)
    gate.clean()
    gate.require(gate.output('git', 'rev-parse', 'HEAD') == head, 'Candidate changed during verification')
    gate.require(path.read_bytes() == raw, 'Initial-publication manifest changed during verification')
    destination()
    print(f'PASS initial-publication gate: {head}, one root commit, empty {repository}', flush=True)
