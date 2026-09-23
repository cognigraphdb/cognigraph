"""CG-64: the dump import contract's reference reader and fixture validator
reject every input shape outside the contract, and detect fixture tampering."""
import importlib.util
import json
from pathlib import Path
import shutil
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts'))
import arangodump_reader as reader  # noqa: E402

spec = importlib.util.spec_from_file_location('fixture_check', ROOT / 'scripts/check-arangodump-fixtures.py')
check = importlib.util.module_from_spec(spec)
spec.loader.exec_module(check)
FIXTURES = ROOT / 'fixtures/arangodump'


class Copy(unittest.TestCase):
    source = '3.12/shop-plain'

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.dump = Path(self.temp.name) / 'dump'
        shutil.copytree(FIXTURES / self.source, self.dump)

    def file(self, collection, kind='data'):
        return next(self.dump.glob(f'{collection}_*.{kind}.json*'))

    def lines(self, collection):
        return self.file(collection).read_text().splitlines()

    def write(self, collection, lines):
        self.file(collection).write_text('\n'.join(lines) + '\n')

    def codes(self, **limits):
        report = reader.read(self.dump, limits)
        return report['status'], [e['code'] for e in report['errors']]


class Names(unittest.TestCase):
    def test_reserved_names_come_from_the_server_sources(self):
        words, owned = reader.reserved_names()
        self.assertTrue({'count', 'for', 'return', 'filter'} <= words)
        self.assertTrue({'facts', 'entities', 'neurons', 'side_views', 'construction_refusals'} <= owned)

    def test_names_must_be_cgql_identifiers_that_the_server_does_not_own(self):
        reserved = reader.reserved_names()
        for name, code in [('order-lines', 'incompatible_collection_name'), ('9lives', 'incompatible_collection_name'),
                           ('naïve', 'incompatible_collection_name'), ('COUNT', 'reserved_collection_name'),
                           ('Return', 'reserved_collection_name'), ('chunks', 'reserved_collection_name')]:
            self.assertEqual(reader.check_name(name, reserved)['code'], code, name)
        for name in ('Customers', 'orders_2026', 'a'):
            self.assertIsNone(reader.check_name(name, reserved), name)


class UniqueValues(unittest.TestCase):
    def test_sparse_absent_and_null_exempt_while_non_sparse_treats_them_as_null(self):
        self.assertIsNone(reader.value_key({'a': 1}, ['b'], True))
        self.assertIsNone(reader.value_key({'a': 1, 'b': None}, ['a', 'b'], True))
        self.assertEqual(reader.value_key({'a': 1}, ['b'], False), '[null]')
        # A dotted path through a non-object is absent, never an error.
        self.assertEqual(reader.value_key({'a': 'x'}, ['a.b'], False), '[null]')
        self.assertEqual(reader.value_key({'a': {'y': 1, 'x': 2}}, ['a'], False),
                         reader.value_key({'a': {'x': 2, 'y': 1}}, ['a'], False))
        self.assertNotEqual(reader.value_key({'a': 1}, ['a'], False), reader.value_key({'a': 1.5}, ['a'], False))


class Layout(Copy):
    def test_symlinks_and_unknown_directories_are_refused_before_reading(self):
        (self.dump / 'link.data.json').symlink_to(self.file('customers'))
        self.assertEqual(self.codes(), ('rejected', ['unsafe_path']))
        (self.dump / 'link.data.json').unlink()
        (self.dump / 'extra').mkdir()
        self.assertEqual(self.codes(), ('rejected', ['unknown_layout']))

    def test_data_without_structure_and_corrupt_structure_are_refused(self):
        stray = self.dump / ('ghost_' + '0' * 32 + '.data.json')
        stray.write_text('')
        self.assertEqual(self.codes(), ('rejected', ['unknown_layout']))
        stray.unlink()
        self.file('customers', 'structure').write_text('{"parameters": ')
        status, codes = self.codes()
        self.assertEqual((status, codes[0]), ('rejected', 'corrupt_structure_file'))

    def test_corrupt_metadata_and_non_none_encryption_marker(self):
        (self.dump / 'dump.json').write_text('{')
        self.assertEqual(self.codes(), ('rejected', ['corrupt_dump_metadata']))
        (self.dump / 'ENCRYPTION').write_text('aes-256-ctr')
        self.assertEqual(self.codes(), ('rejected', ['encrypted_unsupported']))

    def test_vpack_is_detected_by_file_name_even_when_metadata_omits_the_flag(self):
        self.file('customers').rename(self.dump / self.file('customers').name.replace('.json', '.vpack'))
        self.assertEqual(self.codes(), ('rejected', ['vpack_unsupported']))

    def test_every_collection_missing_its_data_is_refused_outside_split_mode(self):
        for path in self.dump.glob('*.data.json'):
            path.unlink()
        status, codes = self.codes()
        self.assertEqual(status, 'rejected')
        self.assertEqual(set(codes), {'missing_data_file'})


class Split(Copy):
    source = '3.12/shop-split'

    def test_split_dumps_omit_empty_collections_and_still_import_them(self):
        report = reader.read(self.dump)
        self.assertEqual(report['status'], 'accepted')
        self.assertEqual(report['collections']['empty_docs'], {'type': 'document', 'documents': {}})
        self.assertEqual(report['collections']['empty_edges']['type'], 'edge')

    def test_parts_of_one_collection_are_read_together_and_share_duplicate_detection(self):
        first = self.file('customers')
        second = self.dump / first.name.replace('.0.data', '.1.data')
        shutil.copy(first, second)
        report = reader.read(self.dump)
        self.assertEqual([e['code'] for e in report['errors']], ['duplicate_key'])


class Records(Copy):
    def test_record_and_expansion_limits(self):
        self.assertEqual(self.codes(max_record_bytes=64)[1][:1], ['record_too_large'])
        status, codes = self.codes(max_expanded_bytes=100)
        self.assertEqual((status, codes[0]), ('rejected', 'dump_too_large'))
        self.assertEqual(self.codes(max_files=3), ('rejected', ['dump_too_large']))

    def test_documents_need_string_keys_and_edges_need_both_ends(self):
        lines = self.lines('customers')
        self.write('customers', [*lines, json.dumps({'_key': ''})])
        self.assertIn('invalid_document', self.codes()[1])
        self.write('customers', lines)
        edges = self.lines('orders')
        first = json.loads(edges[0])
        del first['_to']
        self.write('orders', [json.dumps(first), *edges[1:]])
        self.assertIn('invalid_edge', self.codes()[1])

    def test_blank_lines_are_ignored_and_rev_is_dropped_but_counted(self):
        self.write('customers', ['', *self.lines('customers'), '   '])
        report = reader.read(self.dump)
        self.assertEqual(report['status'], 'accepted')
        self.assertEqual(report['dropped']['_rev'], 14)  # 5 + 4 + 3 + 2 documents
        self.assertTrue(all('_rev' not in d and '_id' not in d
                            for c in report['collections'].values() for d in c['documents'].values()))

    def test_within_a_collection_the_first_problem_in_file_order_wins(self):
        lines = self.lines('customers')
        clash = json.loads(lines[1])
        clash['_key'], clash['_id'] = 'c-late', 'customers/c-late'
        clash['email'] = json.loads(lines[0])['email']
        # A unique violation before a corrupt line is reported, not the corruption...
        self.write('customers', [*lines, json.dumps(clash), '{not json'])
        self.assertEqual(self.codes()[1], ['unique_violation'])
        # ...and a corrupt line before the violation is reported instead.
        self.write('customers', [*lines, '{not json', json.dumps(clash)])
        self.assertEqual(self.codes()[1], ['corrupt_data_file'])

    def test_an_edge_to_an_absent_collection_is_unresolved(self):
        edges = self.lines('referrals')
        first = json.loads(edges[0])
        first['_to'] = 'vendors/v1'
        self.write('referrals', [json.dumps(first), *edges[1:]])
        report = reader.read(self.dump)
        self.assertEqual(report['errors'], [{'code': 'unresolved_edge', 'collection': 'referrals', 'key': 'r1'}])


class Indexes(Copy):
    def rewrite_indexes(self, indexes):
        path = self.file('customers', 'structure')
        structure = json.loads(path.read_text())
        structure['indexes'] = indexes
        path.write_text(json.dumps(structure))

    def test_array_expansion_skiplist_and_non_unique_dispositions(self):
        self.rewrite_indexes([
            {'type': 'primary', 'fields': ['_key'], 'unique': True},
            {'type': 'persistent', 'fields': ['tags[*]'], 'unique': True, 'name': 'tags'},
            {'type': 'skiplist', 'fields': ['email'], 'unique': True, 'name': 'legacy'},
            {'type': 'fulltext', 'fields': ['name'], 'name': 'ft'},
            {'type': 'zkd', 'fields': ['score'], 'name': 'mdi'},
        ])
        report = reader.read(self.dump)
        customers = [c for c in report['constraints'] if c['collection'] == 'customers']
        self.assertEqual(customers, [{'collection': 'customers', 'unique': True, 'fields': ['email'],
                                      'sparse': False}])
        reasons = {n['detail']: n['reason'] for n in report['not_carried'] if n['collection'] == 'customers'}
        self.assertEqual(reasons, {'tags': 'unsupported_index_type', 'ft': 'unsupported_index_type',
                                   'mdi': 'unsupported_index_type'})


class Validator(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name) / 'fixtures'
        shutil.copytree(FIXTURES, self.root)

    def test_pristine_copy_passes(self):
        self.assertEqual(check.check(self.root)['errors'], [])

    def test_tampered_extra_and_missing_files_are_detected(self):
        target = next((self.root / '3.11/shop-plain').glob('customers_*.data.json'))
        target.write_text(target.read_text().replace('Lisboa', 'Porto'))
        (self.root / '3.12/shop-gzip/extra.txt').write_text('x')
        next((self.root / '3.12/shop-split').glob('orders_*.structure.json')).unlink()
        errors = '\n'.join(check.check(self.root)['errors'])
        self.assertIn('3.11/shop-plain/customers_', errors)
        self.assertIn('3.11/shop-plain: collections or documents differ', errors)
        self.assertIn('extra.txt', errors)
        self.assertIn('3.12/shop-split: files differ', errors)

    def test_expected_files_must_equal_the_dataset_oracle(self):
        path = self.root / 'expected/shop.json'
        data = json.loads(path.read_text())
        data['constraints'].pop()
        path.write_text(json.dumps(data))
        self.assertIn('expected/shop.json', '\n'.join(check.check(self.root)['errors']))

    def test_a_fixture_file_that_git_would_ignore_is_reported(self):
        import subprocess
        repository = self.root.parent
        subprocess.run(['git', 'init', '-q', str(repository)], check=True)
        (repository / '.gitignore').write_text('.hidden\n')
        errors = '\n'.join(check.check(self.root)['errors'])
        self.assertIn('derived/dotfile/.hidden: ignored by .gitignore', errors)
        (repository / '.gitignore').write_text('')
        self.assertEqual(check.check(self.root)['errors'], [])

    def test_a_changed_expectation_is_reported_as_an_outcome_mismatch(self):
        manifest = json.loads((self.root / 'manifest.json').read_text())
        manifest['fixtures']['3.12/vpack']['expected'] = {'outcome': 'accepted'}
        (self.root / 'manifest.json').write_text(json.dumps(manifest))
        self.assertIn('3.12/vpack: expected accepted', '\n'.join(check.check(self.root)['errors']))


if __name__ == '__main__':
    unittest.main()
