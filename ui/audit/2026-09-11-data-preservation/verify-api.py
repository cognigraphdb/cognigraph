"""Synthetic Native UI checks. Use only against an isolated, disposable QA server.
CG_QA_PASSWORD supplies the bootstrap admin password; never saved in evidence.
Run seed once, then snapshot LABEL between the browser steps in audit.md.
"""
import json
import os
import sys
import urllib.error
import urllib.request
from pathlib import Path

BASE = os.environ.get('CG_QA_ORIGIN', 'http://127.0.0.1:3001')
OUT = Path(__file__).parent
TOKEN = None


def request(method, path, body=None):
    req = urllib.request.Request(BASE + '/api' + path,
        data=None if body is None else json.dumps(body).encode(), method=method,
        headers={'Content-Type': 'application/json', **({'Authorization': 'Bearer ' + TOKEN} if TOKEN else {})})
    with urllib.request.urlopen(req, timeout=30) as response:
        return json.load(response)


def rows(collection):
    try:
        return request('GET', '/documents?collection=' + collection + '&limit=200')['results']
    except urllib.error.HTTPError as error:
        if error.code == 404:
            return []
        raise


TOKEN = request('POST', '/auth/login', {'username': 'admin', 'password': os.environ['CG_QA_PASSWORD']})['token']
action = sys.argv[1]
if action == 'seed':
    for key, content in [('object', {'note': 'keep', 'nested': [1, None]}), ('string', 'Original raw content must survive'),
                         ('array', [1, None, {'ok': True}]), ('null', None), ('number', 42), ('boolean', False)]:
        request('POST', '/documents', {'collection': 'qa_json', '_key': key, 'title': 'Before ' + key,
            'content': content, 'tags': ['one', 2, None], 'custom': {'flag': False, 'count': 3}, 'embedding': [0.1, 0.2]})
    request('POST', '/documents', {'collection': 'space_type_drafts', '_key': 'qa_import', 'id': 'qa_import', 'status': 'draft',
        'entities': [{'name': 'Nimbus', 'type': 'vendor', 'aliases': []}, {'name': 'DataCloud', 'type': 'platform', 'aliases': []}],
        'relation_rules': [{'source': 'Nimbus', 'relation': 'SUPPLIES', 'target': 'DataCloud', 'when_any': ['datacloud runs on nimbus']},
                           {'source': 'Nimbus', 'relation': 'HOSTS', 'target': 'DataCloud', 'when_any': ['nimbus hosts datacloud']}]})
    request('POST', '/construct/draft/qa_import/accept', {})
    request('POST', '/users', {'username': 'qa_viewer', 'password': os.environ['CG_QA_PASSWORD'], 'role': 'viewer'})
    label = '00-seeded'
elif action == 'intervene':
    request('POST', '/construct/ingest', {'space_type': 'qa_import', 'chunks': [{'id': 'ui-4ae737a39b4792f3c5616e91ab860a249c84e770b5cf166f56954820627939cd', 'text': 'Concurrent writer source.'}]})
    label = '09-intervening-write'
elif action == 'snapshot':
    label = sys.argv[2]
else:
    raise SystemExit('Use seed or snapshot LABEL')
result = {name: rows(name) for name in ['qa_json', 'chunks', 'facts', 'mentions']}
result['environment'] = {'origin': BASE, 'actor': 'admin', 'tenant': 'default', 'backend': 'Native', 'label': label}
(OUT / (label + '.json')).write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps({'saved': label, 'counts': {key: len(value) for key, value in result.items() if isinstance(value, list)}}))
