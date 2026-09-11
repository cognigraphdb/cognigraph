"""CG-58 fixture for a fresh, disposable Native server on 127.0.0.1:38489.

Set CG58_QA_PASSWORD to its temporary administrator password. This verifies the
managed mutation guard and creates an inert draft through the documents API.
The browser acceptance step is intentionally separate. Tokens stay in memory.
"""
import json
import os
import urllib.error
import urllib.request
from pathlib import Path

BASE = 'http://127.0.0.1:38489/api'


def request(method, path, body=None, token=None, expected=200):
    headers = {'Content-Type': 'application/json'}
    if token:
        headers['Authorization'] = 'Bearer ' + token
    req = urllib.request.Request(BASE + path, method=method, headers=headers,
                                 data=None if body is None else json.dumps(body).encode())
    try:
        response = urllib.request.urlopen(req, timeout=20)
    except urllib.error.HTTPError as error:
        response = error
    with response:
        payload = json.load(response)
        assert response.status == expected, (method, path, response.status, expected)
        return payload


token = request('POST', '/auth/login', {
    'username': 'admin', 'password': os.environ['CG58_QA_PASSWORD']})['token']
space = {'id': 'qa-guidance', '_key': 'qa-guidance', 'status': 'draft',
         'entities': [{'name': 'Rigel'}, {'name': 'Vega'}],
         'relation_rules': [{'source': 'Rigel', 'relation': 'LINK', 'target': 'Vega',
                             'when_any': ['connects']}]}
report = {
    'initial_catalog': request('GET', '/collections', token=token),
    'initial_space_list': request('GET', '/documents?collection=space_types', token=token, expected=404),
    'managed_write_denied': request('POST', '/documents', {'collection': 'space_types', **space}, token, expected=403),
    'inert_draft': request('POST', '/documents', {'collection': 'space_type_drafts', **space}, token),
    'accepted_space_still_absent': request('GET', '/documents/space_types/qa-guidance', token=token, expected=404),
}
request('POST', '/users', {'username': 'qa-viewer', 'role': 'viewer',
                          'password': os.environ['CG58_QA_PASSWORD']}, token)
Path(__file__).with_name('seed.json').write_text(json.dumps(report, indent=2) + '\n')
print('PASS: fresh catalog, managed write denied 403, inert draft created; no accepted space yet.')
