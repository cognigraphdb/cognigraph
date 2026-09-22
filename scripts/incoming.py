"""Shared exact-head disposition rules for local pushes and GitHub CI."""
import json


def open_prs(repository, output):
    pages = json.loads(output('gh', 'api', '--paginate', '--slurp',
                              f'repos/{repository}/pulls?state=open&per_page=100'))
    return sorted((p['number'], p['head']['sha'], p['base']['ref'])
                  for page in pages for p in page)


def verify(prs, head, root, ancestor):
    path = root / 'docs/operations/pr-dispositions.json'
    records = json.loads(path.read_text()) if path.exists() else []
    for number, sha, _ in prs:
        if ancestor(sha, head):
            continue
        matched = [r for r in records if r.get('number') == number and r.get('head') == sha
                   and r.get('disposition') in ('superseded', 'deferred', 'needs-changes')
                   and r.get('reason', '').strip() and r.get('decision', '').strip()]
        if not matched:
            raise RuntimeError(f'PR #{number} is not integrated or explicitly dispositioned at head {sha}')
