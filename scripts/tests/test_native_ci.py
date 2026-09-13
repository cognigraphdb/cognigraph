"""Native acceptance orchestration must retain failures and reject incomplete reports."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import native_ci


class NativeAcceptance(unittest.TestCase):
    def report(self):
        return {'edition': 'community', 'binary_sha256': 'synthetic',
                'runs': [{'mode': mode, 'checks': 1} for mode in native_ci.MODES],
                'configuration': [{} for _ in range(5)]}

    def test_success_exit_without_all_modes_cannot_qualify_a_binary(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / 'report.json'
            for mutate in (lambda r: r['runs'].pop(),
                           lambda r: r['runs'].append(r['runs'][0]),
                           lambda r: r['runs'][0].update(checks=0),
                           lambda r: r.update(edition='enterprise'),
                           lambda r: r['configuration'].pop()):
                report = self.report()
                mutate(report)
                path.write_text(json.dumps(report))
                with self.assertRaises(RuntimeError):
                    native_ci.validate_report(path, 'community')
            path.write_text(json.dumps(self.report()))
            self.assertEqual(native_ci.validate_report(path, 'community')['startup_rejections'], 5)

    def test_failure_is_retained_and_stops_later_edition(self):
        with tempfile.TemporaryDirectory() as folder, \
             patch.object(native_ci, 'build', side_effect=RuntimeError('synthetic failed build')) as build:
            with self.assertRaisesRegex(RuntimeError, 'failed build'):
                native_ci.run(Path(folder))
            build.assert_called_once()
            capture = next(Path(folder).glob('run-*/summary.json'))
            report = json.loads(capture.read_text())
            self.assertEqual(report['status'], 'failed')
            self.assertIn('synthetic failed build', report['error'])

    def test_runtime_failure_is_retained_without_reusing_prior_capture(self):
        with tempfile.TemporaryDirectory() as folder, \
             patch.object(native_ci, 'build', return_value=Path('/synthetic/server')), \
             patch.object(native_ci.subprocess, 'run', side_effect=subprocess.CalledProcessError(9, ['synthetic'])):
            output = Path(folder)
            for _ in range(2):
                with self.assertRaises(subprocess.CalledProcessError):
                    native_ci.run(output)
            self.assertEqual(len(list(output.glob('run-*/summary.json'))), 2)
            self.assertEqual(len(list(output.glob('run-*/community-runtime.log'))), 2)


if __name__ == '__main__':
    unittest.main()
