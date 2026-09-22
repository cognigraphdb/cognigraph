"""Regression coverage for executable workflow gates, using disposable Git repositories."""
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts'))
import verify

spec = importlib.util.spec_from_file_location('push_gate', ROOT / 'scripts/pre-push.py')
gate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gate)


class ProtectFiles(unittest.TestCase):
    def test_real_hook_paths_and_malformed_inputs(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            (root / 'alias').symlink_to(root / '.env')
            (root / 'secret-alias').symlink_to(root / 'secrets', target_is_directory=True)
            cases = {
                '.env': 2, '.env.local': 2, '.git': 2, '.git/config': 2,
                'secrets/token': 2, 'secrets/.env.example': 2,
                'alias': 2, 'secret-alias/.env.example': 2,
                'docs/../.env': 2, '.env.example': 0, 'ui/.env.example': 0,
                'docs/normal.md': 0,
            }
            for path, expected in cases.items():
                with self.subTest(path=path):
                    result = subprocess.run(['bash', str(ROOT / '.claude/hooks/protect-files.sh')],
                                            input=json.dumps({'cwd': folder, 'tool_input': {'file_path': path}}),
                                            text=True, capture_output=True)
                    self.assertEqual(result.returncode, expected, result.stderr)
            for payload in ('{', '{}', 'null', '{"tool_input":{"file_path":null}}'):
                result = subprocess.run(['bash', str(ROOT / '.claude/hooks/protect-files.sh')],
                                        input=payload, text=True, capture_output=True)
                self.assertEqual(result.returncode, 2)


class Verification(unittest.TestCase):
    def test_ci_always_includes_ui_and_both_edition_browser_runner(self):
        ui = verify.commands('ui')
        ci = verify.commands('ci')
        self.assertEqual(ci[:len(ui)], ui)
        self.assertIn((ROOT / 'ui', ['bun', 'install', '--frozen-lockfile']), ci)
        self.assertIn((ROOT / 'ui', ['bun', 'run', 'check']), ci)
        self.assertIn((ROOT / 'ui', ['bun', 'test']), ci)
        self.assertIn((ROOT / 'ui', ['bun', 'run', 'build']), ci)
        self.assertEqual(ci[-1], (ROOT, [sys.executable, 'scripts/ui_browser.py']))

    def test_ui_install_failure_blocks_ci_before_rust_or_browser_checks(self):
        with patch.object(verify.subprocess, 'run', return_value=subprocess.CompletedProcess([], 9)) as run:
            with self.assertRaisesRegex(RuntimeError, 'frozen-lockfile.*exit 9'):
                verify.run('ci')
        run.assert_called_once()

    def test_ci_disables_inherited_live_llm_opt_in_without_changing_parent_environment(self):
        with patch.dict(os.environ, {'COGNIGRAPH_LIVE_LLM': '1'}), \
             patch.object(verify, 'commands', return_value=[(ROOT, ['synthetic-check'])]), \
             patch.object(verify.subprocess, 'run', return_value=subprocess.CompletedProcess([], 0)) as run:
            verify.run('ci')
            self.assertEqual(run.call_args.kwargs['env']['COGNIGRAPH_LIVE_LLM'], '0')
            self.assertEqual(os.environ['COGNIGRAPH_LIVE_LLM'], '1')

    def test_failure_stops_later_commands(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            steps = [(root, [sys.executable, '-c', 'raise SystemExit(7)']),
                     (root, [sys.executable, '-c', 'open("should-not-exist", "w").close()'])]
            with patch.object(verify, 'commands', return_value=steps):
                with self.assertRaisesRegex(RuntimeError, 'exit 7'):
                    verify.run('ci')
            self.assertFalse((root / 'should-not-exist').exists())


class PushGate(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name) / 'checkout'
        self.root.mkdir()
        self.remote = Path(self.temp.name) / 'remote.git'
        self.git('init', '-q', '-b', 'main')
        self.git('config', 'user.email', 'qa@example.invalid')
        self.git('config', 'user.name', 'Synthetic QA')
        subprocess.run(['git', 'init', '-q', '--bare', '-b', 'main', str(self.remote)], check=True)
        self.git('remote', 'add', 'origin', str(self.remote))
        self.write_version('1.0.0')
        self.commit('baseline')
        self.git('push', '-q', 'origin', 'main')
        self.base = self.git('rev-parse', 'HEAD')
        self.write_version('1.0.1')
        self.commit('candidate')
        self.head = self.git('rev-parse', 'HEAD')
        self.addCleanup(patch.stopall)
        patch.object(gate, 'ROOT', self.root).start()
        original_output = gate.output

        def output(*args):
            if args[:2] == ('gh', 'api'):
                return 'main'
            return original_output(*args)

        patch.object(gate, 'output', side_effect=output).start()
        patch.object(gate, 'github_repository', return_value='owner/repo').start()
        self.prs = patch.object(gate, 'open_prs', return_value=[]).start()
        self.review = patch.object(gate, 'review_incoming').start()

    def git(self, *args):
        return subprocess.check_output(['git', *args], cwd=self.root, text=True).strip()

    def commit(self, message):
        self.git('add', '--all')
        self.git('commit', '-qm', message)

    def write_version(self, version):
        (self.root / 'Cargo.toml').write_text(f'[workspace.package]\nversion = "{version}"\n')
        (self.root / 'Cargo.lock').write_text(f'[[package]]\nname = "cognigraph-core"\nversion = "{version}"\n')
        records = self.root / 'docs/changelog'
        records.mkdir(parents=True, exist_ok=True)
        (records / 'candidate.md').write_text(f'# Candidate\n\n- Status: v{version}\n')

    def updates(self):
        return gate.parse_updates(f'refs/heads/main {self.head} refs/heads/main {self.base}\n', self.head)

    def preflight(self):
        return gate.preflight('origin', self.updates(), self.head)

    def test_clean_versioned_candidate_passes_with_real_git_remote(self):
        self.preflight()

    def test_dirty_or_untracked_tree_is_rejected(self):
        (self.root / 'untracked.txt').write_text('synthetic')
        with self.assertRaisesRegex(RuntimeError, 'pending work'):
            self.preflight()

    def test_unchanged_version_is_rejected(self):
        self.write_version('1.0.0')
        self.commit('missing bump')
        self.head = self.git('rev-parse', 'HEAD')
        with self.assertRaisesRegex(RuntimeError, 'Bump the product version'):
            self.preflight()

    def test_stale_lockfile_and_missing_version_record_are_rejected(self):
        (self.root / 'Cargo.lock').write_text('[[package]]\nname="cognigraph-core"\nversion="1.0.0"\n')
        self.commit('stale lock')
        self.head = self.git('rev-parse', 'HEAD')
        with self.assertRaisesRegex(RuntimeError, 'Cargo.lock'):
            self.preflight()
        self.write_version('1.0.1')
        (self.root / 'docs/changelog/candidate.md').write_text('# No version record\n')
        self.commit('missing record')
        self.head = self.git('rev-parse', 'HEAD')
        with self.assertRaisesRegex(RuntimeError, 'changelog record'):
            self.preflight()

    def test_unreviewed_pr_and_changed_pr_head_are_rejected(self):
        self.git('checkout', '-qb', 'incoming', self.base)
        self.git('commit', '--allow-empty', '-qm', 'incoming')
        incoming = self.git('rev-parse', 'HEAD')
        self.git('checkout', '-q', 'main')
        self.prs.return_value = [(7, incoming, 'main')]
        with self.assertRaisesRegex(RuntimeError, 'PR #7'):
            self.preflight()
        decisions = self.root / 'docs/operations/pr-dispositions.json'
        decisions.parent.mkdir(parents=True)
        decisions.write_text(json.dumps([{'number': 7, 'head': incoming,
            'disposition': 'deferred', 'reason': 'Synthetic test', 'decision': 'Synthetic operator decision'}]))
        self.commit('review decision')
        self.head = self.git('rev-parse', 'HEAD')
        self.preflight()
        self.prs.return_value = [(7, 'f' * 40, 'main')]
        with self.assertRaisesRegex(RuntimeError, 'PR #7'):
            self.preflight()

    def test_integrated_pr_needs_no_manual_disposition(self):
        self.prs.return_value = [(7, self.base, 'main')]
        self.preflight()

    def test_pr_api_failure_blocks(self):
        self.prs.side_effect = subprocess.CalledProcessError(1, ['gh'])
        with self.assertRaises(subprocess.CalledProcessError):
            self.preflight()

    def test_wrong_candidate_and_deletion_are_rejected(self):
        with self.assertRaisesRegex(RuntimeError, 'HEAD candidate'):
            gate.parse_updates(f'refs/heads/old {self.base} refs/heads/old {gate.ZERO}', self.head)
        with self.assertRaisesRegex(RuntimeError, 'deletion'):
            gate.parse_updates(f'(delete) {gate.ZERO} refs/heads/old {self.base}', self.head)

    def test_annotated_tag_must_match_version(self):
        self.git('tag', '-a', 'v9.0.0', '-m', 'Synthetic wrong tag')
        oid = self.git('rev-parse', 'v9.0.0')
        updates = self.updates() + gate.parse_updates(f'refs/tags/v9.0.0 {oid} refs/tags/v9.0.0 {gate.ZERO}', self.head)
        with self.assertRaisesRegex(RuntimeError, 'Tag must match'):
            gate.preflight('origin', updates, self.head)

    def test_remote_change_and_late_pr_change_block_main(self):
        row = f'refs/heads/main {self.head} refs/heads/main {self.base}\n'
        from io import StringIO
        with patch.object(sys, 'argv', ['pre-push.py', 'origin', str(self.remote)]), \
             patch.object(sys, 'stdin', StringIO(row)), patch.object(verify, 'run'), \
             patch.object(gate, 'open_prs', side_effect=[[], [(9, 'f' * 40, 'main')]]):
            with self.assertRaisesRegex(RuntimeError, 'PRs changed'):
                gate.main()
        state = gate.remote_state('origin')
        with patch.object(sys, 'argv', ['pre-push.py', 'origin', str(self.remote)]), \
             patch.object(sys, 'stdin', StringIO(row)), patch.object(verify, 'run'), \
             patch.object(gate, 'remote_state', side_effect=[state, {**state, 'refs/heads/new': self.head}]):
            with self.assertRaisesRegex(RuntimeError, 'Remote refs changed'):
                gate.main()

    def test_incoming_review_brackets_qualification_and_failure_blocks_before_tests(self):
        from io import StringIO
        row = f'refs/heads/main {self.head} refs/heads/main {self.base}\n'
        with patch.object(sys, 'argv', ['pre-push.py', 'origin', str(self.remote)]), \
                patch.object(sys, 'stdin', StringIO(row)), patch.object(verify, 'run'):
            gate.main()
        self.assertEqual([c.args[0] for c in self.review.call_args_list], ['snapshot', 'compare'])
        self.review.side_effect = RuntimeError('review unavailable')
        with patch.object(sys, 'argv', ['pre-push.py', 'origin', str(self.remote)]), \
                patch.object(sys, 'stdin', StringIO(row)), patch.object(verify, 'run') as run:
            with self.assertRaisesRegex(RuntimeError, 'review unavailable'):
                gate.main()
            run.assert_not_called()


if __name__ == '__main__':
    unittest.main()
