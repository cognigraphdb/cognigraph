"""Regression coverage for issue identifier allocation and the overwrite guard."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / 'scripts/issue.py'

REGISTRY = """## Issues

| Issue | Priority | Status | Finding |
|---|---|---|---|
| [CG-1](CG-1.md) | P2 | Resolved | Original defect |

Next available identifier: **CG-2**.
"""


def run(root, *args):
    env = dict(os.environ, COGNIGRAPH_ROOT=str(root))
    result = subprocess.run([sys.executable, str(SCRIPT), *args], cwd=root, env=env,
                            text=True, capture_output=True)
    return result.returncode, result.stdout, result.stderr


def errors(stdout):
    return json.loads(stdout)['errors']


class IssueRegistry(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.root = Path(self._tmp.name)
        issues = self.root / 'docs/issues'
        issues.mkdir(parents=True)
        (issues / 'CG-1.md').write_text('# CG-1: Original defect\n\n- Status: Resolved\n- Priority: P2\n')
        (issues / 'README.md').write_text(REGISTRY)
        for cmd in (['git', 'init', '-q'], ['git', 'config', 'user.email', 't@t'],
                    ['git', 'config', 'user.name', 't'], ['git', 'add', '-A'],
                    ['git', 'commit', '-qm', 'init']):
            subprocess.run(cmd, cwd=self.root, check=True, capture_output=True)

    def tearDown(self):
        self._tmp.cleanup()

    def test_clean_tree_passes(self):
        code, out, err = run(self.root, 'check')
        self.assertEqual(code, 0, err)
        self.assertEqual(errors(out), [])

    def test_overwriting_a_committed_issue_fails(self):
        (self.root / 'docs/issues/CG-1.md').write_text('# CG-1: Something new\n\n- Status: Open\n')
        code, out, _ = run(self.root, 'check')
        self.assertEqual(code, 1)
        self.assertTrue(any('title changed from the committed record' in e for e in errors(out)))

    def test_hand_written_file_without_row_fails(self):
        (self.root / 'docs/issues/CG-2.md').write_text('# CG-2: Hand written\n\n- Status: Open\n')
        code, out, _ = run(self.root, 'check')
        self.assertEqual(code, 1)
        self.assertTrue(any('no registry row' in e for e in errors(out)))

    def test_next_honours_registry_counter_and_untracked_files(self):
        readme = self.root / 'docs/issues/README.md'
        readme.write_text(readme.read_text().replace('**CG-2**', '**CG-4**'))
        self.assertEqual(run(self.root, 'next')[1].strip(), 'CG-4')
        (self.root / 'docs/issues/CG-6.md').write_text('# CG-6: Someone else\n\n- Status: Open\n')
        self.assertEqual(run(self.root, 'next')[1].strip(), 'CG-7')

    def test_new_allocates_file_and_row(self):
        code, out, err = run(self.root, 'new', 'Guard test issue', '--priority', 'P3', '--area', 'Docs')
        self.assertEqual(code, 0, err)
        created = self.root / 'docs/issues/CG-2.md'
        self.assertTrue(created.is_file())
        self.assertTrue(created.read_text().startswith('# CG-2: Guard test issue\n'))
        registry = (self.root / 'docs/issues/README.md').read_text()
        self.assertIn('| [CG-2](CG-2.md) | P3 | Open | Guard test issue |', registry)
        self.assertIn('Next available identifier: **CG-3**.', registry)
        code, out, err = run(self.root, 'check')
        self.assertEqual(code, 0, err)
        self.assertEqual(errors(out), [])

    def test_new_never_overwrites(self):
        (self.root / 'docs/issues/CG-2.md').write_text('# CG-2: Pre-existing\n\n- Status: Open\n')
        code, out, _ = run(self.root, 'new', 'Another')
        self.assertEqual(code, 0)
        self.assertTrue((self.root / 'docs/issues/CG-3.md').is_file())
        self.assertEqual((self.root / 'docs/issues/CG-2.md').read_text(),
                         '# CG-2: Pre-existing\n\n- Status: Open\n')

    def test_committed_retitle_and_deleted_identity_fail(self):
        issue = self.root / 'docs/issues/CG-1.md'
        registry = self.root / 'docs/issues/README.md'
        issue.write_text(issue.read_text().replace('Original defect', 'Reused identity'))
        registry.write_text(registry.read_text().replace('Original defect', 'Reused identity'))
        subprocess.run(['git', 'add', '-A'], cwd=self.root, check=True)
        subprocess.run(['git', 'commit', '-qm', 'reuse'], cwd=self.root, check=True)
        code, out, _ = run(self.root, 'check')
        self.assertEqual(code, 1)
        self.assertIn('title changed from the committed record', out)
        issue.unlink()
        registry.write_text(REGISTRY.replace('| [CG-1](CG-1.md) | P2 | Resolved | Original defect |\n', '').replace('**CG-2**', '**CG-1**'))
        subprocess.run(['git', 'add', '-A'], cwd=self.root, check=True)
        subprocess.run(['git', 'commit', '-qm', 'delete'], cwd=self.root, check=True)
        self.assertEqual(run(self.root, 'check')[0], 1)
        self.assertEqual(run(self.root, 'next')[1].strip(), 'CG-2')

    def test_bad_link_and_priority_fail(self):
        registry = self.root / 'docs/issues/README.md'
        registry.write_text(REGISTRY.replace('(CG-1.md)', '(CG-2.md)'))
        self.assertEqual(run(self.root, 'check')[0], 1)
        registry.write_text(REGISTRY.replace('| P2 |', '| P3 |'))
        self.assertEqual(run(self.root, 'check')[0], 1)

    def test_invalid_input_leaves_registry_unchanged(self):
        for title in ('', 'a|b', 'a\nb'):
            self.assertNotEqual(run(self.root, 'new', title)[0], 0)
        self.assertEqual((self.root / 'docs/issues/README.md').read_text(), REGISTRY)
        self.assertFalse((self.root / 'docs/issues/CG-2.md').exists())

    def test_parallel_allocators_preserve_every_row(self):
        env = dict(os.environ, COGNIGRAPH_ROOT=str(self.root))
        jobs = [subprocess.Popen([sys.executable, str(SCRIPT), 'new', f'Parallel {i}'],
                                 cwd=self.root, env=env, stdout=subprocess.PIPE,
                                 stderr=subprocess.PIPE, text=True) for i in range(8)]
        for job in jobs:
            out, err = job.communicate(timeout=30)
            self.assertEqual(job.returncode, 0, out + err)
        code, out, err = run(self.root, 'check')
        self.assertEqual(code, 0, out + err)
        self.assertEqual(json.loads(out)['issues'], 9)

    def test_shallow_history_fails_closed(self):
        with tempfile.TemporaryDirectory() as folder:
            clone = Path(folder) / 'clone'
            subprocess.run(['git', 'clone', '-q', '--depth=1', self.root.as_uri(), str(clone)], check=True)
            self.assertNotEqual(run(clone, 'check')[0], 0)


if __name__ == '__main__':
    unittest.main()
