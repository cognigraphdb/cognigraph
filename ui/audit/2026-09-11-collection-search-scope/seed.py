"""Seed a disposable CG-59 Native server. CG59_QA_PASSWORD stays outside evidence."""
import json
import os
import urllib.request
from pathlib import Path

BASE = 'http://127.0.0.1:38493/api'


def request(method, path, body=None, token=None):
    headers = {'Content-Type': 'application/json'}
    if token:
        headers['Authorization'] = 'Bearer ' + token
    req = urllib.request.Request(BASE + path, method=method, headers=headers,
                                 data=None if body is None else json.dumps(body).encode())
    with urllib.request.urlopen(req, timeout=30) as response:
        return json.load(response)


token = request('POST', '/auth/login', {
    'username': 'admin', 'password': os.environ['CG59_QA_PASSWORD']})['token']
docs = {}
for i in range(125):
    key = f'item-{i:03d}'
    docs[key] = {'_id': 'qa_search/' + key, '_key': key,
                 'title': f'Constellation {i:03d}', 'content': 'Constellation sample',
                 'category': 'first-page' if i < 25 else 'later-only',
                 'embedding_status': 'missing' if i < 25 else 'ready',
                 'updatedAt': '2026-09-11T12:00:00Z'}
docs['constellation'] = {'_id': 'qa_search/constellation', '_key': 'constellation',
                          'title': 'Exact key outside text hits', 'category': 'exact-key'}
receipt = request('POST', '/admin/import', {
    'collections': {'qa_search': {'type': 'document', 'documents': docs}}}, token)
page = request('GET', '/documents?collection=qa_search&limit=25&offset=0', token=token)['results']
hits = request('POST', '/search/text', {'collection': 'qa_search', 'query': 'constellation',
               'fields': ['title', 'content', 'summary', 'text'], 'limit': 200}, token)
report = {'import': receipt, 'collection_count': len(docs), 'text_count_limit_200': len(hits['results']),
          'first_page_keys': [d['_key'] for d in page],
          'first_page_categories': sorted({d.get('category') for d in page}),
          'first_page_embedding_states': sorted({d.get('embedding_status', 'missing') for d in page})}
assert len(hits['results']) == 125
Path(__file__).with_name('api-fixture.json').write_text(json.dumps(report, indent=2) + '\n')
print(json.dumps(report, indent=2))
