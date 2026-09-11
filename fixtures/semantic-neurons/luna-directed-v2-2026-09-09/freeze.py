"""Seal the candidate protocol before either replay or live generation."""
from datetime import datetime, timezone
import json
import subprocess

from common import ROOT, BASE, REPO, digest, save


def main():
    assert not (ROOT / 'protocol.json').exists(), 'Candidate already frozen'
    assert not (ROOT / 'replay').exists() and not (ROOT / 'live').exists()
    old = json.loads((BASE / 'protocol.json').read_text())
    paths = set(old['code_sha256']) | {'crates/cognigraph-construct/tests/directed_contracts.rs',
                                      'docs/issues/evidence/directed-contract-fixes.py'}
    rust_paths = ['Cargo.lock', 'crates/cognigraph-construct/Cargo.toml',
        'crates/cognigraph-construct/src/directed.rs', 'crates/cognigraph-construct/tests/directed.rs',
        'crates/cognigraph-server/src/routes/construct/directed.rs']
    patch = subprocess.check_output(['git', 'diff', 'HEAD', '--', *rust_paths], cwd=REPO)
    (ROOT / 'candidate.patch').write_bytes(patch)
    new_test = REPO / 'crates/cognigraph-construct/tests/directed_contracts.rs'
    (ROOT / 'directed_contracts.rs').write_bytes(new_test.read_bytes())
    protocol = {k: old[k] for k in ['arm', 'pricing_source', 'pricing_checked_utc', 'input_separation',
        'source_scope', 'failure_policy', 'transport_policy', 'negative_policy']}
    protocol.update(schema='cognigraph-directed-v2-development-v1',
        frozen_utc=datetime.now(timezone.utc).isoformat(),
        base_commit=subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=REPO).decode().strip(),
        binary_sha256=digest(REPO / 'target/release/cognigraph-server'),
        code_sha256={name: digest(REPO / name) for name in sorted(paths)},
        baseline_manifest_sha256=digest(BASE / 'package-manifest.json'),
        package_sha256={p.name: digest(p) for p in sorted(ROOT.iterdir()) if p.is_file()},
        directed_policy='directed-policy-v2', planned_replay_documents=40,
        planned_development_calls=40, planned_holdout_calls=0, maximum_reserved_usd=1.0,
        candidate_changes=['Unicode word boundaries on both endpoints; quote lookup unchanged',
            'Exact request chunk-ID and taxonomy relation enums; local gates unchanged for identifiers',
            'Policy attribution v2; prompt/model/effort/taxonomy/input text unchanged'],
        replay_policy='Replay original provider choices unchanged through the new release gates; zero external calls and zero metered tokens.',
        inference_order='Verify fixed-output replay first, then one live pass on the same 40 development documents; no retries or holdout calls.')
    save(ROOT / 'protocol.json', protocol)
    print('Frozen candidate protocol:', digest(ROOT / 'protocol.json'))


if __name__ == '__main__':
    main()
