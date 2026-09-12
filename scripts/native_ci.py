#!/usr/bin/env python3
"""Build and qualify both Native release editions; preserve each run's diagnostics."""
import argparse
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
MODES = {'memory', 'resident-embedded', 'resident-sidecar', 'paged-sidecar'}


def build(edition, directory, log_directory):
    command = ['cargo', 'build', '--locked', '--release', '--no-default-features',
               '-p', 'cognigraph-server', '--message-format=json-render-diagnostics']
    if edition == 'enterprise':
        command += ['--features', 'enterprise']
    with (log_directory / f'{edition}-build.jsonl').open('w') as messages, \
         (log_directory / f'{edition}-build.log').open('w') as log:
        subprocess.run(command, cwd=ROOT, stdout=messages, stderr=log, check=True)
    artifacts = [json.loads(line) for line in
                 (log_directory / f'{edition}-build.jsonl').read_text().splitlines()]
    paths = [item['executable'] for item in artifacts
             if item.get('reason') == 'compiler-artifact' and item.get('executable')
             and item['target']['name'] == 'cognigraph-server']
    if len(paths) != 1:
        raise RuntimeError('Cargo did not report exactly one server executable')
    binary = directory / 'cognigraph-server'
    shutil.copy2(paths[0], binary)
    return binary


def validate_report(path, edition):
    report = json.loads(path.read_text())
    runs = report.get('runs', [])
    if report.get('edition') != edition or {run.get('mode') for run in runs} != MODES \
            or len(runs) != len(MODES) or not all(run.get('checks', 0) > 0 for run in runs):
        raise RuntimeError('Native report is missing edition or storage-mode coverage')
    expected = 5 if edition == 'community' else 7
    if len(report.get('configuration', [])) != expected:
        raise RuntimeError('Native report is missing startup rejection coverage')
    return {'checks': sum(run['checks'] for run in runs), 'startup_rejections': expected,
            'binary_sha256': report['binary_sha256']}


def run(output):
    output.mkdir(parents=True, exist_ok=True)
    captures = Path(tempfile.mkdtemp(prefix='run-', dir=output))
    summary = {'status': 'running', 'editions': {}, 'capture_directory': captures.name}
    print(f'Native release verification: {captures}', flush=True)
    try:
        for edition in ('community', 'enterprise'):
            print(f'BUILD + VERIFY Native {edition}', flush=True)
            with tempfile.TemporaryDirectory(prefix=f'cg-native-ci-{edition}-') as temporary:
                binary = build(edition, Path(temporary), captures)
                report = captures / f'{edition}.json'
                with (captures / f'{edition}-runtime.log').open('w') as log:
                    # -I keeps PYTHONOPTIMIZE/PYTHONPATH from changing assertion coverage.
                    subprocess.run([sys.executable, '-I', str(ROOT / 'scripts/check-native-readiness.py'),
                                    '--binary', str(binary), '--edition', edition,
                                    '--output', str(report)], cwd=ROOT, stdout=log,
                                   stderr=subprocess.STDOUT, check=True)
                summary['editions'][edition] = validate_report(report, edition)
                print(f'PASS Native {edition}: {summary["editions"][edition]}', flush=True)
        summary['status'] = 'passed'
    except Exception as error:
        summary['status'] = 'failed'
        summary['error'] = str(error)
        raise
    finally:
        (captures / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
        print(f'Native diagnostics retained in {captures}', flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, default=ROOT / 'target/ci/native')
    args = parser.parse_args()
    try:
        run(args.output.resolve())
    except (OSError, RuntimeError, ValueError, subprocess.CalledProcessError) as error:
        raise SystemExit(str(error)) from None
