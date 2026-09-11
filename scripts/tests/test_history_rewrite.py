"""The exceptional removal path must stay bound to its complete reviewed transaction."""
import base64
import copy
import hashlib
import importlib.util
import io
import os
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import history_rewrite


class HistoryRemovalChecks(unittest.TestCase):
    def setUp(self):
        self.old, self.head, self.tag = '1' * 40, '2' * 40, '3' * 40
        self.manifest = {'operation': 'remove-private-material', 'candidate': self.head,
                         'updates': [{'ref': 'refs/heads/main', 'old': self.old, 'new': self.head},
                                     {'ref': 'refs/tags/v1.0.0', 'old': self.old, 'new': self.tag}]}
        self.stdin = (f'refs/heads/main {self.head} refs/heads/main {self.old}\n'
                      f'refs/tags/v1.0.0 {self.tag} refs/tags/v1.0.0 {self.old}\n')

    def test_exact_transaction_accepts_distinct_historical_tip(self):
        result = history_rewrite.transaction(self.manifest, self.stdin, self.head)
        self.assertEqual(result['refs/tags/v1.0.0'], (self.old, self.tag))

    def test_rejects_partial_extra_deleted_or_changed_refs(self):
        variants = [self.stdin.splitlines()[0], self.stdin + self.stdin.splitlines()[0],
                    self.stdin.replace(self.tag, '0' * 40),
                    self.stdin.replace(self.old, '4' * 40),
                    self.stdin.replace('refs/tags/v1.0.0', 'refs/pull/1/head')]
        for stdin in variants:
            with self.subTest(stdin=stdin), self.assertRaises(RuntimeError):
                history_rewrite.transaction(self.manifest, stdin, self.head)

    def test_manifest_cannot_allow_deletion_or_readonly_refs(self):
        for key, value in [('ref', 'refs/pull/1/head'), ('new', '0' * 40), ('old', 'bad')]:
            manifest = copy.deepcopy(self.manifest)
            manifest['updates'][1][key] = value
            with self.subTest(key=key), self.assertRaises(RuntimeError):
                history_rewrite.transaction(manifest, self.stdin, self.head)

    def test_wrong_candidate_or_operation_fails(self):
        with self.assertRaises(RuntimeError):
            history_rewrite.transaction(self.manifest, self.stdin, '5' * 40)
        self.manifest['operation'] = 'publish-feature'
        with self.assertRaises(RuntimeError):
            history_rewrite.transaction(self.manifest, self.stdin, self.head)

    def test_receipt_must_cover_tips_and_have_no_findings(self):
        updates = history_rewrite.transaction(self.manifest, self.stdin, self.head)
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp) / 'checkout'; root.mkdir()
            path = Path(tmp) / 'receipt.json'
            audit = {'published_tips': {r: n for r, (_, n) in updates.items()},
                     'passed': True, 'remaining_matches': 0, 'unexpected_tree_changes': 0}
            for change in ({}, {'remaining_matches': 1}, {'unexpected_tree_changes': 1},
                           {'passed': False}, {'published_tips': {}}):
                path.write_text(json.dumps(dict(audit, **change)))
                self.manifest.update(receipt_path=str(path),
                    receipt_sha256=hashlib.sha256(path.read_bytes()).hexdigest())
                if change:
                    with self.subTest(change=change), self.assertRaises(RuntimeError):
                        history_rewrite.receipt(self.manifest, updates, root)
                else:
                    history_rewrite.receipt(self.manifest, updates, root)
            self.manifest['receipt_sha256'] = '0' * 64
            with self.assertRaises(RuntimeError):
                history_rewrite.receipt(self.manifest, updates, root)

    def test_mode_flag_is_consumed_before_child_verification(self):
        spec = importlib.util.spec_from_file_location('removal_gate_test',
            Path(__file__).resolve().parents[1] / 'pre-push.py')
        gate = importlib.util.module_from_spec(spec); spec.loader.exec_module(gate)
        url = 'git@github.com:owner/repo.git'
        def output(*args):
            if args == ('git', 'remote'):
                return 'origin'
            if args[:3] == ('git', 'remote', 'get-url'):
                return url
            return self.head
        def removed(*args):
            self.assertNotIn('COGNIGRAPH_HISTORY_REMOVAL_MANIFEST', os.environ)
            self.assertEqual(args[1], '/private/manifest.json')
        with patch.dict(os.environ, {'COGNIGRAPH_HISTORY_REMOVAL_MANIFEST': '/private/manifest.json'}), \
                patch.object(gate, 'output', side_effect=output), \
                patch.object(sys, 'argv', ['pre-push.py', 'origin', url]), \
                patch.object(sys, 'stdin', io.StringIO(self.stdin)), \
                patch.object(history_rewrite, 'run', side_effect=removed) as run:
            gate.main()
            run.assert_called_once()

    def test_ci_failure_stops_before_docker_or_publication(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp) / 'checkout'; root.mkdir()
            (root / 'Cargo.lock').write_text('[[package]]\nname="cognigraph-core"\nversion="2.0.1"\n')
            records = root / 'docs/changelog'; records.mkdir(parents=True)
            (records / 'release.md').write_text('- Status: v2.0.1')
            path = Path(tmp) / 'manifest.json'
            self.manifest.update(repository='owner/repo', remote_state={'refs/heads/main': self.old,
                                                                     'refs/tags/v1.0.0': self.old})
            path.write_text(json.dumps(self.manifest))
            gate = Mock(ROOT=root)
            def require(ok, message):
                if not ok:
                    raise RuntimeError(message)
            gate.require.side_effect = require
            gate.github_repository.return_value = 'owner/repo'
            gate.remote_state.return_value = self.manifest['remote_state']
            content = base64.b64encode(b'[workspace.package]\nversion="2.0.0"').decode()
            gate.output.return_value = json.dumps({'content': content})
            gate.version_at.return_value = (2, 0, 1)
            gate.open_prs.return_value = []
            gate.verify.run.side_effect = RuntimeError('CI failed')
            with patch.object(history_rewrite, 'receipt', return_value=b'checked'):
                with self.assertRaisesRegex(RuntimeError, 'CI failed'):
                    history_rewrite.run(gate, str(path), 'origin', self.stdin, self.head)
            gate.verify.run.assert_called_once_with('ci')


if __name__ == '__main__':
    unittest.main()
