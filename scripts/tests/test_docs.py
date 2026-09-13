"""Exercise public documentation navigation using isolated Git working trees."""

import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

SCRIPT = Path(__file__).resolve().parents[1] / 'check-docs.py'
SPEC = importlib.util.spec_from_file_location('check_docs', SCRIPT)
docs = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(docs)


class DocumentationLinks(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = (Path(self.temp.name) / 'code').resolve()
        self.root.mkdir()
        subprocess.run(['git', 'init', '-q', str(self.root)], check=True)
        for attr, value in [('ROOT', self.root), ('PRODUCT', self.root.parent / 'docs')]:
            context = patch.object(docs, attr, value)
            context.start()
            self.addCleanup(context.stop)
        for name in ('README.md', 'AGENTS.md', 'CHANGELOG.md'):
            self.write(name, '# Test checkout\n')
        self.write('docs/changelog/README.md', docs.render_index([]))
        self.write('docs/plans/documentation-migration-2026-09-10.json', '{}')
        self.policy({})

    def write(self, name, content):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content)
        return path

    def policy(self, entries):
        return self.write('scripts/policies/docs-links.json', json.dumps({
            'schema': 'cognigraph-docs-links-v1', 'frozen_link_exceptions': entries}))

    def freeze(self, name, content):
        path = self.write(name, content)
        self.policy({name: {'sha256': hashlib.sha256(path.read_bytes()).hexdigest(),
                            'reason': 'Preserved original; maintained navigation is separate.'}})
        return path

    def test_new_archived_pages_check_all_target_types_and_anchors(self):
        self.write('docs/reference/guide.md', '# Guide\n\n## Existing\n')
        targets = ['../fixtures/m23/', '../.github/workflows/ci.yml',
                   '../../missing.md', '../../missing.json',
                   '../../reference/guide.md#absent']
        self.write('docs/plans/archive/new.md', '\n'.join(
            f'[Target {i}]({target})' for i, target in enumerate(targets)))
        report = docs.check()
        self.assertEqual(len(report['errors']), len(targets), report)
        for target in targets:
            self.assertTrue(any(target in error for error in report['errors']))

    def test_rebased_archive_links_and_encoded_targets_pass(self):
        self.write('fixtures/m23/README.md', '# Templates\n')
        self.write('.github/workflows/ci.yml', 'name: CI\n')
        self.write('docs/reference/guide.md', '# Guide\n\n## Existing\n')
        self.write('docs/reference/with space.json', '{}')
        self.write('docs/plans/archive/rebased.md',
                   '[Fixtures](../../../fixtures/m23/)\n'
                   '[Workflow](../../../.github/workflows/ci.yml)\n'
                   '[Guide](../../reference/guide.md#existing)\n'
                   '[JSON](../../reference/with%20space.json)\n')
        self.assertEqual(docs.check()['errors'], [])

    def test_exact_original_is_skipped_only_while_hash_matches(self):
        path = self.freeze('docs/plans/archive/source.original.md', '[Old](missing.md)\n')
        report = docs.check()
        self.assertEqual(report['errors'], [])
        self.assertEqual(report['frozen_documents_skipped'], 1)
        path.write_text('[Edited](different.md)\n')
        report = docs.check()
        self.assertEqual(report['frozen_documents_skipped'], 0)
        self.assertTrue(any('frozen original changed' in e for e in report['errors']))
        self.assertTrue(any('missing different.md' in e for e in report['errors']))

    def test_original_filename_does_not_implicitly_exempt_a_document(self):
        self.write('docs/plans/archive/unreviewed.original.md', '[Broken](missing.json)\n')
        self.assertTrue(any('missing missing.json' in e for e in docs.check()['errors']))

    def test_missing_frozen_original_and_missing_policy_fail(self):
        path = self.freeze('docs/plans/archive/source.original.md', '# Original\n')
        path.unlink()
        self.assertTrue(any('missing or unsafe frozen original' in e for e in docs.check()['errors']))
        (self.root / 'scripts/policies/docs-links.json').unlink()
        self.assertTrue(any('policy could not be checked' in e for e in docs.check()['errors']))

    def test_invalid_exception_paths_hashes_reasons_and_schemas_fail(self):
        for entries in [
            {'docs/../outside.md': {'sha256': 'a' * 64, 'reason': 'invalid traversal'}},
            {'docs/source.md': {'sha256': 'invalid', 'reason': 'invalid hash'}},
            {'docs/source.md': {'sha256': 'a' * 64, 'reason': ''}},
        ]:
            with self.subTest(entries=entries):
                self.policy(entries)
                self.assertTrue(docs.check()['errors'])
        self.write('scripts/policies/docs-links.json', '{"schema":"unknown"}')
        self.assertTrue(docs.check()['errors'])

    def test_source_examples_are_not_treated_as_navigation(self):
        self.write('docs/plans/archive/examples.md',
                   '```md\n[Example](missing.md)\n```\n'
                   '`[Inline example](missing.json)`\n')
        self.assertEqual(docs.check()['errors'], [])

    def test_optional_product_checkout_still_requires_explicit_validation(self):
        self.write('docs/overview.md', '[Product](../../docs/README.md)\n')
        self.assertEqual(docs.check()['errors'], [])
        self.assertTrue(docs.check(include_product=True)['errors'])
        product = self.root.parent / 'docs'
        product.mkdir()
        (product / 'README.md').write_text('# Product\n')
        self.assertEqual(docs.check(include_product=True)['errors'], [])


if __name__ == '__main__':
    unittest.main()
