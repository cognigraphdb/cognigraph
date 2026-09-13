#!/usr/bin/env python3
"""Render Helm variants; --live also exercises the CI image and backup over HTTP.

Requires Helm and Bun (YAML parsing); --live also requires Docker, cognigraph:ci
and the configured backup image. Uses disposable containers and synthetic data.
"""
import argparse
import sys
import json
import os
from pathlib import Path
import subprocess as sp
import tempfile
import time
from urllib.request import Request, urlopen
import uuid

root = Path(__file__).resolve().parents[1]
chart = root / 'deploy/helm/cognigraph'
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--live', action='store_true')
parser.add_argument('--enterprise', action='store_true', help='Exercise the Enterprise CI image')
options = parser.parse_args()

def run(args, **kwargs):
    return sp.run(args, text=True, capture_output=True, check=True, **kwargs).stdout

def inspect_backup(image, volume):
    # Inspect as the backup owner, preserving mode 0600. A Docker volume uses
    # Linux ownership semantics on both native Linux and Docker Desktop.
    args = ['docker', 'run', '--rm', '--network', 'none', '--read-only',
            '--cap-drop=ALL', '--security-opt', 'no-new-privileges',
            '-v', f'{volume}:/backups:ro']
    probe = '''import json, os, stat
from pathlib import Path
directory = Path('/backups')
files = list(directory.glob('*.json'))
assert len(files) == 1
assert not list(directory.glob('*.partial'))
path = files[0]
info = path.stat()
assert stat.S_IMODE(info.st_mode) == 0o600
assert info.st_uid == os.getuid()
print(json.dumps({'snapshot': json.loads(path.read_text()), 'bytes': info.st_size, 'filename': path.name}))
'''
    result = json.loads(run(args + ['--user', '10001:10001', image,
                                   'python3', '-I', '-c', probe]))
    denied = '''import sys
from pathlib import Path
try:
    (Path('/backups') / sys.argv[1]).read_bytes()
except PermissionError:
    print('denied')
else:
    raise AssertionError('another user could read the private backup')
'''
    assert run(args + ['--user', '10002:10002', image,
                       'python3', '-I', '-c', denied, result['filename']]).strip() == 'denied'
    return result

def render(values):
    raw = run(['helm', 'template', 'qa', str(chart), '-f', '-'], input=json.dumps(values))
    parsed = run(['bun', '-e', 'const s=await Bun.stdin.text(); console.log(JSON.stringify(s.split(/^---$/m).filter(x=>x.trim()).map(x=>Bun.YAML.parse(x))))'], input=raw)
    return json.loads(parsed)

password = 'qa-quote"-slash\\-newline\n-π'
auth = {'enabled': True, 'adminPassword': password, 'hostAdminPassword': 'synthetic-host-password', 'jwtSecret': 'synthetic-jwt-key-for-isolated-chart-verification'}
values = {'auth': auth, 'backup': {'enabled': True}, 'edition': 'enterprise' if options.enterprise else 'community',
          'image': {'tag': 'ci-enterprise' if options.enterprise else 'ci'},
          'ingress': {'enabled': True}, 'metrics': {'serviceMonitor': {'enabled': True}},
          'networkPolicy': {'enabled': True}}
objects = render(values)
state = next(o for o in objects if o['kind'] == 'StatefulSet')
job = next(o for o in objects if o['kind'] == 'CronJob')
pod = state['spec']['template']
jobpod = job['spec']['jobTemplate']['spec']['template']
serverlabels = pod['metadata']['labels']
backuplabels = jobpod['metadata']['labels']
selectors = [o['spec']['selector'] for o in objects if o['kind'] == 'Service'] + [state['spec']['selector']['matchLabels']]
for selector in selectors:
    assert all(serverlabels.get(k) == v for k,v in selector.items())
    assert not all(backuplabels.get(k) == v for k,v in selector.items())
assert len(objects) == 11
for extra in ({}, {'auth': {'enabled': False}}, {'auth': {'existingSecret': 'qa-existing'}},
              {'storage': {'mode': 'paged', 'vectorMode': 'sidecar'}},
              {'persistence': {'enabled': False}}, {'persistence': {'existingClaim': 'qa-data'}},
              {'edition': 'enterprise', 'enterprise': {'multiTenant': True, 'artifactCas': {'existingClaim': 'qa-cas'}}}):
    render({'auth': auth, **extra})
negative = [{'replicaCount': 2}, {'replicaCount': 1.5}, {'auth': {'enabled': True}},
            {'storage': {'mode': 'paged'}}, {'backup': {'enabled': True}, 'auth': {'enabled': False}},
            {'backup': {'enabled': True, 'retentionDays': 0}},
            {'podLabels': {'app.kubernetes.io/component': 'backup'}},
            {'edition': 'invalid'}, {'enterprise': {'multiTenant': True}},
            {'enterprise': {'governanceRootPublicKey': 'qa'}},
            {'enterprise': {'artifactCas': {'existingClaim': 'qa-cas'}}}]
for extra in negative:
    try: render({'auth': auth, **extra})
    except sp.CalledProcessError: pass
    else: raise AssertionError(f'accepted invalid values: {extra}')
for edition in ('community', 'enterprise'):
    rendered = render({'auth': auth, 'edition': edition})
    image = next(o for o in rendered if o['kind'] == 'StatefulSet')['spec']['template']['spec']['containers'][0]['image']
    assert image.endswith('-enterprise') == (edition == 'enterprise'), image
print('PASS: 10 Helm variants, 11 rejection cases, edition image tags, server/backup selectors', flush=True)

if not options.live:
    sys.exit(0)
workspace = tempfile.TemporaryDirectory(prefix='cognigraph-helm-')
out = Path(workspace.name)

secrets = {o['metadata']['name']: o for o in objects if o['kind'] == 'Secret'}
def envs(container):
    env = {}
    for item in container['env']:
        if 'value' in item: env[item['name']] = str(item['value'])
        else:
            key = item['valueFrom']['secretKeyRef']
            secret = secrets[key['name']]
            if 'stringData' in secret: env[item['name']] = secret['stringData'][key['key']]
            else:
                import base64
                env[item['name']] = base64.b64decode(secret['data'][key['key']]).decode()
    return env

server = pod['spec']['containers'][0]
backup = jobpod['spec']['containers'][0]
script = next(o for o in objects if o['kind'] == 'ConfigMap')['data']['backup.py']
(out / 'backup.py').write_text(script)
name = 'cognigraph-push-qa-' + uuid.uuid4().hex[:8]
container = None
network = False
backup_volume = None
def http(base, path, body=None, token=None):
    headers = {'Content-Type': 'application/json'}
    if token: headers['Authorization'] = f'Bearer {token}'
    req = Request(base+path, data=None if body is None else json.dumps(body).encode(), headers=headers)
    with urlopen(req, timeout=20) as r: return json.load(r)

try:
    run(['docker','network','create',name]); network = True
    backup_volume = run(['docker', 'volume', 'create', name + '-backups']).strip()
    # Initialize only this disposable volume, as a provisioner would do for a
    # PVC. The writer and both readers below remain non-root without capabilities.
    run(['docker', 'run', '--rm', '--network', 'none', '--read-only', '--user', '0:0',
         '--cap-drop=ALL', '--cap-add=CHOWN', '--security-opt', 'no-new-privileges',
         '-v', f'{backup_volume}:/backups', backup['image'], 'python3', '-I', '-c',
         "import os; os.chmod('/backups', 0o755); os.chown('/backups', 10001, 10001)"])
    env = envs(server)
    args = ['docker','run','-d','--name',name,'--network',name,'--network-alias','qa-cognigraph',
            '--read-only','--user','10001:10001','--cap-drop=ALL','--security-opt','no-new-privileges',
            '--tmpfs','/data:uid=10001,gid=10001,mode=0700','--tmpfs','/tmp:uid=10001,gid=10001',
            '-p','127.0.0.1::3000']
    for key in env: args += ['-e',key]
    container = run(args+[server['image']], env={**os.environ, **env}).strip()
    port = json.loads(run(['docker','inspect',container]))[0]['NetworkSettings']['Ports']['3000/tcp'][0]['HostPort']
    base = f'http://127.0.0.1:{port}'
    for _ in range(60):
        try:
            http(base,'/health/database'); break
        except Exception: time.sleep(0.5)
    else: raise RuntimeError('server did not become ready')
    assert http(base, '/health')['edition'] == values['edition']
    token = http(base,'/api/auth/login',{'username':'admin','password':password})['token']
    fixture = {'collections': {'helm_qa': {'type':'document', 'documents': {'one': {'_key':'one','text':'synthetic chart backup evidence'}}}}}
    http(base,'/api/admin/import',fixture,token)
    env = envs(backup)
    args = ['docker','run','--rm','--network',name,'--read-only','--user','10001:10001',
            '--cap-drop=ALL','--security-opt','no-new-privileges',
            '-v',f'{out}/backup.py:/scripts/backup.py:ro','-v',f'{backup_volume}:/backups']
    for key in env: args += ['-e',key]
    result = run(args+[backup['image']]+backup['command'], env={**os.environ, **env})
    inspected = inspect_backup(backup['image'], backup_volume)
    snapshot = inspected['snapshot']
    assert snapshot['collections']['helm_qa']['documents']['one']['text'] == 'synthetic chart backup evidence'
    print('PASS: rendered server env, probes, login, imported document, private backup and non-owner denial in hardened containers', flush=True)
    (out/'result.json').write_text(json.dumps({'edition':values['edition'],'helm_variants':10,'rejections':11,'objects':11,'http_backup':'passed','snapshot_bytes':inspected['bytes'],'cluster_deployment':False}, indent=2))
finally:
    if container: run(['docker','rm','-f',container])
    if network: run(['docker','network','rm',name])
    if backup_volume: run(['docker', 'volume', 'rm', backup_volume])
    workspace.cleanup()
