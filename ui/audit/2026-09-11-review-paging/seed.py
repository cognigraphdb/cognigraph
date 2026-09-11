"""Synthetic CG-54 fixture. Only for the disposable local server on port 38481.

Run with CG54_QA_PASSWORD matching its temporary admin. This imports 101 space
fixtures and 402 neurons. Never run against an existing application database.
Login tokens stay in memory; output contains only fixture IDs and counts.
"""
import json
import os
import urllib.request
from pathlib import Path

BASE = 'http://127.0.0.1:38481/api'


def request(method, path, body=None, token=None):
    headers = {'Content-Type': 'application/json'}
    if token:
        headers['Authorization'] = 'Bearer ' + token
    req = urllib.request.Request(BASE + path, method=method, headers=headers,
                                 data=None if body is None else json.dumps(body).encode())
    with urllib.request.urlopen(req, timeout=30) as response:
        return json.load(response)


token = request('POST', '/auth/login', {'username': 'admin', 'password': os.environ['CG54_QA_PASSWORD']})['token']
spaces = {}
for i in range(101):
    key = f'qa-space-{i:03}'
    spaces[key] = {'_key': key, 'id': key, 'name': 'Synthetic paging fixture',
                   'entities': [{'name': 'Rigel'}, {'name': 'Vega'}],
                   'relation_rules': [{'source': 'Rigel', 'relation': 'LINK', 'target': 'Vega', 'when_any': ['connects']}]}
neurons = {}
for status in ['proposed', 'accepted']:
    for i in range(201):
        key = f'qa-{status}-{i:03}'
        rank = status == 'accepted' and i < 200
        neurons[key] = {'_key': key, 'id': key, 'space_type': 'qa-space-000',
                        'type': 'relation_rank_hint' if rank else 'relation_hint',
                        'status': status, 'confidence': 0.9, 'rationale': 'Synthetic paging verification',
                        'evidence': ['Rigel connects Vega.'], 'source': 'Rigel', 'relation': 'LINK', 'target': 'Vega',
                        'triggers': ['connects'], 'entity': '', 'aliases': [], 'boost': 0.1 if rank else 0,
                        'proposed_by': 'qa-fixture', 'proposed_at': 1789131600}
request('POST', '/admin/import', {'collections': {
    'space_types': {'type': 'document', 'documents': spaces},
    'neurons': {'type': 'document', 'documents': neurons},
    'chunks': {'type': 'document', 'documents': {'qa-chunk': {'_key': 'qa-chunk', 'space_id': 'qa-space-000', 'text': 'Rigel connects Vega.'}}}
}}, token)
report = {
    'spaces': len(spaces), 'proposed': 201, 'accepted': 201,
    'space_tail': request('GET', '/documents?collection=space_types&limit=100&offset=100', token=token),
    'proposed_tail': request('GET', '/neurons?space_type=qa-space-000&status=proposed&limit=200&offset=200', token=token),
    'graduation': request('GET', '/neurons/graduation?space_type=qa-space-000', token=token),
}
assert report['space_tail']['results'][0]['_key'] == 'qa-space-100'
assert report['proposed_tail']['neurons'][0]['_key'] == 'qa-proposed-200'
assert any(c['neuron_id'] == 'qa-accepted-200' for c in report['graduation']['candidates'])
Path(__file__).with_name('seed.json').write_text(json.dumps(report, indent=2) + '\n')
print('Seeded and verified 101 spaces, 201 proposed and 201 accepted neurons; one off-page graduation flag.')
