"""Image scan regressions: coverage, exact subject, failures and suppression bypasses."""
import copy
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
sys.path.insert(0, str(ROOT / 'scripts'))
spec = importlib.util.spec_from_file_location('image_vulnerabilities',
                                             ROOT / 'scripts/check-image-vulnerabilities.py')
scanner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(scanner)
IMAGE = 'sha256:' + 'a' * 64
REPORT = {'SchemaVersion': 2, 'Metadata': {'ImageID': IMAGE, 'OS': {'Family': 'debian'}},
          'Results': [{'Class': 'os-pkgs', 'Packages': [{'Name': 'libc6'}],
                       'Vulnerabilities': []}]}


def vulnerability(severity='LOW', fixed='2'):
    return {'VulnerabilityID': 'CVE-2099-0001', 'PkgName': 'synthetic',
            'InstalledVersion': '1', 'FixedVersion': fixed, 'Severity': severity}


class ScanCoverage(unittest.TestCase):
    def test_empty_inventory_wrong_identity_or_unsupported_os_is_not_a_clean_scan(self):
        variants = []
        for key, value in [('SchemaVersion', 1), ('Results', [])]:
            report = copy.deepcopy(REPORT)
            report[key] = value
            variants.append(report)
        for os_info in ({}, {'Family': 'debian', 'EOSL': True}, {'Family': 'unknown'}):
            report = copy.deepcopy(REPORT)
            report['Metadata']['OS'] = os_info
            variants.append(report)
        report = copy.deepcopy(REPORT)
        report['Metadata']['ImageID'] = 'sha256:' + 'b' * 64
        variants.append(report)
        for report in variants:
            with self.subTest(report=report), self.assertRaises(RuntimeError):
                scanner.findings(report, IMAGE)
        self.assertEqual(scanner.findings(REPORT, IMAGE), [])

    def test_incomplete_finding_cannot_silently_qualify(self):
        report = copy.deepcopy(REPORT)
        report['Results'][0]['Vulnerabilities'] = [{'PkgName': 'synthetic'}]
        with self.assertRaisesRegex(RuntimeError, 'Incomplete'):
            scanner.findings(report, IMAGE)

    def test_all_fixable_severities_block_and_unfixed_findings_are_retained(self):
        report = copy.deepcopy(REPORT)
        items = [vulnerability(severity) for severity in ('LOW', 'MEDIUM', 'HIGH', 'CRITICAL')]
        items.append(vulnerability('HIGH', ''))
        report['Results'][0]['Vulnerabilities'] = items
        with tempfile.TemporaryDirectory() as folder:
            directory = Path(folder)
            def fake_run(command, **kwargs):
                self.assertEqual(command[-1], IMAGE)
                self.assertEqual(kwargs['cwd'], directory)
                self.assertFalse(any(k.startswith('TRIVY_') for k in kwargs['env']))
                self.assertIn(str(directory / 'empty.ignore'), command)
                self.assertIn(str(directory / 'empty.yaml'), command)
                self.assertTrue(kwargs['check'])
                (directory / 'community.json').write_text(json.dumps(report))
            with patch.dict(os.environ, {'TRIVY_IGNORE_UNFIXED': 'true', 'TRIVY_SEVERITY': 'LOW'}), \
                    patch.object(scanner.subprocess, 'run', side_effect=fake_run):
                result = scanner.scan('community', IMAGE, directory)
            self.assertEqual(result['fixable'], 4)
            self.assertEqual(result['findings'], 5)
            self.assertEqual(result['by_severity']['HIGH'], 2)

    def test_failure_in_either_edition_blocks_even_if_the_other_is_clean(self):
        for failure in (RuntimeError('invalid report'),
                        subprocess.CalledProcessError(1, ['trivy'])):
            with tempfile.TemporaryDirectory() as folder, \
                    patch.object(scanner, 'ROOT', Path(folder)), \
                    patch.object(scanner, 'output', return_value=IMAGE), \
                    patch.object(scanner, 'scan', side_effect=[failure, {'fixable': 0}]) as scan:
                with self.assertRaisesRegex(RuntimeError, 'Image scan failed'):
                    scanner.main()
                self.assertEqual(scan.call_count, 2)

    def test_fixable_finding_fails_the_complete_gate(self):
        with tempfile.TemporaryDirectory() as folder, \
                patch.object(scanner, 'ROOT', Path(folder)), \
                patch.object(scanner, 'output', return_value=IMAGE), \
                patch.object(scanner, 'scan', side_effect=[{'fixable': 0}, {'fixable': 1}]):
            with self.assertRaisesRegex(RuntimeError, 'Fixable image vulnerabilities'):
                scanner.main()


if __name__ == '__main__':
    unittest.main()
