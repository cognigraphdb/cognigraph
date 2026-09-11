"""Explicit, manifest-bound data-removal transaction; normal pushes use pre-push.py."""
import base64
import hashlib
import json
from pathlib import Path
import re
import tomllib

OID = re.compile(r'[0-9a-f]{40}')


def transaction(manifest, stdin, head):
    """Validate the entire transaction, not a subset selected by the push command."""
    if manifest.get('operation') != 'remove-private-material' or manifest.get('candidate') != head:
        raise RuntimeError('History-removal manifest does not identify this candidate')
    records = manifest.get('updates', [])
    expected = {}
    for row in records:
        ref, old, new = (row.get(k, '') for k in ('ref', 'old', 'new'))
        if (not ref.startswith(('refs/heads/', 'refs/tags/')) or ref in expected
                or not OID.fullmatch(old) or not OID.fullmatch(new)
                or old == '0' * 40 or new == '0' * 40 or old == new):
            raise RuntimeError('Invalid history-removal ref update')
        expected[ref] = (old, new)
    actual = {}
    for line in stdin.splitlines():
        fields = line.split()
        if len(fields) != 4:
            raise RuntimeError('Malformed history-removal push input')
        _, new, ref, old = fields
        if ref in actual:
            raise RuntimeError('Duplicate outgoing ref')
        actual[ref] = (old, new)
    if not expected or actual != expected:
        raise RuntimeError('Push must exactly match the reviewed history-removal transaction')
    if expected.get('refs/heads/main', ('', ''))[1] != head:
        raise RuntimeError('History removal must publish the checked main candidate')
    return expected


def receipt(manifest, updates, root):
    path = Path(manifest['receipt_path'])
    if not path.is_absolute() or path.resolve().is_relative_to(root.resolve()):
        raise RuntimeError('Keep the sensitive removal receipt outside the checkout')
    raw = path.read_bytes()
    if hashlib.sha256(raw).hexdigest() != manifest['receipt_sha256']:
        raise RuntimeError('Removal receipt changed')
    audit = json.loads(raw)
    expected = {ref: new for ref, (_, new) in updates.items()}
    if audit.get('published_tips') != expected or audit.get('passed') is not True:
        raise RuntimeError('Removal receipt does not cover every outgoing tip')
    if audit.get('remaining_matches') != 0 or audit.get('unexpected_tree_changes') != 0:
        raise RuntimeError('History-removal scan has unresolved findings')
    return raw


def run(gate, filename, remote, stdin, head):
    path = Path(filename)
    gate.require(path.is_absolute() and not path.resolve().is_relative_to(gate.ROOT),
                 'Use an absolute, out-of-repository history-removal manifest')
    raw = path.read_bytes()
    manifest = json.loads(raw)
    updates = transaction(manifest, stdin, head)
    audit_raw = receipt(manifest, updates, gate.ROOT)
    gate.clean()
    repository = gate.github_repository(remote)
    gate.require(repository == manifest.get('repository'), 'Removal manifest names another repository')
    state = gate.remote_state(remote)
    expected_state = manifest['remote_state']
    gate.require(state == expected_state, 'Remote refs changed since the removal audit')
    for ref, (old, new) in updates.items():
        gate.require(state.get(ref) == old, 'Remote ref differs from its removal lease')
        gate.output('git', 'rev-parse', f'{new}^{{commit}}')
    # Read the published version without importing contaminated history into this clone.
    content = json.loads(gate.output('gh', 'api',
        f'repos/{repository}/contents/Cargo.toml?ref={state["refs/heads/main"]}'))
    previous = tomllib.loads(base64.b64decode(content['content']).decode())['workspace']['package']['version']
    version = gate.version_at(head)
    gate.require(version > tuple(map(int, previous.split('.'))), 'Bump the cleanup candidate version')
    expected_version = '.'.join(map(str, version))
    lock = tomllib.loads((gate.ROOT / 'Cargo.lock').read_text())
    packages = [p for p in lock['package'] if p['name'].startswith('cognigraph') and 'source' not in p]
    gate.require(packages and all(p['version'] == expected_version for p in packages), 'Refresh Cargo.lock versions')
    gate.require(any(f'- Status: v{expected_version}' in p.read_text()
                     for p in (gate.ROOT / 'docs/changelog').glob('*.md')), 'Missing cleanup release record')
    prs = gate.open_prs(repository)
    gate.require(not prs, 'Process incoming PRs before replacing repository history')
    gate.verify.run('ci')
    gate.verify.run('docker')
    gate.clean()
    gate.require(gate.output('git', 'rev-parse', 'HEAD') == head, 'Candidate changed during verification')
    gate.require(path.read_bytes() == raw and receipt(manifest, updates, gate.ROOT) == audit_raw,
                 'Removal manifest or receipt changed during verification')
    gate.require(gate.remote_state(remote) == state and gate.open_prs(repository) == prs,
                 'Remote refs or incoming PRs changed during verification')
    print(f'PASS history-removal gate: {head}, {len(updates)} exact ref replacements', flush=True)
