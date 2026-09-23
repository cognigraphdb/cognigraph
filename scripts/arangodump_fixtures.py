#!/usr/bin/env python3
"""Regenerate the CG-64 ArangoDB dump fixtures (requires Docker; not run in CI).

Loads the synthetic datasets from arangodump_dataset.py into disposable
ArangoDB containers over HTTP (never through arangosh, whose option parser
expands `@name@` as environment variables), runs arangodump with each option
variant, derives corrupted variants by recorded mutations and writes
fixtures/arangodump/manifest.json with image digests, exact options, expected
outcomes and SHA-256 hashes. Regeneration produces new revision ids and ticks,
so file hashes change; expected outcomes must not.
"""
import argparse
import base64
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import time
from urllib.error import HTTPError
from urllib.request import Request, urlopen

sys.path.insert(0, str(Path(__file__).resolve().parent))
from arangodump_dataset import BROKEN_ERRORS, DATASETS, expected  # noqa: E402

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / 'fixtures/arangodump'
VERSIONS = {
    # Last Apache-2.0 community line.
    '3.11': 'arangodb:3.11.14',
    # ArangoDB no longer ships 3.12 community images; the Enterprise image
    # runs unlicensed in evaluation mode, which is enough to produce dumps.
    '3.12': 'arangodb/enterprise:3.12.11',
}
ACCEPT = {'outcome': 'accepted'}
PLAIN = ['--compress-output', 'false']
BROKEN = {'outcome': 'rejected', 'errors': BROKEN_ERRORS}


def reject(code):
    return {'outcome': 'rejected', 'errors': [{'code': code}]}


# (variant, dataset or None for every database, arangodump options, expectation)
DUMPS = {
    '3.11': [
        ('shop-gzip', 'shop', [], ACCEPT),
        ('shop-plain', 'shop', PLAIN, ACCEPT),
        ('shop-envelope', 'shop', ['--envelope', 'true', *PLAIN], ACCEPT),
        ('shop-system', 'shop', ['--include-system-collections', 'true'], ACCEPT),
        ('broken-plain', 'broken', PLAIN, BROKEN),
        # 3.11 writes empty data files, identical to a dump of empty collections.
        ('structure-only', 'shop', ['--dump-data', 'false'],
         {'outcome': 'accepted', 'empty': True, 'warnings': ['no_documents']}),
        ('all-databases', None, ['--all-databases', 'true'], reject('multiple_databases')),
    ],
    '3.12': [
        ('shop-gzip', 'shop', [], ACCEPT),
        ('shop-plain', 'shop', PLAIN, ACCEPT),
        ('shop-split', 'shop', ['--parallel-dump', 'true', '--split-files', 'true'], ACCEPT),
        ('shop-system', 'shop', ['--include-system-collections', 'true'], ACCEPT),
        ('broken-plain', 'broken', PLAIN, BROKEN),
        ('all-databases', None, ['--all-databases', 'true'], reject('multiple_databases')),
        ('vpack', 'shop', ['--parallel-dump', 'true', '--dump-vpack', 'true'], reject('vpack_unsupported')),
        ('encrypted', 'shop', ['--encryption.keyfile', '/tmp/fixture.key'], reject('encrypted_unsupported')),
    ],
}


def data_file(directory, collection):
    matches = sorted(directory.glob(f'{collection}_*.data.json*'))
    if len(matches) != 1:
        raise RuntimeError(f'{directory}: expected one data file for {collection}, found {matches}')
    return matches[0]


def rewrite_lines(path, change):
    lines = path.read_text().splitlines()
    path.write_text('\n'.join(change(lines)) + '\n')


def truncate_gzip(directory):
    path = data_file(directory, 'customers')
    path.write_bytes(path.read_bytes()[: len(path.read_bytes()) // 2])


def append_invalid_line(directory):
    rewrite_lines(data_file(directory, 'customers'), lambda lines: [*lines, '{"_key": "c-9", '])


def duplicate_key(directory):
    rewrite_lines(data_file(directory, 'customers'), lambda lines: [*lines, lines[0]])


def mismatch_id(directory):
    def change(lines):
        first = json.loads(lines[0])
        first['_id'] = 'products/' + first['_key']
        return [json.dumps(first, ensure_ascii=False), *lines[1:]]
    rewrite_lines(data_file(directory, 'customers'), change)


def violate_unique(directory):
    def change(lines):
        docs = [json.loads(line) for line in lines]
        for doc in docs:
            if doc['_key'] == 'p2':
                doc['region'] = 'eu'  # p1 already holds (A-1, eu)
        return [json.dumps(doc, ensure_ascii=False) for doc in docs]
    rewrite_lines(data_file(directory, 'products'), change)


def remove_marker(directory):
    def change(lines):
        return [*lines, json.dumps({'type': 2302, 'data': {'_key': 'c-1', '_rev': '_x'}})]
    rewrite_lines(data_file(directory, 'customers'), change)


# Exporter behavior seen while producing these fixtures; the contract relies on it.
OBSERVATIONS = [
    '3.11 and 3.12 default to gzip JSON without envelope; every dump writes an ENCRYPTION file '
    'containing `none` unless encrypted, then `aes-256-ctr` and even dump.json is ciphertext.',
    '3.12 removed --envelope; 3.11 --envelope wraps each line as {"type":2300,"data":{...}}.',
    '3.12 --split-files names data files <name>_<md5>.<n>.data.json[.gz] and writes no data '
    'file for an empty collection; non-split dumps always write one, empty or not.',
    '3.12 --dump-vpack writes <name>_<md5>.data.vpack.gz and sets useVPack true in dump.json.',
    '3.12.11 --dump-data false still writes full data files; 3.11 writes empty ones, so a '
    'structure-only dump cannot be told apart from a dump of empty collections.',
    'Views are written as <name>.view.json; named graphs live only in the _graphs system '
    'collection, dumped only with --include-system-collections.',
    '`hash` indexes keep type `hash` in structure files; inverted index fields are objects.',
    '--all-databases writes one subdirectory per database, each with its own dump.json.',
]

# (variant, source "<version>/<variant>", mutation, description, expectation)
DERIVED = [
    ('truncated-gzip', '3.12/shop-gzip', truncate_gzip,
     'customers data file cut to half its compressed length', reject('corrupt_data_file')),
    ('invalid-json-line', '3.12/shop-plain', append_invalid_line,
     'customers data file gains an unterminated JSON object line', reject('corrupt_data_file')),
    ('duplicate-key', '3.12/shop-plain', duplicate_key,
     'first customers line appended again', reject('duplicate_key')),
    ('id-mismatch', '3.12/shop-plain', mismatch_id,
     'first customers `_id` names another collection', reject('identity_mismatch')),
    ('unique-violation', '3.12/shop-plain', violate_unique,
     'p2 region set to eu so (sku, region) repeats', reject('unique_violation')),
    ('missing-data-file', '3.12/shop-plain',
     lambda d: data_file(d, 'products').unlink(),
     'products data file deleted', reject('missing_data_file')),
    ('missing-dump-json', '3.12/shop-plain', lambda d: (d / 'dump.json').unlink(),
     'dump.json deleted', reject('missing_dump_metadata')),
    ('unknown-file', '3.12/shop-plain', lambda d: (d / 'notes.txt').write_text('synthetic\n'),
     'unrecognized notes.txt added', reject('unknown_layout')),
    ('dotfile', '3.12/shop-plain', lambda d: (d / '.DS_Store').write_bytes(b'\0synthetic'),
     '.DS_Store added (ignored and reported)', {'outcome': 'accepted', 'ignored': ['.DS_Store']}),
    ('envelope-remove-marker', '3.11/shop-envelope', remove_marker,
     'a type 2302 removal marker appended to customers', reject('unsupported_marker')),
]


def http(base, path, body=None, method=None):
    request = Request(base + path, data=None if body is None else json.dumps(body).encode(),
                      method=method or ('POST' if body is not None else 'GET'),
                      headers={'Content-Type': 'application/json',
                               'Authorization': 'Basic ' + base64.b64encode(b'root:').decode()})
    try:
        with urlopen(request, timeout=30) as response:
            return json.loads(response.read() or b'null')
    except HTTPError as error:
        raise RuntimeError(f'{method or "POST"} {path}: {error.code} {error.read().decode()}') from error


def load(base, dataset):
    http(base, '/_api/database', {'name': dataset['database']})
    db = f'/_db/{dataset["database"]}'
    for collection in dataset['collections']:
        spec = {'name': collection['name'], 'type': collection['type']}
        if 'schema' in collection:
            spec['schema'] = collection['schema']
        http(base, f'{db}/_api/collection', spec)
        for definition, _ in collection['indexes']:
            http(base, f'{db}/_api/index?collection={collection["name"]}', definition)
        if collection['documents']:
            result = http(base, f'{db}/_api/document/{collection["name"]}', collection['documents'])
            failed = [item for item in result if item.get('error')]
            if failed:
                raise RuntimeError(f'{collection["name"]}: {failed}')
    for view in dataset['views']:
        http(base, f'{db}/_api/view', {'name': view['name'], 'type': view['type'],
                                        'links': view['links']})
    for graph in dataset['graphs']:
        http(base, f'{db}/_api/gharial', graph)


def container(image):
    name = f'cg64-fixtures-{hashlib.sha256(image.encode()).hexdigest()[:8]}'
    subprocess.run(['docker', 'rm', '-f', name], capture_output=True)
    subprocess.run(['docker', 'run', '-d', '--name', name, '-e', 'ARANGO_NO_AUTH=1',
                    '-p', '127.0.0.1::8529', image], check=True, capture_output=True)
    port = subprocess.check_output(['docker', 'port', name, '8529/tcp'], text=True).split(':')[-1].strip()
    base = f'http://127.0.0.1:{port}'
    for _ in range(120):
        try:
            http(base, '/_api/version')
            return name, base
        except (OSError, RuntimeError):
            time.sleep(1)
    raise RuntimeError(f'{image} did not become ready')


def run_dump(name, target, dataset, options):
    command = ['docker', 'exec', name, 'arangodump', '--server.endpoint', 'tcp://127.0.0.1:8529',
               '--server.password', '', '--output-directory', '/tmp/out', '--overwrite', 'true',
               '--threads', '1', *options]
    if dataset:
        command += ['--server.database', DATASETS[dataset]['database']]
    subprocess.run(['docker', 'exec', name, 'rm', '-rf', '/tmp/out'], check=True)
    subprocess.run(command, check=True, capture_output=True)
    subprocess.run(['docker', 'cp', '-q', f'{name}:/tmp/out', str(target)], check=True)


def files(directory):
    return [{'path': path.relative_to(directory).as_posix(), 'bytes': path.stat().st_size,
             'sha256': hashlib.sha256(path.read_bytes()).hexdigest()}
            for path in sorted(p for p in directory.rglob('*') if p.is_file())]


def observations(directory):
    metadata = directory / 'dump.json'
    if not metadata.exists():
        return {}
    try:
        data = json.loads(metadata.read_text())
    except (UnicodeDecodeError, json.JSONDecodeError):
        return {'dump_json': 'unreadable'}
    return {key: data.get(key) for key in ('database', 'useEnvelope', 'useVPack')}


def generate():
    if OUTPUT.exists():
        shutil.rmtree(OUTPUT)
    manifest = {'schema': 'cognigraph-arangodump-fixtures-v1', 'issue': 'CG-64', 'sources': {},
                'observations': OBSERVATIONS, 'fixtures': {}}
    for version, image in VERSIONS.items():
        digest = subprocess.check_output(['docker', 'image', 'inspect', '-f', '{{index .RepoDigests 0}}',
                                          image], text=True).strip()
        name, base = container(image)
        try:
            for dataset in DATASETS.values():
                load(base, dataset)
            subprocess.run(['docker', 'exec', name, 'sh', '-c',
                            'head -c 32 /dev/urandom > /tmp/fixture.key'], check=True)
            tool = subprocess.check_output(['docker', 'exec', name, 'arangodump', '--version'],
                                           text=True).splitlines()[0].strip()
            manifest['sources'][version] = {'image': image, 'digest': digest, 'arangodump': tool,
                                            'server': http(base, '/_api/version')['version']}
            for variant, dataset, options, expectation in DUMPS[version]:
                target = OUTPUT / version / variant
                target.parent.mkdir(parents=True, exist_ok=True)
                run_dump(name, target, dataset, options)
                manifest['fixtures'][f'{version}/{variant}'] = {
                    'source': version, 'dataset': dataset, 'arangodump_options': options,
                    'observed': observations(target), 'expected': expectation}
                print(f'DUMP {version}/{variant}', flush=True)
        finally:
            subprocess.run(['docker', 'rm', '-f', name], capture_output=True)
    for variant, source, mutate, description, expectation in DERIVED:
        target = OUTPUT / 'derived' / variant
        shutil.copytree(OUTPUT / source, target)
        mutate(target)
        origin = manifest['fixtures'][source]
        manifest['fixtures'][f'derived/{variant}'] = {
            'source': origin['source'], 'dataset': origin['dataset'], 'derived_from': source,
            'mutation': description, 'observed': observations(target), 'expected': expectation}
        print(f'DERIVE derived/{variant}', flush=True)
    for key, entry in manifest['fixtures'].items():
        entry['files'] = files(OUTPUT / key)
    (OUTPUT / 'expected').mkdir()
    for name, dataset in DATASETS.items():
        (OUTPUT / 'expected' / f'{name}.json').write_text(
            json.dumps(expected(dataset), indent=2, ensure_ascii=False) + '\n')
    (OUTPUT / 'manifest.json').write_text(json.dumps(manifest, indent=2, ensure_ascii=False) + '\n')
    print(f'PASS: {len(manifest["fixtures"])} fixtures under {OUTPUT.relative_to(ROOT)}', flush=True)


if __name__ == '__main__':
    argparse.ArgumentParser(description=__doc__).parse_args()
    try:
        generate()
    except (OSError, RuntimeError, subprocess.CalledProcessError) as error:
        print(f'FAIL: {error}', file=sys.stderr)
        raise SystemExit(1)
