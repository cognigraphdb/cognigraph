#!/usr/bin/env python3
"""Scan both built editions; fail on any vulnerability with an available fix."""
from collections import Counter
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

from docker_images import IMAGES, ROOT, output, require


def findings(report, image_id):
    require(report.get('SchemaVersion') == 2, 'Unsupported Trivy report schema')
    metadata = report.get('Metadata', {})
    require(metadata.get('ImageID') == image_id, 'Scan does not match the built image')
    require(metadata.get('OS', {}).get('Family') == 'debian', 'Missing Debian scan coverage')
    require(not metadata['OS'].get('EOSL', False), 'Container OS is end of life')
    results = report.get('Results', [])
    require(any(r.get('Class') == 'os-pkgs' and r.get('Packages') for r in results),
            'Missing operating-system package inventory')
    vulnerabilities = []
    for result in results:
        for item in result.get('Vulnerabilities', []):
            require(all(item.get(key) for key in ('VulnerabilityID', 'PkgName',
                                                 'InstalledVersion', 'Severity')),
                    'Incomplete vulnerability record')
            vulnerabilities.append(item)
    return vulnerabilities


def scan(edition, image_id, directory):
    report_path = directory / f'{edition}.json'
    # Local ignore files, configuration and TRIVY_* overrides must not weaken CI.
    environment = {key: value for key, value in os.environ.items()
                   if not key.startswith('TRIVY_')}
    subprocess.run([
        'trivy', 'image', '--scanners', 'vuln', '--image-src', 'docker',
        '--config', str(directory / 'empty.yaml'),
        '--ignorefile', str(directory / 'empty.ignore'),
        '--format', 'json', '--output', str(report_path), '--list-all-pkgs',
        '--timeout', '10m', image_id,
    ], cwd=directory, env=environment, check=True)
    items = findings(json.loads(report_path.read_text()), image_id)
    fixed = [item for item in items if item.get('FixedVersion', '').strip()]
    summary = dict(Counter(item['Severity'] for item in items))
    print(f'{edition}: {len(items)} package/CVE findings {summary}; '
          f'{len(fixed)} have fixes; full report: {report_path}', flush=True)
    for item in fixed:
        print(f"BLOCK {edition}: {item['VulnerabilityID']} {item['PkgName']} "
              f"{item['InstalledVersion']} -> {item['FixedVersion']} ({item['Severity']})",
              flush=True)
    return {'image_id': image_id, 'findings': len(items), 'by_severity': summary,
            'fixable': len(fixed), 'report': report_path.name}


def main():
    parent = ROOT / 'target/ci/image-security'
    parent.mkdir(parents=True, exist_ok=True)
    directory = Path(tempfile.mkdtemp(prefix='run-', dir=parent))
    (directory / 'empty.yaml').write_text('{}\n')
    (directory / 'empty.ignore').write_text('')
    # Pin IDs before either scan, so mutable local tags cannot change its subject.
    images = {edition: output('docker', 'image', 'inspect', '--format', '{{.Id}}', image)
              for edition, (image, _) in IMAGES.items()}
    summaries, errors = {}, []
    for edition, image_id in images.items():
        try:
            summaries[edition] = scan(edition, image_id, directory)
        except (OSError, ValueError, RuntimeError, subprocess.CalledProcessError) as error:
            errors.append(f'{edition}: {error}')
    (directory / 'summary.json').write_text(json.dumps(
        {'editions': summaries, 'errors': errors}, indent=2) + '\n')
    (directory / 'scanner.json').write_text(output('trivy', '--version', '--format', 'json'))
    require(not errors, 'Image scan failed: ' + '; '.join(errors))
    require(not any(result['fixable'] for result in summaries.values()),
            'Fixable image vulnerabilities block this candidate')
    print('PASS: no fixable image vulnerabilities. Unfixed findings remain in the reports; '
          'this is not a zero-CVE or exploitability claim.', flush=True)


if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError, RuntimeError, subprocess.CalledProcessError) as error:
        print(error, file=sys.stderr)
        raise SystemExit(1)
