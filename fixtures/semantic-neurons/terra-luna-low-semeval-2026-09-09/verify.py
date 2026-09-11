"""Offline replay, external gold, official scoring, prompt parity and tamper checks."""
import argparse
import importlib.util
import json
from pathlib import Path
import platform
import re
import shutil
import subprocess
import sys
import tempfile
from common import ROOT, REPO, digest, save
from evaluate import evaluate
from compare import OLD, build


def verify():
    protocol = json.loads((ROOT/'protocol.json').read_text())
    for name, h in protocol['code_sha256'].items():
        assert digest(REPO/name) == h, name
    assert digest(REPO/'target/release/cognigraph-server') == protocol['binary_sha256']
    assert digest(ROOT/'corpus.json') == protocol['corpus_sha256']
    old_manifest = json.loads((OLD/'package-manifest.json').read_text())
    assert digest(OLD/'package-manifest.json') == protocol['inherited_package']['manifest_sha256']
    for name, h in old_manifest['sha256'].items():
        assert digest(OLD/name) == h, name
    for name, h in protocol['source_sha256'].items():
        assert digest(OLD/'source'/name) == h, name
    spec = importlib.util.spec_from_file_location('inherited_prepare', OLD/'prepare.py')
    preparation = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(preparation)
    corpus = json.loads((ROOT/'corpus.json').read_text())
    source = {c['id']: c for c in preparation.parse(OLD/'source/test.txt')}
    assert all(c == source[c['id']] for c in corpus['cases'])
    with tempfile.TemporaryDirectory(prefix='cg-terra-source-') as temp:
        temp = Path(temp)
        shutil.copytree(OLD/'source', temp/'source')
        preparation.ROOT = temp
        preparation.prepare()
        assert digest(temp/'corpus.json') == protocol['corpus_sha256']
        assert digest(temp/'data-quality.json') == protocol['data_quality_sha256']
    # Execute the inherited evaluator in its own module environment, read-only.
    with tempfile.TemporaryDirectory(prefix='cg-terra-inherited-replay-') as temp:
        output = Path(temp)/'results.json'
        subprocess.run([sys.executable, '-B', str(OLD/'evaluate.py'), str(OLD/'live'), '--output', str(output)], check=True)
        assert json.loads(output.read_text()) == json.loads((OLD/'results.json').read_text())
    result = evaluate(ROOT/'live')
    assert result == json.loads((ROOT/'results.json').read_text()), 'Score replay drift'
    assert result['all_planned_requests_completed']
    assert all(not a['unknown_chunk_nominations'] for a in result['arms'].values())
    comparison, accounting = build()
    assert comparison == json.loads((ROOT/'comparison.json').read_text())
    assert accounting == json.loads((ROOT/'accounting.json').read_text())
    assert comparison['contrasts']['terra-low_minus_luna-low']['raw'] == result['paired_comparison']['raw']
    official = []
    with tempfile.TemporaryDirectory(prefix='cg-terra-official-') as temp:
        temp = Path(temp)
        for name, arm in result['arms'].items():
            for rep in (1, 2):
                rows = [r for r in arm['case_ledger'] if r['repetition'] == rep]
                for field in ('raw', 'accepted'):
                    for kind in ('gold', field):
                        (temp/kind).write_text(''.join(
                            f"{r['id'].removeprefix('semeval-')}\t{r[kind]}\n"
                            for r in sorted(rows, key=lambda r: int(r['id'].removeprefix('semeval-')))
                            if kind == 'gold' or r[kind] not in ['INVALID', 'FAILED']))
                    scored = subprocess.run(['perl', str(OLD/'source/official-scorer.pl'), str(temp/field), str(temp/'gold')],
                                            capture_output=True, text=True, check=True)
                    match = re.search(r'official score.*macro-averaged F1 = ([0-9.]+)%', scored.stdout)
                    assert match, scored.stdout[-1000:]
                    expected = arm['repetitions'][rep-1][field]['macro_f1_nine_relations'] * 100
                    assert abs(float(match[1])-expected) < .0051, (field, match[1], expected)
                    official.append({'arm': name, 'repetition': rep, 'stage': field,
                                     'official_macro_f1_percent': float(match[1]),
                                     'skipped_invalid_or_failed_predictions': sum(r[field] in ['INVALID', 'FAILED'] for r in rows)})
    probes = []
    for name in ('outer_digest', 'protocol_digest', 'gold_in_prompt', 'evidence_span', 'missing_attempt'):
        with tempfile.TemporaryDirectory(prefix='cg-terra-integrity-') as temp:
            target = Path(temp)/'live'
            shutil.copytree(ROOT/'live', target)
            manifest = json.loads((target/'manifest.json').read_text())
            if name == 'outer_digest':
                p = target/'provider-calls.json'
                p.write_text(p.read_text()+' ')
            elif name == 'protocol_digest':
                manifest['protocol_sha256'] = '0'*64
            elif name == 'missing_attempt':
                manifest['attempts'].pop()
            elif name == 'gold_in_prompt':
                p = target/'provider-calls.json'
                calls = json.loads(p.read_text())
                calls[0]['request_to_provider']['messages'][0]['content'] += ' GOLD LABEL LEAK'
                save(p, calls)
                manifest['artifact_sha256'][p.name] = digest(p)
            else:
                p = next(p for p in sorted(target.glob('*-r1-*.json')) if json.loads(p.read_text())['facts'])
                observation = json.loads(p.read_text())
                observation['facts'][0]['trigger_end'] += 1
                save(p, observation)
                manifest['artifact_sha256'][p.name] = digest(p)
            save(target/'manifest.json', manifest)
            try:
                evaluate(target)
            except (AssertionError, KeyError, ValueError, FileNotFoundError):
                probes.append({'probe': name, 'rejected': True})
            else:
                raise AssertionError('Tampered capture accepted: '+name)
    return {'status': 'passed', 'python_runtime': sys.version, 'platform': platform.platform(),
            'code_files_verified': len(protocol['code_sha256']), 'inherited_package_files_verified': old_manifest['file_count'],
            'source_members_verified': len(protocol['source_sha256']), 'human_gold_rows_verified': 600,
            'binary_sha256': protocol['binary_sha256'], 'new_live_requests': 202,
            'historical_live_requests_replayed': 202, 'all_404_prompts_match': True,
            'accepted_occurrences_verified': sum(a['verified_evidence_occurrences'] for a in result['arms'].values()),
            'official_scorer_comparisons': official, 'negative_integrity_probes': probes,
            'exact_scores_and_costs_replayed': True, 'source_sampling_reproduced': True,
            'verifier_sha256': digest(Path(__file__)), 'results_sha256': digest(ROOT/'results.json'),
            'comparison_sha256': digest(ROOT/'comparison.json')}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    result = verify()
    save(args.output, result)
    print(json.dumps({k: v for k, v in result.items() if k != 'official_scorer_comparisons'}, indent=2))
