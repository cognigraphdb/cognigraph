#!/usr/bin/env python3
"""CG-66 acceptance: stores built by `cognigraph import --from-arangodump`
serve the exact source data through a real server, in every persistent
storage mode, before and after a restart.

Imports the 3.12 gzip and 3.11 envelope `shop` fixtures with the given CLI,
then starts the given server on copies of each store with fresh credentials
(the dump carries none) and checks documents with every key punctuation
class, integer and float precision, collection types including empty ones,
CGQL reads, traversal over both edge collections, carried unique
constraints (enforced and listed) and the absence of stamped fields.

    python3 docs/verification/harnesses/arangodump-import-http.py \\
        --bin DIR_WITH_cognigraph_AND_cognigraph-server --edition community
"""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
from urllib.parse import quote

REPO = Path(__file__).resolve().parents[3]
spec = importlib.util.spec_from_file_location('edition_probe', REPO / 'scripts/check-editions-live.py')
probe = importlib.util.module_from_spec(spec)
spec.loader.exec_module(probe)
probe.ENV = {key: os.environ[key] for key in ('PATH', 'HOME', 'TMPDIR') if key in os.environ}
FIXTURES = REPO / 'fixtures/arangodump'
SOURCES = ('3.12/shop-gzip', '3.11/shop-envelope')
MODES = ('resident-embedded', 'resident-sidecar', 'paged-sidecar')
STAMPED = ('created_at', 'updated_at', 'relation_type', 'confidence')


def expected():
    return json.loads((FIXTURES / 'expected/shop.json').read_text())


def import_store(cli, source, directory):
    store = directory / 'imported.redb'
    result = subprocess.run([str(cli), 'import', '--from-arangodump', str(FIXTURES / source),
                             '--output', str(store)], env=probe.ENV, capture_output=True, text=True, timeout=120)
    assert result.returncode == 0, (source, result.stderr)
    report = json.loads(result.stdout)
    assert report['status'] == 'published', report
    return store, report


def check(server, store, edition, mode, directory):
    oracle = expected()
    storage, vector = mode.split('-')
    config = {'COGNIGRAPH_NATIVE_PATH': str(store), 'COGNIGRAPH_STORAGE_MODE': storage,
              'COGNIGRAPH_VECTOR_MODE': vector, 'COGNIGRAPH_CACHE_BYTES': '4096',
              'COGNIGRAPH_EMBEDDING_PROVIDER': 'none'}
    checks = 0

    def expect(condition, label):
        nonlocal checks
        assert condition, (edition, mode, label)
        checks += 1

    for phase in ('first start', 'after restart'):
        with probe.server(server, directory, config) as base:
            expect(probe.http(base, '/health')['edition'] == edition, f'{phase}: edition')
            token = probe.login(base)
            listed = {c['name']: c for c in probe.http(base, '/api/collections', token=token)['collections']}
            for name, collection in oracle['collections'].items():
                expect(name in listed, f'{phase}: collection {name} exists')
                rows = probe.http(base, '/api/search/query', {'query': f'FOR d IN {name} RETURN d'}, token)['results']
                expect(len(rows) == len(collection['documents']), f'{phase}: {name} count')
                for key, document in collection['documents'].items():
                    stored = probe.http(base, f'/api/documents/{name}/{quote(key, safe="")}', token=token)
                    expect(stored.pop('_id') == f'{name}/{key}', f'{phase}: {name}/{key} id')
                    expect(stored == document, f'{phase}: {name}/{key} exact body')
                    expect(not any(field in stored for field in STAMPED if field not in document),
                           f'{phase}: {name}/{key} not stamped')
            numbers = probe.http(base, f'/api/documents/customers/{quote("c@3", safe="")}', token=token)['numbers']
            expect(numbers['big'] == 9007199254740993 and numbers['huge'] == 1e300, f'{phase}: number precision')
            hop = 'FOR v IN 1..1 OUTBOUND @start {edges} RETURN v._key'
            orders = probe.http(base, '/api/search/query', {'query': hop.format(edges='orders'),
                                                            'bind_vars': {'start': 'customers/c-1'}}, token)['results']
            expect(orders == ['p1'], f'{phase}: traversal over orders')
            chain = probe.http(base, '/api/search/query', {
                'query': 'FOR v IN 1..2 OUTBOUND @start referrals RETURN v._key',
                'bind_vars': {'start': 'customers/c-1'}}, token)['results']
            expect(sorted(chain) == ['c.4', 'c@3'], f'{phase}: two-hop traversal over referrals')
            indexes = probe.http(base, '/api/collections/customers/indexes', token=token)['indexes']
            expect({tuple(i['fields']) for i in indexes if i['unique']} == {('email',), ('loyalty.id',)},
                   f'{phase}: carried constraints listed')
            duplicate = probe.http(base, '/api/documents', {'collection': 'customers', '_key': f'dup-{phase[0]}',
                                   'email': 'ana@example.test'}, token, status=409)
            expect(duplicate['code'] == 'unique_violation', f'{phase}: carried constraint enforced')
            compound = probe.http(base, '/api/documents', {'collection': 'products', '_key': f'dup-{phase[0]}',
                                  'sku': 'A-1', 'region': 'eu'}, token, status=409)
            expect(compound['code'] == 'unique_violation', f'{phase}: compound constraint enforced')
            probe.http(base, '/api/documents', {'collection': 'customers', '_key': f'new-{phase[0]}',
                       'email': f'new-{phase[0]}@example.test'}, token)
            expect(True, f'{phase}: sparse constraint admits a document without loyalty.id')
            probe.http(base, f'/api/documents/customers/new-{phase[0]}', token=token, method='DELETE')
    return checks


def run(bin_dir, edition):
    cli, server = bin_dir / 'cognigraph', bin_dir / 'cognigraph-server'
    results = []
    with tempfile.TemporaryDirectory(prefix='cg66-live-') as temporary:
        root = Path(temporary)
        for source in SOURCES:
            target = root / source.replace('/', '-')
            target.mkdir()
            imported, report = import_store(cli, source, target)
            for mode in MODES:
                directory = root / f'{source.replace("/", "-")}-{mode}'
                directory.mkdir()
                store = directory / 'store.redb'
                shutil.copy2(imported, store)
                checks = check(server, store, edition, mode, directory)
                results.append({'source': source, 'mode': mode, 'checks': checks,
                                'constraints': len(report['constraints']), 'not_carried': len(report['not_carried'])})
                print(f'PASS {edition} {source} {mode}: {checks} checks', flush=True)
    return results


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--bin', type=Path, required=True)
    parser.add_argument('--edition', choices=('community', 'enterprise'), required=True)
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    results = run(args.bin.resolve(), args.edition)
    summary = {'issue': 'CG-66', 'edition': args.edition, 'runs': results,
               'checks': sum(r['checks'] for r in results)}
    if args.output:
        args.output.write_text(json.dumps(summary, indent=2) + '\n')
    print(f'PASS {args.edition}: {summary["checks"]} checks over {len(results)} store/mode runs', flush=True)
