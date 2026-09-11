"""Check shared-file preservation, local links and credential-free artifacts."""
import argparse
import ast
import json
from pathlib import Path
import re
import subprocess
from urllib.parse import unquote
from common import ROOT, REPO, digest, save, previous_providers

DOCUMENT_EDITS = {
    'CHANGELOG.md', 'docs/implementation-plan.md', 'docs/decisions/decision_luna_baseline.md',
    'docs/decisions/README.md', 'docs/issues/README.md',
    'docs/decisions/decision_sideviews_provider_and_benchmark.md',
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--snapshot', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    before = json.loads(args.snapshot.read_text())
    preserved = 0
    for name, expected in before.items():
        if name in DOCUMENT_EDITS:
            continue
        path = REPO/name
        actual = digest(path) if path.is_file() else None
        assert actual == expected, 'Pre-existing file changed: '+name
        preserved += 1
    for p in ROOT.glob('*.py'):
        ast.parse(p.read_text())
    # Read values only in memory and never print them or a matching source line.
    keys = []
    for name in ('OPENAI_API_KEY', 'ZHIPU_API_KEY', 'DEEPSEEK_API_KEY'):
        try:
            value = previous_providers.key_from_env(name)
        except RuntimeError:
            continue
        assert len(value) > 10
        keys.append(value.encode())
    files = [p for p in ROOT.rglob('*') if p.is_file() and '__pycache__' not in p.parts]
    for p in files:
        data = p.read_bytes()
        assert all(key not in data for key in keys), 'Credential detected in '+str(p.relative_to(ROOT))
    markdown = [p for p in files if p.suffix == '.md']
    markdown += [REPO/name for name in sorted(DOCUMENT_EDITS)]
    links = 0
    for p in markdown:
        for target in re.findall(r'\]\(([^)]+)\)', p.read_text()):
            if target.startswith(('https://', 'http://', '#', 'mailto:')):
                continue
            target = unquote(target.split('#')[0].strip('<>'))
            if not target:
                continue
            assert (p.parent/target).exists(), f'Broken local link in {p.relative_to(REPO)}: {target}'
            links += 1
    subprocess.run(['git', 'diff', '--check'], cwd=REPO, check=True)
    assert not subprocess.check_output(['git', 'diff', '--cached', '--name-only'], cwd=REPO).strip(), 'Unexpected staged work'
    save(args.output, {'status': 'passed', 'pre_existing_paths_preserved': preserved,
                       'allowed_document_edit_paths': sorted(DOCUMENT_EDITS),
                       'python_sources_parsed': len(list(ROOT.glob('*.py'))),
                       'artifact_files_scanned': len(files), 'credential_values_checked': len(keys),
                       'credential_matches': 0, 'local_markdown_links_checked': links,
                       'git_diff_check': 'passed', 'staging_empty': True,
                       'validator_sha256': digest(Path(__file__))})
    print(f'Preserved {preserved} pre-existing paths; scanned {len(files)} artifacts; {links} local links pass.')


if __name__ == '__main__':
    main()
