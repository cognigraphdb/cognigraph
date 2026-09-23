"""Current releases, compatible resolutions and narrowly reviewed pins."""
from datetime import date
import io
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts'))
import dependency_freshness as gate

EMPTY = {'cargo': {'added': [], 'removed': []}, 'npm': {'added': [], 'removed': []}}


class Freshness(unittest.TestCase):
    def test_new_major_direct_release_and_compatible_transitive_drift_block(self):
        direct = {('cargo', 'synthetic', '1.0.0')}
        self.assertFalse(gate.assess(direct, {('cargo', 'synthetic'): '2.0.0'}, EMPTY, [])['passed'])
        changed = {**EMPTY, 'cargo': {'removed': [('indirect', '1.0.0')], 'added': [('indirect', '1.0.1')]}}
        self.assertFalse(gate.assess(direct, {('cargo', 'synthetic'): '1.0.0'}, changed, [])['passed'])
        self.assertTrue(gate.assess(direct, {('cargo', 'synthetic'): '1.0.0'}, EMPTY, [])['passed'])

    def test_exact_reviewed_pin_does_not_cover_a_newer_release(self):
        record = {'ecosystem': 'cargo', 'package': 'synthetic', 'current': '1.0.0', 'latest': '2.0.0'}
        direct = {('cargo', 'synthetic', '1.0.0')}
        self.assertTrue(gate.assess(direct, {('cargo', 'synthetic'): '2.0.0'}, EMPTY, [record])['passed'])
        with self.assertRaisesRegex(RuntimeError, 'stale'):
            gate.assess(direct, {('cargo', 'synthetic'): '2.0.1'}, EMPTY, [record])

    def test_expired_or_untraceable_pins_block(self):
        with tempfile.TemporaryDirectory() as folder, patch.object(gate, 'ROOT', Path(folder)):
            root = Path(folder)
            (root / 'docs').mkdir()
            (root / 'docs/decision.md').write_text('# Synthetic review')
            path = root / 'exceptions.json'
            record = {'ecosystem': 'cargo', 'package': 'synthetic', 'current': '1', 'latest': '2',
                      'reason': 'Synthetic migration review', 'decision': 'docs/decision.md',
                      'expires': '2026-09-15'}
            path.write_text(json.dumps([record]))
            self.assertEqual(gate.exceptions(path, date(2026, 9, 14)), [record])
            with self.assertRaisesRegex(RuntimeError, 'Expired'):
                gate.exceptions(path, date(2026, 9, 16))
            record['decision'] = '../outside.md'
            path.write_text(json.dumps([record]))
            with self.assertRaisesRegex(RuntimeError, 'engineering decision'):
                gate.exceptions(path, date(2026, 9, 14))

    def test_workspace_inventory_includes_optional_dev_and_patched_dependencies(self):
        metadata = {'workspace_members': ['app', 'core'], 'packages': [
            {'id': 'app', 'name': 'app', 'version': '1'},
            {'id': 'core', 'name': 'core', 'version': '1'},
            {'id': 'optional', 'name': 'optional', 'version': '2', 'source': 'registry+https://github.com/rust-lang/crates.io-index'},
            {'id': 'patched', 'name': 'patched', 'version': '3', 'source': None}],
            'resolve': {'nodes': [{'id': 'app', 'deps': [{'pkg': 'core'}, {'pkg': 'optional'}]},
                                  {'id': 'core', 'deps': [{'pkg': 'patched'}]}]}}
        self.assertEqual(gate.cargo_direct(metadata), {('cargo', 'optional', '2'), ('cargo', 'patched', '3')})
        manifest = {'dependencies': {'prod': '^1'}, 'devDependencies': {'dev': '^2'}}
        lock = {'packages': {'prod': ['prod@1.1.0'], 'dev': ['dev@2.1.0'], 'other': ['other@3.0.0']}}
        self.assertEqual(gate.npm_direct(manifest, lock), {('npm', 'prod', '1.1.0'), ('npm', 'dev', '2.1.0')})

    def test_resolving_updates_does_not_change_candidate_lockfiles(self):
        with tempfile.TemporaryDirectory() as folder, patch.object(gate, 'ROOT', Path(folder)):
            root = Path(folder)
            for name in ('crates', 'vendor', 'reports', *gate.BUN_PROJECTS):
                (root / name).mkdir(parents=True)
            (root / 'Cargo.toml').write_text('[workspace]')
            old = '[[package]]\nname="synthetic"\nversion="1.0.0"\n'
            (root / 'Cargo.lock').write_text(old)
            for project in gate.BUN_PROJECTS:
                (root / project / 'package.json').write_text('{"dependencies":{"synthetic":"^1"}}')
                (root / project / 'bun.lock').write_text('candidate bytes')
            resolved = []
            metadata = {'workspace_members': [], 'packages': [], 'resolve': {'nodes': []}}
            lock = {'packages': {'synthetic': ['synthetic@1.0.0']}}
            def execute(command, cwd, log):
                if command[:2] == ['cargo', 'metadata']:
                    return json.dumps(metadata)
                self.assertNotEqual(cwd, root)
                if command[:2] == ['cargo', 'update']:
                    (cwd / 'Cargo.lock').write_text(old.replace('1.0.0', '1.0.1'))
                if command[:2] == ['bun', 'update']:
                    resolved.append(cwd)
                    self.assertIn('--ignore-scripts', command)
                    self.assertIn('--no-cache', command)
                    (cwd / 'bun.lock').write_text('temporary update')
                return ''
            with patch.object(gate, 'run', side_effect=execute), patch.object(gate, 'bun_lock', return_value=lock):
                _, changes = gate.resolutions(root / 'reports')
            self.assertTrue(changes['cargo']['added'])
            self.assertEqual((root / 'Cargo.lock').read_text(), old)
            for project in gate.BUN_PROJECTS:
                self.assertEqual((root / project / 'bun.lock').read_text(), 'candidate bytes')
            self.assertEqual(sorted(p.as_posix().split('/')[-1] for p in resolved),
                             sorted(Path(p).name for p in gate.BUN_PROJECTS))
            self.assertIn('clients/typescript', gate.BUN_PROJECTS)
            self.assertIn('ui', gate.BUN_PROJECTS)

    def test_registry_errors_are_not_interpreted_as_no_updates(self):
        with patch.object(gate, 'urlopen', side_effect=OSError('network unavailable')):
            with self.assertRaises(OSError):
                gate.latest('cargo', 'synthetic')
        with patch.object(gate, 'urlopen', return_value=io.StringIO('{"crate":{"max_stable_version":null,"max_version":"2.0.0-rc.1"}}')):
            self.assertEqual(gate.latest('cargo', 'synthetic'), '2.0.0-rc.1')


if __name__ == '__main__':
    unittest.main()
