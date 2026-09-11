"""Candidate-local paths and integrity checks; historical inputs stay immutable."""
import hashlib
import json
from pathlib import Path
import unicodedata

ROOT = Path(__file__).resolve().parent
REPO = ROOT.parents[2]
BASE = ROOT.parent / 'luna-documents-2026-09-09'


def normalize(text):
    return ' '.join(unicodedata.normalize('NFC', text).split()).casefold()


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def save(path, value):
    Path(path).write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n')


def canonical_relation(head, relation, tail, symmetric):
    if relation in symmetric:
        head, tail = sorted((head, tail))
    return head, relation, tail


def verify_protocol(current_code=False):
    protocol = json.loads((ROOT / 'protocol.json').read_text())
    assert digest(BASE / 'package-manifest.json') == protocol['baseline_manifest_sha256']
    # Hash the holdout as opaque bytes only. No holdout generation or analysis.
    for name, meta in json.loads((BASE / 'package-manifest.json').read_text())['files'].items():
        assert digest(BASE / name) == meta['sha256'], name
    for name, expected in protocol['package_sha256'].items():
        assert digest(ROOT / name) == expected, name
    if current_code:
        for name, expected in protocol['code_sha256'].items():
            assert digest(REPO / name) == expected, name
    return protocol


def expected_wire(old, doc, settings):
    expected = json.loads(json.dumps(old))
    item = expected['response_format']['json_schema']['schema']['properties']['facts']['items']
    item['properties']['chunk_id'] = {'type': 'string', 'enum': [doc['id']]}
    item['properties']['relation'] = {'type': 'string', 'enum': sorted(r['relation'] for r in settings['taxonomy'])}
    return expected
