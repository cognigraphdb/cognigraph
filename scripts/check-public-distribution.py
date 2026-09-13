#!/usr/bin/env python3
"""Check the public working-tree candidate without requiring private evidence."""

import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]
POLICY = 'scripts/policies/public-distribution.json'
CATALOG = 'docs/evidence/catalog.json'
SHA256 = re.compile(r'[0-9a-f]{64}')
HOST = re.compile(rb'(?<![a-zA-Z0-9-])[a-zA-Z0-9][a-zA-Z0-9.-]*\.up\.railway\.app\b')
SECRET = re.compile(rb'-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----|'
                    rb'\bgh[pousr]_[A-Za-z0-9]{30,}|\bgithub_pat_[A-Za-z0-9_]{40,}|'
                    rb'\bdckr_pat_[A-Za-z0-9_-]{20,}|\bsk-(?:proj-)?[A-Za-z0-9_-]{30,}')


def safe_path(value):
    return (isinstance(value, str) and bool(value) and '\\' not in value
            and not PurePosixPath(value).is_absolute()
            and all(part not in ('', '.', '..') for part in value.split('/')))


def candidate_files(root):
    result = subprocess.check_output(
        ['git', 'ls-files', '--cached', '--others', '--exclude-standard', '-z'], cwd=root)
    return sorted(set(p for p in result.decode().split('\0') if p))


def check(root, names=None, private_root=None):
    policy = json.loads((root / POLICY).read_text())
    names = candidate_files(root) if names is None else names
    errors = []
    public_inputs = policy['semantic_neuron_test_inputs']
    checked = 0
    for name in names:
        path = root / name
        if not safe_path(name):
            errors.append('Unsafe candidate path')
            continue
        if not path.exists() and not path.is_symlink():
            # The ordinary push gate requires a clean index before publication.
            continue
        if path.is_symlink():
            errors.append(f'{name}: symlinks require an explicit distribution review')
            continue
        if not path.is_file():
            continue
        checked += 1
        if (any(name.startswith(prefix) for prefix in policy['private_capture_prefixes'])
                or re.fullmatch(r'ui/[^/]+\.png', name)
                or path.name in policy['private_capture_filenames']):
            errors.append(f'{name}: raw capture belongs outside the public repository')
        content = path.read_bytes()
        limit = policy['max_file_bytes']
        exception = policy['size_exceptions'].get(name)
        if exception:
            if not exception.get('reason'):
                errors.append(f'{name}: size exception lacks rationale')
            limit = exception['max_bytes']
        if len(content) > limit:
            errors.append(f'{name}: {len(content)} bytes exceeds public size limit {limit}')
        if HOST.search(content):
            errors.append(f'{name}: use an example origin; actual Railway hostname is private')
        if SECRET.search(content):
            errors.append(f'{name}: credential-shaped content requires review')
        if name.startswith('fixtures/semantic-neurons/') and path.suffix != '.md':
            expected = public_inputs.get(name)
            if expected is None:
                errors.append(f'{name}: not an approved public test input')
            elif hashlib.sha256(content).hexdigest() != expected['sha256']:
                errors.append(f'{name}: public test input changed; review its policy entry')
    for name in public_inputs:
        if (not safe_path(name) or not name.startswith('fixtures/semantic-neurons/')
                or PurePosixPath(name).suffix not in ('.json', '.jsonl')):
            errors.append('Public test input policy contains an invalid fixture path')
            continue
        if not (root / name).is_file():
            errors.append(f'{name}: required public test input missing')

    catalog = json.loads((root / CATALOG).read_text())
    if catalog.get('schema') != 'cognigraph-private-evidence-catalog-v1':
        errors.append('Unknown evidence catalog schema')
    if catalog.get('repository') != 'cognigraphdb/cognigraph-evidence':
        errors.append('Unexpected private evidence repository')
    ids, originals = set(), set()
    verified = 0
    for entry in catalog.get('artifacts', []):
        original = entry.get('original_path')
        if not safe_path(original):
            errors.append('Unsafe original evidence path')
            continue
        artifact = hashlib.sha256(original.encode()).hexdigest()[:20]
        if entry.get('id') != artifact or artifact in ids or original in originals:
            errors.append(f'{original}: incorrect or duplicate artifact identity')
        ids.add(artifact)
        originals.add(original)
        expected_path = 'archives/2026-09-13-public-boundary/original/' + original
        if entry.get('archive_path') != expected_path or entry.get('access') != 'private':
            errors.append(f'{original}: incorrect archive path or access classification')
        digest = entry.get('sha256', '')
        size = entry.get('bytes')
        if not isinstance(digest, str) or not SHA256.fullmatch(digest):
            errors.append(f'{original}: invalid SHA-256')
        if type(size) is not int or size < 0:
            errors.append(f'{original}: invalid byte length')
        package = entry.get('package', '')
        if not re.fullmatch(r'[a-zA-Z0-9._-]+', package) or package in ('.', '..'):
            errors.append(f'{original}: unsafe package name')
            continue
        descriptor = root / 'docs/evidence' / (package + '.md')
        if not descriptor.is_file():
            errors.append(f'{original}: missing public package descriptor')
        else:
            text = descriptor.read_text()
            section = re.search(r'^## Artifact ' + artifact + r'\n(.*?)(?=^## |\Z)', text, re.M | re.S)
            if (not section or f'`{original}`' not in section[1]
                    or f'- SHA-256: `{digest}`' not in section[1]
                    or f'- Bytes: {size}\n' not in section[1]):
                errors.append(f'{original}: public descriptor disagrees with catalog')
        if private_root is not None:
            path = private_root / expected_path
            if not path.is_file() or not path.resolve().is_relative_to(private_root.resolve()):
                errors.append(f'{original}: missing or unsafe private original')
                continue
            content = path.read_bytes()
            if len(content) != size or hashlib.sha256(content).hexdigest() != digest:
                errors.append(f'{original}: private original disagrees with catalog')
            else:
                verified += 1
            variant = entry.get('published_v2_7_11_variant')
            if variant:
                p = private_root / 'archives/2026-09-13-public-boundary/published-v2.7.11' / original
                if (not p.is_file() or p.stat().st_size != variant['bytes']
                        or hashlib.sha256(p.read_bytes()).hexdigest() != variant['sha256']):
                    errors.append(f'{original}: published variant mismatch')
    return {'candidate_files': checked, 'private_artifact_descriptors': len(ids),
            'public_test_inputs': len(public_inputs), 'private_originals_verified': verified,
            'private_archive_check_requested': private_root is not None, 'errors': errors}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=ROOT)
    parser.add_argument('--private-root', type=Path)
    args = parser.parse_args()
    try:
        result = check(args.root, private_root=args.private_root)
    except (OSError, ValueError, KeyError, TypeError, subprocess.CalledProcessError) as error:
        result = {'errors': [f'Public distribution check could not complete: {type(error).__name__}']}
    print(json.dumps(result, indent=2))
    raise SystemExit(bool(result['errors']))
