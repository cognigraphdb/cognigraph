#!/usr/bin/env python3
"""Validate the CG-64 ArangoDB dump fixtures without Docker.

Checks that every fixture file is listed in the manifest with its exact size
and SHA-256 and nothing else is present, that the committed expected results
equal the independent dataset oracle, and that the reference reader reaches
each fixture's recorded outcome, errors and data.
"""
import argparse
import hashlib
import json
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
import arangodump_dataset as dataset  # noqa: E402
import arangodump_reader as reader  # noqa: E402

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / 'fixtures/arangodump'
TOP_LEVEL = {'manifest.json', 'README.md', 'expected'}


def canonical(items):
    return sorted(json.dumps(item, sort_keys=True, ensure_ascii=False) for item in items)


def check_files(root, manifest):
    problems = []
    listed = set()
    for key, entry in manifest['fixtures'].items():
        directory = root / key
        actual = {p.relative_to(directory).as_posix() for p in directory.rglob('*') if p.is_file()} \
            if directory.is_dir() else set()
        recorded = {item['path']: item for item in entry['files']}
        if actual != set(recorded):
            problems.append(f'{key}: files differ from manifest '
                            f'(extra {sorted(actual - set(recorded))}, missing {sorted(set(recorded) - actual)})')
        for path in actual & set(recorded):
            data = (directory / path).read_bytes()
            if len(data) != recorded[path]['bytes'] or hashlib.sha256(data).hexdigest() != recorded[path]['sha256']:
                problems.append(f'{key}/{path}: bytes or SHA-256 differ from manifest')
        listed |= {(directory / path).relative_to(root).as_posix() for path in recorded}
    for path in root.rglob('*'):
        relative = path.relative_to(root)
        if path.is_file() and relative.parts[0] not in TOP_LEVEL and relative.as_posix() not in listed:
            problems.append(f'{relative}: not listed in the manifest')
    return problems


def check_expected(root):
    problems = []
    for name, definition in dataset.DATASETS.items():
        path = root / 'expected' / f'{name}.json'
        if not path.exists() or json.loads(path.read_text()) != json.loads(
                json.dumps(dataset.expected(definition), ensure_ascii=False)):
            problems.append(f'{path.relative_to(root)}: differs from arangodump_dataset.expected()')
    return problems


def check_outcome(key, entry, report):
    wanted = entry['expected']
    if report['status'] != wanted['outcome']:
        return [f'{key}: expected {wanted["outcome"]}, reader says {report["status"]} {report["errors"]}']
    problems = []
    if wanted['outcome'] == 'rejected':
        actual = canonical({k: e.get(k) for k in w} for e in report['errors']
                           for w in wanted['errors'] if e['code'] == w['code'])
        if canonical(wanted['errors']) != sorted(set(actual)) or len(report['errors']) != len(wanted['errors']):
            problems.append(f'{key}: errors {report["errors"]} != expected {wanted["errors"]}')
        return problems
    oracle = dataset.expected(dataset.DATASETS[entry['dataset']])
    collections = {name: {'type': c['type'], 'documents': {} if wanted.get('empty') else c['documents']}
                   for name, c in oracle['collections'].items()}
    if json.loads(json.dumps(report['collections'])) != json.loads(json.dumps(collections)):
        problems.append(f'{key}: collections or documents differ from the dataset oracle')
    if canonical(report['constraints']) != canonical(oracle['constraints']):
        problems.append(f'{key}: carried constraints {report["constraints"]}')
    if canonical(report['not_carried']) != canonical(oracle['not_carried']):
        problems.append(f'{key}: not-carried dispositions {report["not_carried"]}')
    if report['ignored'] != wanted.get('ignored', []) or report['warnings'] != wanted.get('warnings', []):
        problems.append(f'{key}: ignored {report["ignored"]} / warnings {report["warnings"]}')
    system = key.endswith('shop-system')
    if bool(report['excluded']) != system or not all(e['collection'].startswith('_') for e in report['excluded']):
        problems.append(f'{key}: excluded {report["excluded"]}')
    return problems


def check(root=FIXTURES):
    manifest = json.loads((root / 'manifest.json').read_text())
    problems = check_files(root, manifest) + check_expected(root)
    outcomes = {'accepted': 0, 'rejected': 0}
    for key, entry in manifest['fixtures'].items():
        report = reader.read(root / key)
        problems += check_outcome(key, entry, report)
        outcomes[report['status']] += 1
    return {'fixtures': len(manifest['fixtures']), **outcomes,
            'sources': {v: s['digest'] for v, s in manifest['sources'].items()}, 'errors': problems}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=FIXTURES)
    summary = check(parser.parse_args().root)
    print(json.dumps(summary, indent=2))
    raise SystemExit(1 if summary['errors'] else 0)
