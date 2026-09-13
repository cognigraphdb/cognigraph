"""Publication boundaries must fail closed without requiring a private checkout."""

import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

SCRIPT = Path(__file__).resolve().parents[1] / 'check-public-distribution.py'
SPEC = importlib.util.spec_from_file_location('public_distribution', SCRIPT)
policy_check = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(policy_check)


class PublicDistributionTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name) / 'code'
        self.private = Path(self.temp.name) / 'private'
        self.original = 'fixtures/semantic-neurons/trial/provider-calls.json'
        self.body = b'preserved capture'
        self.entry = {
            'id': hashlib.sha256(self.original.encode()).hexdigest()[:20],
            'original_path': self.original, 'package': 'trial', 'access': 'private',
            'archive_path': 'archives/2026-09-13-public-boundary/original/' + self.original,
            'sha256': hashlib.sha256(self.body).hexdigest(), 'bytes': len(self.body),
        }
        self.write(policy_check.POLICY, {
            'max_file_bytes': 4096, 'size_exceptions': {},
            'private_capture_prefixes': ['docs/issues/evidence/', 'ui/audit/'],
            'private_capture_filenames': ['provider-calls.json'],
            'semantic_neuron_test_inputs': {},
        })
        self.catalog = {'schema': 'cognigraph-private-evidence-catalog-v1',
                        'repository': 'cognigraphdb/cognigraph-evidence',
                        'artifacts': [self.entry]}
        self.write(policy_check.CATALOG, self.catalog)
        e = self.entry
        self.write('docs/evidence/trial.md', f"# Trial\n\n## Artifact {e['id']}\n\n"
                   f"`{self.original}`\n\n- SHA-256: `{e['sha256']}`\n- Bytes: {e['bytes']}\n")
        p = self.private / self.entry['archive_path']
        p.parent.mkdir(parents=True)
        p.write_bytes(self.body)

    def write(self, name, value):
        p = self.root / name
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(json.dumps(value) if isinstance(value, dict) else value)

    def check(self, private=False):
        names = [p.relative_to(self.root).as_posix() for p in self.root.rglob('*') if p.is_file()]
        return policy_check.check(self.root, names, self.private if private else None)

    def test_public_checkout_works_without_private_access(self):
        (self.private / self.entry['archive_path']).unlink()
        self.assertEqual(self.check()['errors'], [])
        self.assertTrue(self.check(private=True)['errors'])

    def test_private_byte_tampering_is_detected(self):
        self.assertEqual(self.check(private=True)['private_originals_verified'], 1)
        (self.private / self.entry['archive_path']).write_bytes(b'altered capture!')
        self.assertTrue(self.check(private=True)['errors'])

    def test_raw_captures_cannot_return_under_old_or_new_locations(self):
        for name in ['ui/audit/new/check.json', 'docs/issues/evidence/result.json',
                     'docs/research/provider-calls.json', 'ui/screenshot.png']:
            with self.subTest(name=name):
                self.write(name, 'small synthetic placeholder')
                self.assertTrue(self.check()['errors'])
                (self.root / name).unlink()

    def test_unreviewed_test_corpus_and_oversized_files_are_rejected(self):
        self.write('fixtures/semantic-neurons/new/labels.json', '{}')
        self.assertTrue(self.check()['errors'])
        (self.root / 'fixtures/semantic-neurons/new/labels.json').unlink()
        self.write('docs/results.bin', 'x' * 4097)
        self.assertTrue(self.check()['errors'])

    def test_compiler_cache_cannot_be_required_as_public_fixture(self):
        name = 'fixtures/semantic-neurons/trial/__pycache__/score.pyc'
        policy = json.loads((self.root / policy_check.POLICY).read_text())
        policy['semantic_neuron_test_inputs'][name] = {'sha256': hashlib.sha256(b'cache').hexdigest()}
        self.write(policy_check.POLICY, policy)
        self.write(name, 'cache')
        self.assertIn('Public test input policy contains an invalid fixture path', self.check()['errors'])

    def test_real_origin_and_credential_shape_are_rejected_without_echo(self):
        for value in ['https://example-test.' + 'up.railway.app', 'ghp_' + 'A' * 36]:
            self.write('docs/example.md', value)
            errors = self.check()['errors']
            self.assertTrue(errors)
            self.assertNotIn(value, str(errors))

    def test_catalog_paths_duplicates_and_descriptors_are_checked(self):
        self.entry['archive_path'] = '../outside'
        self.write(policy_check.CATALOG, self.catalog)
        self.assertTrue(self.check()['errors'])
        self.entry['archive_path'] = 'archives/2026-09-13-public-boundary/original/' + self.original
        self.catalog['artifacts'].append(dict(self.entry))
        self.write(policy_check.CATALOG, self.catalog)
        self.assertTrue(self.check()['errors'])
        self.catalog['artifacts'].pop()
        self.write(policy_check.CATALOG, self.catalog)
        self.write('docs/evidence/trial.md', '# Unsupported success claim\n')
        self.assertTrue(self.check()['errors'])


if __name__ == '__main__':
    unittest.main()
