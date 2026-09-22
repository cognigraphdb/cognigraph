#!/usr/bin/env python3
"""Check incoming PRs before/after CI without merging or changing source files."""
import argparse
import json
from pathlib import Path
import re
import subprocess
import sys

import incoming

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_SNAPSHOT = ROOT / 'target/ci/incoming.json'


def output(*args):
    return subprocess.check_output(args, cwd=ROOT, text=True).strip()


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def ancestor(base, tip):
    result = subprocess.run(['git', 'merge-base', '--is-ancestor', base, tip],
                            cwd=ROOT, capture_output=True, check=False)
    require(result.returncode in (0, 1), 'Missing Git history for incoming-work check')
    return result.returncode == 0


def repository():
    remote = output('git', 'remote', 'get-url', 'origin')
    match = re.fullmatch(r'(?:git@github\.com:|https://github\.com/|ssh://git@github\.com/)([^/]+/[^/]+?)(?:\.git)?', remote)
    require(match is not None, 'Incoming gate requires a GitHub origin without embedded credentials')
    return match[1]


def inspect():
    repo, head = repository(), output('git', 'rev-parse', 'HEAD')
    # Fetch only the integration base. Candidate history comes from fetch-depth: 0.
    output('git', 'fetch', '--quiet', '--no-tags', 'origin', 'refs/heads/develop')
    base = output('git', 'rev-parse', 'FETCH_HEAD')
    require(ancestor(base, head), 'Candidate does not include current develop; integrate and requalify')
    prs = incoming.open_prs(repo, output)
    details = []
    for number, sha, target in prs:
        detail = json.loads(output('gh', 'pr', 'view', str(number), '--repo', repo, '--json',
                            'number,url,title,isDraft,headRefOid,baseRefName,reviewDecision,statusCheckRollup'))
        require((detail['headRefOid'], detail['baseRefName']) == (sha, target),
                f'PR #{number} changed while being inspected')
        details.append(detail)
    require(incoming.open_prs(repo, output) == prs, 'Incoming PRs changed during inspection')
    return {'repository': repo, 'candidate': head, 'develop': base,
            'prs': [list(p) for p in prs], 'details': details}


def compare(previous, current):
    # Check states change during our own CI; identities and targets must not.
    for key in ('repository', 'candidate', 'develop', 'prs'):
        require(previous.get(key) == current[key],
                f'Incoming gate snapshot changed ({key}); review and rerun qualification')


def main(mode, path=DEFAULT_SNAPSHOT):
    current = inspect()
    if mode == 'compare':
        compare(json.loads(path.read_text()), current)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(current, indent=2) + '\n')
    # Save review/check details even when an unprocessed PR blocks qualification.
    # Missing objects for unintegrated PRs require a disposition, not automatic inclusion.
    def included(sha, tip):
        result = subprocess.run(['git', 'merge-base', '--is-ancestor', sha, tip],
                                cwd=ROOT, capture_output=True, check=False)
        return result.returncode == 0
    incoming.verify(current['prs'], current['candidate'], ROOT, included)
    for detail in current['details']:
        if included(detail['headRefOid'], current['candidate']):
            require(detail['reviewDecision'] != 'CHANGES_REQUESTED',
                    f"PR #{detail['number']} has unresolved requested changes")
    print(f"PASS incoming: {len(current['prs'])} open PRs accounted for; "
          f"develop {current['develop']}; details: {path}", flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--mode', choices=('snapshot', 'compare'), default='snapshot')
    parser.add_argument('--snapshot', type=Path, default=DEFAULT_SNAPSHOT)
    args = parser.parse_args()
    try:
        main(args.mode, args.snapshot)
    except (OSError, ValueError, RuntimeError, KeyError, subprocess.CalledProcessError) as error:
        print(f'Incoming gate failed: {error}', file=sys.stderr)
        raise SystemExit(1)
