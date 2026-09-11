"""Validate the captured CG-55 counts and timing boundaries (no network calls)."""
import hashlib
import json
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).parent


def load(name):
    return json.loads((ROOT / name).read_text())


def digest(body):
    return hashlib.sha256(json.dumps(body, separators=(',', ':')).encode()).hexdigest()


def query(text):
    return {'query': text, 'bind_vars': {}, 'language': 'cgql'}


cases = [
    ('lua', {'script': 'return graph.create_document("qa_execution", { trial = "cg55-write" })'}, 2),
    ('lua', {'script': 'error("CG55 expected failure")'}, 1),
    ('lua', {'script': 'return "CG55 recovered"'}, 1),
    ('lua', {'script': 'error("CG55 obsolete completion")'}, 1),
    ('lua', {'script': 'error("CG55 departed Lua request")'}, 1),
    ('query', query('FOR d IN qa_execution SORT d._key RETURN { key: d._key, trial: d.trial }'), 1),
    ('query', query('RETURN ('), 1),
    ('query', query('FOR d IN qa_execution FILTER d.trial == "absent" RETURN d'), 1),
    ('query', query('RETURN "CG55 current execution"'), 1),
    ('query', query('RETURN "CG55 retained owner"'), 2),
]
requests = load('requests.json')['requests']
executions = [r for r in requests if r['kind'] != 'validation']
expected = Counter({(kind, digest(body)): count for kind, body, count in cases})
actual = Counter((r['kind'], r['body_sha256']) for r in executions)
assert actual == expected, (actual, expected)
for kind in ('lua', 'query'):
    group = [r for r in executions if r['kind'] == kind]
    for before, after in zip(group, group[1:]):
        assert before['delivered_at'] < after['received_at'], (before, after)

first = load('lua-write-count-first.json')['results']
second = load('lua-write-count-second.json')['results']
assert len(first) == 1 and len(second) == 2
assert {d['_key'] for d in first} < {d['_key'] for d in second}
assert all(d['trial'] == 'cg55-write' for d in second)
for name in ('lua-pending.json', 'query-pending.json', 'ownership-pending.json'):
    assert load(name)['buttons'] == [{'text': 'Running…', 'disabled': True}]
old = next(r for r in executions if r['body_sha256'] == digest(cases[4][1]))
current = [r for r in executions if r['body_sha256'] == digest(cases[-1][1])]
captured = load('ownership-pending.json')['at'] / 1000
assert old['delivered_at'] < captured < current[0]['delivered_at']
tab_check = load('query-tab-pending-check.json')
assert tab_check['before']['disabled'] and tab_check['after']['disabled']
assert current[1]['received_at'] < tab_check['before']['at'] / 1000
assert tab_check['after']['at'] / 1000 < current[1]['delivered_at']
assert load('ownership-completed.json')['buttons'] == [{'text': 'Run query', 'disabled': False}]
assert 'CG55 retained owner' in load('ownership-completed.json')['result']
assert 'CG55 recovered' in (ROOT / 'lua-recovered.txt').read_text()
code_lines = [json.loads(line.strip().removeprefix('- code: '))
              for line in (ROOT / 'query-recovered-empty.txt').read_text().splitlines()
              if line.strip().startswith('- code: ')]
assert json.loads(''.join(code_lines)) == {'count': 0, 'results': []}
assert load('console.json') == []
print(f'PASS: {len(executions)} intended executions; no overlapping same-console requests; '
      'persisted write counts, ownership and retry evidence verified.')
