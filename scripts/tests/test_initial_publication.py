"""Fresh publication cannot carry old ancestry or target a reused repository by name alone."""
import importlib.util
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import call, patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts'))
import initial_publication

spec = importlib.util.spec_from_file_location('initial_gate', ROOT / 'scripts/pre-push.py')
gate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gate)


class InitialPublication(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name) / 'checkout'
        self.root.mkdir()
        self.remote = Path(self.temp.name) / 'remote.git'
        self.git('init', '-q', '-b', 'main')
        self.git('config', 'user.email', 'qa@example.invalid')
        self.git('config', 'user.name', 'Synthetic QA')
        subprocess.run(['git', 'init', '-q', '--bare', str(self.remote)], check=True)
        self.git('remote', 'add', 'origin', str(self.remote))
        (self.root / 'Cargo.toml').write_text('[workspace.package]\nversion="2.7.0"\n')
        (self.root / 'Cargo.lock').write_text('[[package]]\nname="cognigraph-core"\nversion="2.7.0"\n')
        records = self.root / 'docs/changelog'
        records.mkdir(parents=True)
        (records / 'release.md').write_text('- Status: v2.7.0\n')
        self.git('add', '--all')
        self.git('commit', '-qm', 'Synthetic root snapshot')
        self.head = self.git('rev-parse', 'HEAD')
        self.manifest = {'operation': 'initial-publication', 'candidate': self.head,
                         'repository': 'owner/repo', 'repository_id': 123,
                         'private': True,
                         'previous_version': '2.6.2'}
        self.path = Path(self.temp.name) / 'manifest.json'
        self.write_manifest()
        self.addCleanup(patch.stopall)
        patch.object(gate, 'ROOT', self.root).start()
        original = gate.output

        def output(*args):
            if args[:2] == ('gh', 'api'):
                return json.dumps({'id': 123, 'private': True})
            return original(*args)

        patch.object(gate, 'output', side_effect=output).start()
        patch.object(gate, 'github_repository', return_value='owner/repo').start()
        self.prs = patch.object(gate, 'open_prs', return_value=[]).start()
        self.verify = patch.object(gate.verify, 'run').start()

    def git(self, *args):
        return subprocess.check_output(['git', *args], cwd=self.root, text=True).strip()

    def write_manifest(self):
        self.path.write_text(json.dumps(self.manifest))

    def row(self):
        return f'refs/heads/main {self.head} refs/heads/main {gate.ZERO}\n'

    def run_gate(self, stdin=None):
        initial_publication.run(gate, str(self.path), 'origin', self.row() if stdin is None else stdin, self.head)

    def test_single_root_runs_every_suite(self):
        self.run_gate()
        self.assertEqual(self.verify.call_args_list, [call('ci'), call('docker'), call('ui')])
        self.assertEqual(self.git('ls-remote', 'origin'), '')

    def test_existing_history_is_rejected(self):
        self.git('commit', '--allow-empty', '-qm', 'Inherited history')
        self.head = self.git('rev-parse', 'HEAD')
        self.manifest['candidate'] = self.head
        self.write_manifest()
        with self.assertRaisesRegex(RuntimeError, 'one root commit'):
            self.run_gate()
        self.verify.assert_not_called()

    def test_wrong_repository_identity_candidate_or_version_is_rejected(self):
        for key, value in [('operation', 'ordinary'), ('candidate', 'f' * 40),
                           ('repository', 'owner/other'), ('repository_id', 456),
                           ('private', False), ('private', 'true'),
                           ('previous_version', '2.7.0'), ('previous_version', 'bad')]:
            original = self.manifest[key]
            self.manifest[key] = value
            self.write_manifest()
            with self.subTest(key=key, value=value), self.assertRaises(RuntimeError):
                self.run_gate()
            self.manifest[key] = original
        self.verify.assert_not_called()

    def test_extra_refs_wrong_branch_and_existing_branch_are_rejected(self):
        for stdin in (self.row() * 2, self.row().replace('refs/heads/main', 'refs/heads/other'),
                      self.row().replace(gate.ZERO, 'f' * 40), ''):
            with self.subTest(stdin=stdin), self.assertRaises(RuntimeError):
                self.run_gate(stdin)
        self.verify.assert_not_called()

    def test_nonempty_destination_and_incoming_prs_are_rejected(self):
        self.prs.return_value = [(1, 'f' * 40, 'main')]
        with self.assertRaisesRegex(RuntimeError, 'incoming PRs'):
            self.run_gate()
        self.prs.return_value = []
        self.git('push', '-q', 'origin', 'HEAD:refs/heads/existing')
        with self.assertRaisesRegex(RuntimeError, 'must be empty'):
            self.run_gate()

    def test_stale_lock_or_missing_record_is_rejected(self):
        for relative, content, message in [
            ('Cargo.lock', '[[package]]\nname="cognigraph-core"\nversion="2.6.2"\n', 'Cargo.lock'),
            ('docs/changelog/release.md', '# Missing record\n', 'release record'),
        ]:
            path = self.root / relative
            original = path.read_text()
            path.write_text(content)
            self.git('add', relative)
            self.git('commit', '--amend', '--no-edit', '-q')
            self.head = self.git('rev-parse', 'HEAD')
            self.manifest['candidate'] = self.head
            self.write_manifest()
            with self.assertRaisesRegex(RuntimeError, message):
                self.run_gate()
            path.write_text(original)
            self.git('add', relative)

    def test_failed_ci_stops_later_suites(self):
        self.verify.side_effect = RuntimeError('CI failed')
        with self.assertRaisesRegex(RuntimeError, 'CI failed'):
            self.run_gate()
        self.verify.assert_called_once_with('ci')

    def test_manifest_ref_identity_and_pr_drift_are_rejected(self):
        def change_manifest(suite):
            if suite == 'ui':
                self.path.write_text('{}')
        self.verify.side_effect = change_manifest
        with self.assertRaisesRegex(RuntimeError, 'manifest changed'):
            self.run_gate()
        self.write_manifest()
        self.verify.side_effect = None
        with patch.object(gate, 'remote_state', side_effect=[{}, {'refs/heads/other': self.head}]):
            with self.assertRaisesRegex(RuntimeError, 'must be empty'):
                self.run_gate()
        with patch.object(gate, 'open_prs', side_effect=[[], [(1, self.head, 'main')]]):
            with self.assertRaisesRegex(RuntimeError, 'incoming PRs'):
                self.run_gate()
        original = gate.output
        count = 0
        def changed_identity(*args):
            nonlocal count
            if args[:2] == ('gh', 'api'):
                count += 1
                return json.dumps({'id': 123 if count == 1 else 456, 'private': True})
            return original(*args)
        with patch.object(gate, 'output', side_effect=changed_identity):
            with self.assertRaisesRegex(RuntimeError, 'identity changed'):
                self.run_gate()

    def test_visibility_change_during_verification_blocks_publication(self):
        original = gate.output
        count = 0
        def changed_visibility(*args):
            nonlocal count
            if args[:2] == ('gh', 'api'):
                count += 1
                return json.dumps({'id': 123, 'private': count == 1})
            return original(*args)
        with patch.object(gate, 'output', side_effect=changed_visibility):
            with self.assertRaisesRegex(RuntimeError, 'visibility changed'):
                self.run_gate()

    def test_mode_flag_is_consumed_and_conflicting_modes_are_rejected(self):
        def dispatched(*args):
            self.assertNotIn('COGNIGRAPH_INITIAL_PUBLICATION_MANIFEST', os.environ)
            self.assertEqual(args[1], str(self.path))
        with patch.dict(os.environ, {'COGNIGRAPH_INITIAL_PUBLICATION_MANIFEST': str(self.path)}), \
                patch.object(sys, 'argv', ['pre-push.py', 'origin', str(self.remote)]), \
                patch.object(sys, 'stdin', io.StringIO(self.row())), \
                patch.object(initial_publication, 'run', side_effect=dispatched) as run:
            gate.main()
            run.assert_called_once()
        with patch.dict(os.environ, {'COGNIGRAPH_INITIAL_PUBLICATION_MANIFEST': str(self.path),
                                     'COGNIGRAPH_HISTORY_REMOVAL_MANIFEST': str(self.path)}), \
                patch.object(sys, 'argv', ['pre-push.py', 'origin', str(self.remote)]), \
                patch.object(sys, 'stdin', io.StringIO(self.row())):
            with self.assertRaisesRegex(RuntimeError, 'only one'):
                gate.main()


if __name__ == '__main__':
    unittest.main()
