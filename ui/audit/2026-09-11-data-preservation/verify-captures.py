"""Recheck the saved synthetic observations; this does not rerun the browser."""
import json
from pathlib import Path

ROOT = Path(__file__).parent

def load(name):
    return json.loads((ROOT / (name + '.json')).read_text())

def same(left, right):
    assert json.dumps(left, sort_keys=True) == json.dumps(right, sort_keys=True)

seed = {row['_key']: row for row in load('00-seeded')['qa_json']}
for row in load('01-title-edits')['qa_json']:
    before = seed[row['_key']]
    same({k: v for k, v in before.items() if k not in ['title', 'updated_at']},
         {k: v for k, v in row.items() if k not in ['title', 'updated_at']})
    assert row['title'] == 'After ' + row['_key']
custom = next(row for row in load('02-custom-and-validation')['qa_json'] if row['_key'] == 'string')
same(custom['content'], seed['string']['content'])
same(custom['custom'], {'flag': False, 'count': None})
same(custom['custom_new'], [1, None, False])

for name, counts in [
    ('03-preview-no-write', (0, 0, 0)), ('04-first-import', (1, 1, 2)),
    ('05-second-import', (2, 2, 4)), ('06-retry', (2, 2, 4)),
    ('07-cancelled', (2, 2, 4)), ('08-confirmed-replacement', (2, 1, 2)),
    ('12-final-confirmed-import', (2, 2, 4)),
]:
    snapshot = load(name)
    assert tuple(len(snapshot[k]) for k in ['chunks', 'facts', 'mentions']) == counts
for collection in ['chunks', 'facts', 'mentions']:
    same(sorted(row['_key'] for row in load('05-second-import')[collection]),
         sorted(row['_key'] for row in load('06-retry')[collection]))
    same(load('06-retry')[collection], load('07-cancelled')[collection])
    same(load('09-intervening-write')[collection], load('10-stale-and-invalid-rejected')[collection])
for collection in ['qa_json', 'chunks', 'facts', 'mentions']:
    same(load('10-stale-and-invalid-rejected')[collection], load('11-viewer-denials')[collection])
assert {row['text'] for row in load('12-final-confirmed-import')['chunks']} == {
    'DataCloud runs on Nimbus.', 'Nimbus hosts DataCloud.'}
sidecar = load('sidecar')
assert 'embedding' not in sidecar['before'] and 'embedding' not in sidecar['after']
same(sidecar['patch'], {'title': 'After'})
assert len(sidecar['vectorsBefore']['results']) == len(sidecar['vectorsAfter']['results']) == 1
created = load('created-after-http')
assert created['title'] == 'Created and edited' and created['content'] is None
same(created['custom'], {'kept': [1, False, None]})
assert 'collection' not in created and 'created_at' in load('created-inspector-before')
same(load('console-final'), [])
same(load('console-created-final'), [])
assert [entry['status'] for entry in load('denied-requests')] == [403, 403]
print('PASS: document fidelity, import identities/retry/cancel/replacement, stale preview, denials, sidecar, and create-flow captures.')
