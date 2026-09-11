"""Add canonical _id properties to the synthetic import before graph QA.

The initial vector capture used imported _key-only documents. Graph rendering
requires full vertex IDs; retain every existing fixture/write while adding them.
"""
import json
import os
import urllib.request
from pathlib import Path

BASE = 'http://127.0.0.1:38485/api'
ROOT = Path(__file__).parent


def request(path, body=None, token=None):
    headers = {'Content-Type': 'application/json'}
    if token:
        headers['Authorization'] = 'Bearer ' + token
    req = urllib.request.Request(BASE + path, data=json.dumps(body).encode() if body is not None else None, headers=headers)
    with urllib.request.urlopen(req, timeout=30) as response:
        return json.load(response)


token = request('/auth/login', {'username': 'admin', 'password': os.environ['CG56_QA_PASSWORD']})['token']
collections = {}
for name, kind in [('qa_docs', 'document'), ('qa_edges', 'edge')]:
    docs = request('/documents?collection=' + name + '&limit=100', token=token)['results']
    collections[name] = {'type': kind, 'documents': {
        d['_key']: {**d, '_id': name + '/' + d['_key']} for d in docs}}
request('/admin/import', {'collections': collections}, token)
(ROOT / 'fixture-with-ids.json').write_text(json.dumps(collections, indent=2) + '\n')
print('Added canonical IDs to the fixture; preserved all documents and the Lua write.')
