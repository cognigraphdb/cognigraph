"""Synthetic fixture for the disposable CG-56 Native server only.

Set CG56_QA_PASSWORD to its temporary administrator's password. Tokens stay in
memory. Never point this fixture at an existing application database.
"""
import json
import os
import urllib.request
from pathlib import Path

ROOT = Path(__file__).parent
BASE = 'http://127.0.0.1:38485'


def request(path, body=None, token=None):
    headers = {'Content-Type': 'application/json'}
    if token:
        headers['Authorization'] = 'Bearer ' + token
    req = urllib.request.Request(BASE + path, data=json.dumps(body).encode() if body is not None else None, headers=headers)
    with urllib.request.urlopen(req, timeout=30) as response:
        return json.load(response)


token = request('/api/auth/login', {'username': 'admin', 'password': os.environ['CG56_QA_PASSWORD']})['token']
docs = {key: {'_key': key, 'title': key.title(), 'trial': 'cg56', 'embedding': vector}
        for key, vector in [('alpha', [1, 0]), ('beta', [0, 1]), ('gamma', [-1, 0]), ('delta', [0, -1])]}
edges = {f'{source}-{target}': {'_key': f'{source}-{target}', '_from': 'qa_docs/' + source,
                              '_to': 'qa_docs/' + target, 'relation_type': 'LINK', 'confidence': 0.9}
         for source, target in [('alpha', 'beta'), ('beta', 'gamma'), ('gamma', 'delta')]}
request('/api/admin/import', {'collections': {
    'qa_docs': {'type': 'document', 'documents': docs},
    'qa_edges': {'type': 'edge', 'documents': edges},
    'qa_other_edges': {'type': 'edge', 'documents': {}},
}}, token)
result = {'health': request('/health'), 'session': request('/api/auth/session', token=token)}
for key, vector in [('alpha', [1, 0]), ('beta', [0, 1])]:
    result[key] = request('/api/search/vector', {'collection': 'qa_docs', 'vector': vector, 'limit': 10, 'threshold': 0.3}, token)
    assert result[key]['results'][0]['document']['_key'] == key
result['graph'] = request('/api/graph/traverse', {'start_vertex': 'qa_docs/alpha', 'edge_collection': 'qa_edges', 'direction': 'outbound', 'min_depth': 1, 'max_depth': 2, 'min_confidence': 0.5, 'path_decay': 0.8}, token)
(ROOT / 'seed.json').write_text(json.dumps(result, indent=2) + '\n')
print('Seeded four documents and three edges; real vector and traversal reads passed.')
