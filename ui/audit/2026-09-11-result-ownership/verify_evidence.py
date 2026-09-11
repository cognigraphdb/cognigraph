"""Verify the captured CG-56 state, request ordering and persisted outcomes."""
import json
from pathlib import Path

ROOT = Path(__file__).parent


def load(name):
    return json.loads((ROOT / (name + '.json')).read_text())


for name in ['query-edited', 'query-returned-to-old-input', 'bindings-edited',
             'query-late-response-discarded', 'query-late-success-discarded',
             'lua-edited', 'lua-late-response-discarded']:
    state = load(name)
    assert state['result'] == 'Run the request to see results. Results clear when inputs change.', name
    assert state['timings'] == [], name
for name in ['query-edited-while-pending', 'query-success-bindings-edited-pending',
             'lua-edited-while-pending']:
    assert 'previous execution is still finishing' in load(name)['result'], name
    assert load(name)['timings'] == []
for name in ['graph-root-edited', 'graph-depth-edited', 'graph-confidence-edited',
             'graph-direction-edited', 'graph-collection-edited']:
    state = load(name)
    assert state['graph'].startswith('Enter a start vertex'), name
    assert state['inspectors'] == state['paths'] == 0, name
for name in ['graph-json-failed-rerun', 'graph-visual-failed-rerun', 'graph-error-still-visible']:
    state = load(name)
    assert state['graph'] == 'Failed to fetch', name
    assert state['inspectors'] == state['paths'] == 0
assert load('graph-error-still-visible')['at'] - load('graph-json-failed-rerun')['at'] > 2600
for name in ['graph-old-visual-pending', 'graph-old-json-pending']:
    assert 'Running request' in load(name)['graph']
assert 'Beta' in load('vector-new-result')['search']
assert load('vector-new-result')['search'] == load('vector-old-completion-ignored')['search']
assert 'search' not in load('vector-threshold-edited')
assert 'search' not in load('vector-invalid-rerun')
assert load('vector-invalid-rerun')['alerts'] == ['The vector must be a JSON array of numbers.']
assert 'Gamma' in load('graph-old-completion-ignored')['graph']
assert 'Alpha' not in load('graph-old-completion-ignored')['graph']
assert 'CG56_ADDED' in load('edge-created-same-json')['graph']
assert 'CG56_OTHER' in load('edge-created-other-collection')['graph']
assert load('semantic-provider-error')['alerts']
assert not load('semantic-input-clears-error')['alerts']
assert 'CG56 current Lua' in load('lua-recovered')['result']
assert 'CG56 expected Lua failure' in load('lua-failed-rerun')['result']
assert 'CG56 changed pending binding' in load('query-new-bindings-recovered')['result']

requests = load('requests')
old_vector = next(r for r in requests if r['path'] == '/api/search/vector' and r['input']['vector'] == [1, 0])
new_vector = next(r for r in requests if r['path'] == '/api/search/vector' and r['input']['vector'] == [0, 1])
old_graph = next(r for r in requests if r['path'] == '/api/graph/traverse' and r['delay_seconds'] == 20)
new_graph = next(r for r in requests if r['path'] == '/api/graph/traverse' and r['input']['start_vertex'] == 'qa_docs/gamma')
for old, new, captured in [(old_vector, new_vector, 'vector-old-completion-ignored'),
                           (old_graph, new_graph, 'graph-old-completion-ignored')]:
    assert old['received_at'] < new['received_at'] < new['delivered_at'] < old['delivered_at']
    assert load(captured)['at'] / 1000 > old['delivered_at']
    assert old['status'] == new['status'] == 200
old_query = next(r for r in requests if not r['validation'] and r['input'].get('bind_vars', {}).get('value') == 'CG56 slow bound query')
assert old_query['status'] == 200
assert load('query-late-success-discarded')['at'] / 1000 > old_query['delivered_at']
writes = [r for r in requests if r['path'] == '/api/lua/execute' and 'graph.create_document' in r['input']['script']]
assert len(writes) == 1 and writes[0]['status'] == 200
assert load('lua-late-response-discarded')['at'] / 1000 > writes[0]['delivered_at']
persisted = load('persisted-final')
assert len([d for d in persisted['qa_docs']['results'] if d.get('trial') == 'cg56-write']) == 1
assert len([e for e in persisted['qa_edges']['results'] if e.get('relation_type') == 'CG56_ADDED']) == 1
assert len([e for e in persisted['qa_other_edges']['results'] if e.get('relation_type') == 'CG56_OTHER']) == 1
assert 'CG56 slow write' in (ROOT / 'write-reloaded.txt').read_text()
assert 'CG56_OTHER' in (ROOT / 'relationship-reloaded.txt').read_text()
for geometry in load('error-geometry'):
    assert geometry['scrollWidth'] == geometry['width']
    assert 0 <= geometry['alert']['left'] < geometry['alert']['right'] <= geometry['width']
    assert 0 <= geometry['alert']['top'] < geometry['alert']['bottom'] <= geometry['height']
for name in ['console-before-reload', 'console-final', 'console-reload']:
    assert load(name) == []
print('PASS: input resets, obsolete replies, reversed real responses, shared graph errors, '
      'persisted mutations, reloads and scaled error geometry.')
