#!/usr/bin/env python3
"""Verify the CG-70 snapshot offline, allowing only the reviewed manifest patch."""
import hashlib
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]


def check(root=ROOT):
    vendor = root / 'vendor'
    record = json.loads((vendor / 'tantivy-0.26.2.provenance.json').read_text())
    snapshot = vendor / 'tantivy-0.26.2'
    expected = record['original_files']
    actual = {p.relative_to(snapshot).as_posix() for p in snapshot.rglob('*') if p.is_file()}
    if actual != set(expected):
        raise ValueError(f'Vendored file inventory changed: {sorted(actual ^ set(expected))}')
    for name, checksum in expected.items():
        path = snapshot / name
        if path.is_symlink():
            raise ValueError(f'Vendored symlink is not permitted: {name}')
        data = path.read_bytes()
        if name == 'Cargo.toml':
            patched = b'[dependencies.lru]\nversion = "0.18.2"'
            original = b'[dependencies.lru]\nversion = "0.16.3"'
            if data.count(patched) != 1:
                raise ValueError('Tantivy must retain the reviewed safe lru requirement')
            data = data.replace(patched, original)
        if hashlib.sha256(data).hexdigest() != checksum:
            raise ValueError(f'Vendored source differs from the release archive: {name}')


if __name__ == '__main__':
    try:
        check()
    except (OSError, ValueError, KeyError) as error:
        print(f'FAIL: {error}', file=sys.stderr)
        raise SystemExit(1)
    print('PASS: Tantivy release bytes and the single reviewed lru patch')
