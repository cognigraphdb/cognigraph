"""Synthetic CG-57 fixture for the disposable Native server on port 38487.

Run with CG57_QA_PASSWORD matching its temporary admin. Tokens stay in memory.
Never run against an existing application database.
"""
import json
import os
import urllib.request
from pathlib import Path

BASE = 'http://127.0.0.1:38487/api'


def request(method, path, body=None, token=None):
    headers = {'Content-Type': 'application/json'}
    if token:
        headers['Authorization'] = 'Bearer ' + token
    req = urllib.request.Request(BASE + path, method=method, headers=headers,
                                 data=None if body is None else json.dumps(body).encode())
    with urllib.request.urlopen(req, timeout=30) as response:
        return json.load(response)


token = request('POST', '/auth/login', {
    'username': 'admin', 'password': os.environ['CG57_QA_PASSWORD']})['token']
fixture = {
    'aa_keyboard_docs': {'type': 'document', 'documents': {
        'rigel': {'title': 'Synthetic Rigel keyboard document', 'embedding': [1, 0]},
        'vega': {'title': 'Synthetic Vega keyboard document', 'embedding': [0, 1]},
    }},
    'qa_edges': {'type': 'edge', 'documents': {
        'connects': {'_from': 'aa_keyboard_docs/rigel', '_to': 'aa_keyboard_docs/vega'},
    }},
    'space_types': {'type': 'document', 'documents': {'qa-keyboard': {
        'id': 'qa-keyboard', 'name': 'Synthetic keyboard fixture',
        'entities': [{'name': 'Rigel'}, {'name': 'Vega'}],
        'relation_rules': [{'source': 'Rigel', 'relation': 'LINK', 'target': 'Vega',
                            'when_any': ['connects']}],
    }}},
    'neurons': {'type': 'document', 'documents': {'qa-neuron': {
        'id': 'qa-neuron', 'space_type': 'qa-keyboard', 'type': 'relation_hint',
        'status': 'proposed', 'confidence': 0.9, 'rationale': 'Synthetic keyboard verification',
        'evidence': ['Rigel connects Vega.'], 'source': 'Rigel', 'relation': 'LINK',
        'target': 'Vega', 'triggers': ['connects'], 'entity': '', 'aliases': [],
        'boost': 0, 'proposed_by': 'qa-fixture', 'proposed_at': 1789131600,
    }}},
}
for collection, entry in fixture.items():
    for key, doc in entry['documents'].items():
        doc.update(_key=key, _id=f'{collection}/{key}')
request('POST', '/admin/import', {'collections': fixture}, token)
request('POST', '/users', {'username': 'qa-keyboard-viewer', 'role': 'viewer',
                          'password': os.environ['CG57_QA_PASSWORD'], 'tenant': 'default'}, token)
report = {
    'collections': request('GET', '/collections', token=token),
    'document': request('GET', '/documents/aa_keyboard_docs/rigel', token=token),
    'neuron': request('GET', '/documents/neurons/qa-neuron', token=token),
    'vector': request('POST', '/search/vector', {
        'collection': 'aa_keyboard_docs', 'vector': [1, 0], 'limit': 10, 'threshold': 0.3}, token),
}
Path(__file__).with_name('seed.json').write_text(json.dumps(report, indent=2) + '\n')
print('Seeded and read back synthetic documents, edge, space, neuron and viewer account.')
