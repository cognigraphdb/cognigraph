#!/usr/bin/env python3
"""Check CG-26's six server module families against the documented 450-line cap."""
import argparse
import json
from pathlib import Path

FAMILIES = ('promotions', 'jobs', 'materialized_repairs', 'artifact_consumption',
            'governance', 'routes/construct')
SOFT_CAP = 450


def check(root):
    source = root / 'crates/cognigraph-server/src'
    exceptions = json.loads((root / 'docs/issues/evidence/server-modularity-exceptions.json').read_text())
    errors, inventory = [], {}
    for family in FAMILIES:
        directory = source / family
        if (source / (family+'.rs')).exists():
            errors.append(f'unsplit legacy module: {family}.rs')
        if not (directory / 'mod.rs').is_file() or not (directory / 'tests/mod.rs').is_file():
            errors.append(f'missing module/test entry point: {family}')
        for path in sorted(directory.rglob('*.rs')):
            relative = path.relative_to(root).as_posix()
            lines = len(path.read_text().splitlines())
            inventory[relative] = lines
            exception = exceptions.get(relative)
            if lines > SOFT_CAP:
                if not exception:
                    errors.append(f'{relative}: {lines} lines exceeds {SOFT_CAP}; split at a coherent seam')
                elif not exception.get('reason') or lines > exception.get('max_lines', 0):
                    errors.append(f'{relative}: exceeds or lacks its documented exception')
            elif exception:
                errors.append(f'{relative}: remove the no-longer-needed exception')
    for path in exceptions:
        if path not in inventory:
            errors.append(f'exception does not identify an in-scope source file: {path}')
    return {'families': len(FAMILIES), 'files': len(inventory),
            'files_within_soft_cap': sum(n <= SOFT_CAP for n in inventory.values()),
            'exceptions': len(exceptions), 'largest_file_lines': max(inventory.values(), default=0),
            'inventory': inventory, 'errors': errors}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=Path(__file__).resolve().parents[1])
    args = parser.parse_args()
    result = check(args.root)
    print(json.dumps(result, indent=2))
    raise SystemExit(bool(result['errors']))
