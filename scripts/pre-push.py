#!/usr/bin/env python3
"""Fail closed unless this clean candidate has reviewed incoming work and passes CI."""
import os
from pathlib import Path
import re
import subprocess
import sys
import tomllib
from types import SimpleNamespace

sys.dont_write_bytecode = True
import verify
import incoming

ROOT = Path(__file__).resolve().parents[1]
ZERO = '0' * 40


def output(*args):
    return subprocess.check_output(args, cwd=ROOT, text=True).strip()


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def ancestor(base, tip):
    result = subprocess.run(['git', 'merge-base', '--is-ancestor', base, tip],
                            cwd=ROOT, capture_output=True, check=False)
    return result.returncode == 0


def version_at(commit):
    data = tomllib.loads(output('git', 'show', f'{commit}:Cargo.toml'))
    version = data['workspace']['package']['version']
    require(re.fullmatch(r'(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)', version),
            'Push gate supports stable product versions; define prerelease policy before using one')
    return tuple(map(int, version.split('.')))


def parse_updates(text, head):
    updates = []
    for line in text.splitlines():
        fields = line.split()
        require(len(fields) == 4, 'Malformed pre-push ref input')
        local_ref, local_oid, remote_ref, remote_oid = fields
        require(all(re.fullmatch(r'[0-9a-f]{40}', value) for value in (local_oid, remote_oid)),
                'Malformed Git object ID')
        require(local_oid != ZERO, 'Ref deletion needs a separate reviewed operation')
        commit = output('git', 'rev-parse', f'{local_oid}^{{commit}}')
        require(commit == head, 'Push only the checked-out HEAD candidate; check out other candidates first')
        require(remote_ref.startswith(('refs/heads/', 'refs/tags/')),
                'Only branch and version-tag refs are supported')
        updates.append((local_ref, local_oid, remote_ref, remote_oid))
    return updates


def github_repository(remote):
    url = output('git', 'remote', 'get-url', '--push', remote)
    match = re.fullmatch(r'(?:git@github\.com:|https://github\.com/|ssh://git@github\.com/)([^/]+/[^/]+?)(?:\.git)?', url)
    require(match is not None, 'Push gate requires a configured GitHub remote without embedded credentials')
    return match[1]


def remote_state(remote):
    return dict((ref, oid) for oid, ref in
                (line.split() for line in output('git', 'ls-remote', '--refs', remote).splitlines()))


def open_prs(repository):
    return incoming.open_prs(repository, output)


def verify_incoming(prs, head):
    incoming.verify(prs, head, ROOT, ancestor)


def review_incoming(mode):
    subprocess.run([sys.executable, str(ROOT / 'scripts/check-incoming.py'), '--mode', mode],
                   cwd=ROOT, check=True)


def clean():
    require(not output('git', 'status', '--porcelain', '--untracked-files=all'),
            'Commit or preserve pending work before push; the checked tree must equal HEAD')


def preflight(remote, updates, head):
    clean()
    repository = github_repository(remote)
    state = remote_state(remote)
    default_branch = output('gh', 'api', f'repos/{repository}', '--jq', '.default_branch')
    default_ref = 'refs/heads/' + default_branch
    require(default_ref in state, 'Remote default branch is missing; initial publication needs explicit setup')
    # Fetch commits needed for version/ancestry checks without changing the worktree.
    subprocess.run(['git', 'fetch', '--quiet', '--no-tags', remote, default_ref], cwd=ROOT, check=True)
    version = version_at(head)
    baselines = {state[default_ref]}
    for _, oid, ref, previous in updates:
        require(state.get(ref, ZERO) == previous, 'Remote ref changed; refresh before pushing')
        if ref.startswith('refs/heads/') and previous != ZERO:
            subprocess.run(['git', 'fetch', '--quiet', '--no-tags', remote, ref], cwd=ROOT, check=True)
            require(ancestor(previous, head), 'Non-fast-forward branch push is not permitted')
            baselines.add(previous)
        if ref.startswith('refs/tags/'):
            require(ref == 'refs/tags/v' + '.'.join(map(str, version)), 'Tag must match the product version')
            require(output('git', 'cat-file', '-t', oid) == 'tag', 'Use an annotated release tag')
            require(previous in (ZERO, oid), 'Do not replace a published tag')
    branch_update = any(ref.startswith('refs/heads/') for _, _, ref, _ in updates)
    for base in baselines:
        if base == head:
            continue
        require(version > version_at(base), 'Bump the product version above the remote candidate before pushing')
    if not branch_update:
        require(ancestor(head, state[default_ref]), 'Tag-only publication must identify an integrated commit')
    lock = tomllib.loads((ROOT / 'Cargo.lock').read_text())
    expected = '.'.join(map(str, version))
    packages = [p for p in lock['package'] if p['name'].startswith('cognigraph') and 'source' not in p]
    require(packages and all(p['version'] == expected for p in packages), 'Refresh workspace versions in Cargo.lock')
    records = list((ROOT / 'docs/changelog').glob('*.md'))
    require(any(f'- Status: v{expected}' in p.read_text() for p in records),
            'Add a changelog record for the outgoing product version')
    prs = open_prs(repository)
    verify_incoming(prs, head)
    return repository, state, prs, baselines


def main():
    require(len(sys.argv) == 3, 'Run through git push; expected remote name and URL')
    remote = sys.argv[1]
    require(remote in output('git', 'remote').splitlines(), 'Use a configured remote name')
    require(sys.argv[2] == output('git', 'remote', 'get-url', '--push', remote),
            'Push URL differs from the configured remote')
    head = output('git', 'rev-parse', 'HEAD')
    stdin = sys.stdin.read()
    removal_manifest = os.environ.pop('COGNIGRAPH_HISTORY_REMOVAL_MANIFEST', None)
    initial_manifest = os.environ.pop('COGNIGRAPH_INITIAL_PUBLICATION_MANIFEST', None)
    require(not (removal_manifest and initial_manifest), 'Select only one exceptional publication mode')
    if initial_manifest:
        import initial_publication
        initial_publication.run(SimpleNamespace(**globals()), initial_manifest, remote, stdin, head)
        return
    if removal_manifest:
        import history_rewrite
        history_rewrite.run(SimpleNamespace(**globals()), removal_manifest, remote, stdin, head)
        return
    updates = parse_updates(stdin, head)
    if not updates:
        return
    repository, state, prs, _ = preflight(remote, updates, head)
    review_incoming('snapshot')
    verify.run('ci')
    verify.run('docker')
    clean()
    require(output('git', 'rev-parse', 'HEAD') == head, 'HEAD changed during verification')
    require(remote_state(remote) == state, 'Remote refs changed during verification; review incoming work again')
    require(open_prs(repository) == prs, 'Incoming PRs changed during verification; review them again')
    review_incoming('compare')
    print(f'PASS pre-push: {head}, {len(prs)} open PRs accounted for', flush=True)


if __name__ == '__main__':
    try:
        main()
    except (OSError, RuntimeError, ValueError, KeyError, subprocess.CalledProcessError) as error:
        print(f'Push blocked: {error}', file=sys.stderr)
        raise SystemExit(1)
