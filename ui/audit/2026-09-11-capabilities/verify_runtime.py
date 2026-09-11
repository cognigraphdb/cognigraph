"""CG-53 disposable Native runtime matrix; never point this at user data.

Start the documented release binaries on 38476-38479 with fresh stores. Set
CG53_QA_PASSWORD and CG53_QA_HOST_PASSWORD. This creates only synthetic fixtures.
Successful login/token responses are kept in memory, never captured.
"""
import json
import os
import urllib.error
import urllib.request
from pathlib import Path

PASSWORD = os.environ['CG53_QA_PASSWORD']
HOST_PASSWORD = os.environ['CG53_QA_HOST_PASSWORD']
OUTPUT = Path(__file__).parent
ROLES = ['admin', 'editor', 'viewer', 'script-runner', 'host-admin',
         'policy-author', 'policy-approver', 'promoter', 'artifact-attestor']
READERS = {'admin', 'editor', 'viewer', 'script-runner'}
WRITERS = {'admin', 'editor'}
GOVERNANCE = {'admin', 'policy-author', 'policy-approver', 'promoter', 'artifact-attestor'}


def request(port, method, path, body=None, token=None, expected=200):
    headers = {'Content-Type': 'application/json'}
    if token:
        headers['Authorization'] = 'Bearer ' + token
    req = urllib.request.Request(f'http://127.0.0.1:{port}/api{path}', method=method,
                                 headers=headers, data=None if body is None else json.dumps(body).encode())
    try:
        response = urllib.request.urlopen(req, timeout=20)
    except urllib.error.HTTPError as error:
        response = error
    with response:
        payload = json.load(response)
        assert response.status in (expected if isinstance(expected, tuple) else (expected,)), (port, method, path, response.status, expected,
                                             payload.get('error') if isinstance(payload, dict) else '')
        return payload


def login(port, name, password=PASSWORD):
    return request(port, 'POST', '/auth/login', {'username': name, 'password': password})['token']


def seed(port, token, enterprise):
    for key in ['one', 'two']:
        request(port, 'POST', '/documents', {'collection': 'qa_docs', '_key': key,
                'title': 'Synthetic ' + key, 'text': 'Rigel connects Vega.', 'value': 1}, token, (200, 409))
    request(port, 'POST', '/collections', {'name': 'qa_edges', 'collection_type': 'edge'}, token)
    request(port, 'POST', '/graph/relationships', {'collection': 'qa_edges', 'from': 'qa_docs/one',
            'to': 'qa_docs/two', 'relation_type': 'LINK'}, token)
    if enterprise:
        space = {'id': 'qa_space', '_key': 'qa_space', 'status': 'draft', 'name': 'Synthetic QA space',
                 'entities': [{'name': 'Rigel'}, {'name': 'Vega'}],
                 'relation_rules': [{'source': 'Rigel', 'relation': 'LINK', 'target': 'Vega',
                                     'when_any': ['connects']}]}
        request(port, 'POST', '/documents', {'collection': 'space_type_drafts', **space}, token, (200, 409))
        stored = request(port, 'GET', '/documents/space_types/qa_space', token=token, expected=(200, 404))
        if '_key' not in stored:
            request(port, 'POST', '/construct/draft/qa_space/accept', {}, token)
        request(port, 'POST', '/construct/ingest', {'space_type': 'qa_space', 'chunks': [{'id': 'qa-source', 'text': 'Rigel connects Vega.'}]}, token)
        request(port, 'POST', '/neurons', {'space_type': 'qa_space', 'id': 'qa-hint',
                'type': 'relation_hint', 'source': 'Rigel', 'relation': 'LINK', 'target': 'Vega',
                'triggers': ['links to'], 'confidence': 0.9, 'rationale': 'Synthetic UI fixture',
                'evidence': ['Rigel links to Vega.']}, token, (200, 409))
        if token:
            request(port, 'POST', '/admin/import', {'collections': {'eval_specs': {'type': 'document',
                    'documents': {'qa_space': {'_key': 'qa_space', 'space_id': 'qa_space', 'questions': []}}}}}, token)


report = {'sessions': [], 'checks': [], 'anonymous': []}
for edition, port in [('community', 38476), ('enterprise', 38477)]:
    admin = login(port, 'admin')
    for role in ROLES:
        if role not in {'admin', 'host-admin'}:
            request(port, 'POST', '/users', {'username': 'qa-' + role, 'password': PASSWORD, 'role': role}, admin, (200, 409))
    seed(port, admin, edition == 'enterprise')
    request(port, 'GET', '/auth/session', expected=401)
    request(port, 'GET', '/auth/session', token='invalid', expected=401)
    for role in ROLES:
        name = role if role in {'admin', 'host-admin'} else 'qa-' + role
        token = login(port, name, HOST_PASSWORD if role == 'host-admin' else PASSWORD)
        context = request(port, 'GET', '/auth/session', token=token)
        assert context['user']['role'] == role and context['user']['tenant'] == 'default'
        assert context['edition'] == edition and context['auth_enabled'] is True
        report['sessions'].append(context)
        cases = [
            ('GET', '/collections', None, 200 if role in READERS else 403),
            ('GET', '/documents/qa_docs/one', None, 200 if role in READERS else 403),
            ('PATCH', '/documents/qa_docs/two', {'value': role}, 200 if role in WRITERS else 403),
            ('POST', '/search/query', {'query': 'FOR d IN qa_docs LIMIT 2 RETURN d._key'}, 200 if role in READERS else 403),
            ('POST', '/graph/traverse', {'start_vertex': 'qa_docs/one', 'edge_collection': 'qa_edges'}, 200 if role in WRITERS else 403),
            ('POST', '/lua/execute', {'script': 'return 53'}, 200 if role in WRITERS or role == 'script-runner' else 403),
            ('GET', '/users', None, 200 if role == 'admin' else 403),
            ('GET', '/cache/stats', None, 200 if role == 'admin' else 403),
            ('GET', '/admin/export', None, 200 if role == 'admin' else 403),
            ('GET', '/tenants', None, (200 if edition == 'enterprise' else 403) if role == 'host-admin' else 403),
        ]
        if edition == 'enterprise':
            cases += [
                ('GET', '/neurons?space_type=qa_space', None, 200 if role in READERS else 403),
                ('POST', '/construct/advise', {'space_type': 'qa_space'}, 200 if role in READERS else 403),
                ('POST', '/construct/evaluate', {'space_type': 'qa_space'}, 200 if role in READERS else 403),
                ('GET', '/promotions/evidence', None, 200 if role in GOVERNANCE else 403),
            ]
        else:
            cases.append(('GET', '/neurons', None, 404))
        for method, path, body, expected in cases:
            result = request(port, method, path, body, token, expected)
            report['checks'].append({'edition': edition, 'role': role, 'method': method,
                                    'path': path, 'status': expected,
                                    **({'error': result['error']} if isinstance(result, dict) and 'error' in result else {})})
    # Verify real token revocation through the new endpoint.
    user = request(port, 'POST', '/users', {'username': 'qa-revoked', 'password': PASSWORD, 'role': 'viewer'}, admin)
    jwt = login(port, 'qa-revoked')
    request(port, 'DELETE', '/users/' + user['key'], token=admin)
    request(port, 'GET', '/auth/session', token=jwt, expected=401)
for edition, port in [('community', 38478), ('enterprise', 38479)]:
    context = request(port, 'GET', '/auth/session')
    assert context == {'auth_enabled': False, 'user': None, 'scopes': [], 'edition': edition}
    seed(port, None, edition == 'enterprise')
    request(port, 'POST', '/lua/execute', {'script': 'return 53'})
    # Lua mutation is denied even though direct development data writes are allowed.
    result = request(port, 'POST', '/lua/execute', {'script': 'return graph.create_document("qa_docs", { _key = "forbidden-lua" })'}, expected=403)
    request(port, 'GET', '/documents/qa_docs/forbidden-lua', expected=404)
    denied = request(port, 'GET', '/admin/export', expected=403)
    report['anonymous'].append({'edition': edition, 'context': context, 'lua_mutation': result,
                                'export_denial': denied})
(OUTPUT / 'runtime-matrix.json').write_text(json.dumps(report, indent=2) + '\n')
print(f"PASS: {len(report['sessions'])} verified role/edition sessions, {len(report['checks'])} API checks, both anonymous modes")
