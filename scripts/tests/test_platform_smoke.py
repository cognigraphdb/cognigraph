"""Host-platform binary smoke: build identity, ordering and report completeness."""
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('platform_smoke', ROOT / 'scripts/check-platform-smoke.py')
smoke = importlib.util.module_from_spec(spec)
spec.loader.exec_module(smoke)


def artifact(name, executable, kind='bin'):
    return json.dumps({'reason': 'compiler-artifact', 'target': {'name': name, 'kind': [kind]},
                       'executable': executable})


class HostIdentity(unittest.TestCase):
    def test_platform_names_follow_the_docker_convention(self):
        for system, machine, expected in [('Darwin', 'arm64', 'darwin/arm64'),
                                          ('Linux', 'aarch64', 'linux/arm64'),
                                          ('Linux', 'x86_64', 'linux/amd64'),
                                          ('Darwin', 'x86_64', 'darwin/amd64')]:
            with self.subTest(system=system, machine=machine), \
                    patch.object(smoke.platform, 'system', return_value=system), \
                    patch.object(smoke.platform, 'machine', return_value=machine):
                self.assertEqual(smoke.host(), expected)


class Build(unittest.TestCase):
    def test_both_binaries_come_from_cargo_and_the_enterprise_feature_is_explicit(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            for name in ('server-bin', 'cli-bin'):
                (root / name).write_bytes(b'#!/bin/sh\n')
                os.chmod(root / name, 0o755)
            lines = '\n'.join([artifact('cognigraph-server', str(root / 'server-bin')),
                               artifact('cognigraph', str(root / 'cli-bin')),
                               json.dumps({'reason': 'build-finished', 'success': True})])
            with patch.object(smoke.subprocess, 'run', return_value=subprocess.CompletedProcess([], 0, lines)) as run:
                binaries = smoke.build('enterprise', root / 'out')
            command = run.call_args.args[0]
            self.assertEqual(command[:5], ['cargo', 'build', '--locked', '--release', '--no-default-features'])
            self.assertEqual(command[command.index('--features') + 1], 'enterprise')
            self.assertEqual(binaries, {'server': root / 'out/cognigraph-server', 'cli': root / 'out/cognigraph'})
            self.assertTrue(all(os.access(path, os.X_OK) for path in binaries.values()))
            with patch.object(smoke.subprocess, 'run', return_value=subprocess.CompletedProcess([], 0, lines)) as run:
                smoke.build('community', root / 'community')
            self.assertNotIn('--features', run.call_args.args[0])

    def test_missing_or_ambiguous_executables_are_rejected(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            (root / 'server-bin').write_bytes(b'')
            only_server = artifact('cognigraph-server', str(root / 'server-bin'))
            twice = '\n'.join([only_server, only_server, artifact('cognigraph', str(root / 'server-bin'))])
            library = artifact('cognigraph', str(root / 'server-bin'), kind='lib')
            for stdout in (only_server, twice, '', '\n'.join([only_server, library])):
                with self.subTest(stdout=stdout[:20]), \
                        patch.object(smoke.subprocess, 'run', return_value=subprocess.CompletedProcess([], 0, stdout)):
                    with self.assertRaisesRegex(RuntimeError, 'exactly one'):
                        smoke.build('community', root / 'out')


class Run(unittest.TestCase):
    def test_failed_build_stops_before_any_smoke_and_writes_no_report(self):
        with tempfile.TemporaryDirectory() as folder, \
                patch.object(smoke, 'build', side_effect=RuntimeError('synthetic failed build')), \
                patch.object(smoke, 'smoke') as run_smoke:
            with self.assertRaisesRegex(RuntimeError, 'failed build'):
                smoke.run(Path(folder))
            run_smoke.assert_not_called()
            self.assertEqual(list(Path(folder).iterdir()), [])

    def test_failed_community_smoke_stops_the_enterprise_build(self):
        with tempfile.TemporaryDirectory() as folder, \
                patch.object(smoke, 'build', return_value={'server': Path('s'), 'cli': Path('c')}) as build, \
                patch.object(smoke, 'smoke', side_effect=AssertionError('synthetic failed check')):
            with self.assertRaises(AssertionError):
                smoke.run(Path(folder))
            self.assertEqual([c.args[0] for c in build.call_args_list], ['community'])

    def test_report_names_platform_version_and_both_editions(self):
        results = {'community': {'edition': 'community', 'checks': 9, 'binary_sha256': {}},
                   'enterprise': {'edition': 'enterprise', 'checks': 9, 'binary_sha256': {}}}
        with tempfile.TemporaryDirectory() as folder, \
                patch.object(smoke, 'build', return_value={'server': Path('s'), 'cli': Path('c')}), \
                patch.object(smoke, 'smoke', side_effect=lambda binaries, edition, directory: results[edition]), \
                patch.object(smoke, 'host', return_value='darwin/arm64'), \
                patch.object(smoke, 'toolchain', return_value='rustc synthetic'):
            path = smoke.run(Path(folder))
            report = json.loads(path.read_text())
            self.assertEqual(path.name, 'darwin-arm64.json')
            self.assertEqual(report['platform'], 'darwin/arm64')
            self.assertEqual(report['issue'], 'CG-87')
            self.assertRegex(report['version'], r'^\d+\.\d+\.\d+$')
            self.assertEqual([e['edition'] for e in report['editions']], ['community', 'enterprise'])
            smoke.validate_report(path)
            report['editions'].pop()
            path.write_text(json.dumps(report))
            with self.assertRaisesRegex(RuntimeError, 'both editions'):
                smoke.validate_report(path)


if __name__ == '__main__':
    unittest.main()
