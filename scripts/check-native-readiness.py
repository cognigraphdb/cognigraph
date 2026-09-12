#!/usr/bin/env python3
"""Qualify Native release binaries with owned stores and a local embedding stub.

Reuses the sealed CG-67 protocol, adding current startup rejection probes.
The output must be a new path. No existing data or environment file is changed.
"""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import threading
from http.server import ThreadingHTTPServer

ROOT = Path(__file__).resolve().parents[1]
PROTOCOL = ROOT / 'docs/issues/evidence/native-runtime-http.py'
spec = importlib.util.spec_from_file_location('native_protocol', PROTOCOL)
protocol = importlib.util.module_from_spec(spec)
spec.loader.exec_module(protocol)


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def rejected_configuration(binary, edition, root):
    cases = [
        ('bootstrap-without-auth', {'COGNIGRAPH_ADMIN_PASSWORD': 'synthetic'}, 'COGNIGRAPH_AUTH_ENABLED=true'),
        ('bootstrap-without-jwt', {'COGNIGRAPH_AUTH_ENABLED': 'true', 'COGNIGRAPH_ADMIN_PASSWORD': 'synthetic'}, 'COGNIGRAPH_JWT_SECRET'),
        ('invalid-listen-address', {'COGNIGRAPH_PORT': 'invalid'}, 'Invalid listen address'),
    ]
    if edition == 'community':
        cases += [('enterprise-directory', {'COGNIGRAPH_DATA_DIR': str(root / 'tenants')}, 'enterprise_feature_required'),
                  ('enterprise-cas', {'COGNIGRAPH_ARTIFACT_SOURCE': 'local-cas'}, 'enterprise_feature_required')]
    else:
        cases += [('invalid-completion-provider', {'COGNIGRAPH_COMPLETION_PROVIDER': 'invalid'}, 'Invalid completion configuration'),
                  ('invalid-judge-key', {'COGNIGRAPH_JUDGE_MODEL': 'synthetic-model'}, 'Invalid judge configuration'),
                  ('missing-cas-root', {'COGNIGRAPH_ARTIFACT_SOURCE': 'local-cas'}, 'COGNIGRAPH_ARTIFACT_CAS_ROOT'),
                  ('relative-cas-root', {'COGNIGRAPH_ARTIFACT_SOURCE': 'local-cas', 'COGNIGRAPH_ARTIFACT_CAS_ROOT': 'relative'}, 'must be absolute')]
    records = []
    for name, config, message in cases:
        directory = root / name
        directory.mkdir()
        env = {**protocol.probe.ENV, 'COGNIGRAPH_NATIVE_PATH': str(directory / 'store.redb'), **config}
        result = subprocess.run([binary], cwd=directory, env=env, text=True, capture_output=True, timeout=15)
        assert result.returncode != 0 and message in result.stderr, (name, result.returncode, result.stderr)
        assert not list(directory.iterdir()), (name, 'startup rejection wrote files')
        assert not (root / 'tenants').exists(), 'rejected Community startup created tenant storage'
        records.append({'case': name, 'exit_nonzero': True, 'expected_error': message, 'no_storage_created': True})
    return records


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--edition', choices=('community', 'enterprise'), required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if args.output.exists():
        parser.error('output exists; choose a new capture path')
    binary = args.binary.resolve()
    fixture = ThreadingHTTPServer(('127.0.0.1', 0), protocol.probe.Embeddings)
    thread = threading.Thread(target=fixture.serve_forever, daemon=True)
    thread.start()
    report = {'issue': 'CG-68', 'edition': args.edition,
              'base_commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
              'binary_sha256': digest(binary), 'harness_sha256': digest(Path(__file__)),
              'protocol_sha256': digest(PROTOCOL), 'helper_sha256': digest(protocol.HELPER),
              'provider': 'deterministic loopback embedding stub; no hosted calls', 'runs': []}
    try:
        with tempfile.TemporaryDirectory(prefix='cg68-native-') as name:
            root = Path(name)
            report['configuration'] = rejected_configuration(binary, args.edition, root)
            for mode in protocol.MODES:
                directory = root / mode
                directory.mkdir()
                result = protocol.verify(binary, args.edition, mode, directory, f'http://127.0.0.1:{fixture.server_port}/v1')
                report['runs'].append(result)
                print(args.edition, mode, result['checks'], 'checks passed', flush=True)
    finally:
        fixture.shutdown()
        fixture.server_close()
        thread.join()
    with args.output.open('x') as output:
        output.write(json.dumps(report, indent=2) + '\n')
    print('PASS', args.edition, sum(r['checks'] for r in report['runs']), 'runtime checks,',
          len(report['configuration']), 'startup rejection cases;', args.output, flush=True)


if __name__ == '__main__':
    main()
