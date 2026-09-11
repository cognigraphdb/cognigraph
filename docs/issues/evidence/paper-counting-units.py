"""Recount CG-24 fixture units and replay stored construction without any model calls.

Build first: cargo build --release -p cognigraph-construct --example blind_recheck
Run with a separately authorized private kit directory:
python3 docs/issues/evidence/paper-counting-units.py --fixture-root /path/to/blind --output /tmp/cg24-counts.json
The existing Rust reviewer treats supplied proposals as accepted in disposable
in-memory stores. It does not generate proposals or alter the source fixtures.
"""
import argparse
import hashlib
import itertools
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile

REPO = Path(__file__).resolve().parents[3]
KITS = [
    ('qualitative-research-software-market-size', 15, 12, 10),
]


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def fact(raw):
    source, rest = raw.split('--', 1)
    relation, target = rest.split('-->', 1)
    return tuple(s.strip() for s in (source, relation, target))


def units(spec, field):
    mentions = [fact(raw) for q in spec['questions'] for raw in q[field]]
    return mentions, [key for key, _ in itertools.groupby(mentions)], set(mentions)


def replay(binary, kit, neurons, expected, forbidden):
    env = {key: os.environ[key] for key in ('PATH', 'HOME', 'TMPDIR') if key in os.environ}
    run = subprocess.run([str(binary), str(kit), str(neurons)], cwd=REPO, env=env,
        capture_output=True, text=True, check=True, timeout=60)
    match = re.search(r'^recall (\d+)/(\d+)  restraint violations (\d+)/(\d+)$', run.stdout, re.M)
    assert match, run.stdout
    found, total, violated, traps = map(int, match.groups())
    assert total == len(expected[2]) and traps == len(forbidden[2])
    missing = {fact(line.removeprefix('  STILL MISSING  ')) for line in run.stdout.splitlines()
        if line.startswith('  STILL MISSING  ')}
    assert missing <= expected[2] and found == total - len(missing)
    return {'exit_code': 0, 'expected_found': found, 'expected_total': total,
        'forbidden_triggered': violated, 'forbidden_total': traps,
        'legacy_adjacent_dedup_found': sum(f not in missing for f in expected[1]),
        'legacy_adjacent_dedup_total': len(expected[1]),
        'stdout_sha256': hashlib.sha256(run.stdout.encode()).hexdigest()}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=REPO / 'target/release/examples/blind_recheck')
    parser.add_argument('--fixture-root', type=Path, required=True,
                        help='External directory containing the privately retained research kits')
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    binary = args.binary.resolve()
    root = args.fixture_root.resolve()
    inputs, rows = {}, []
    with tempfile.TemporaryDirectory(prefix='cognigraph-cg24-counts-') as scratch:
        empty = Path(scratch) / 'empty.neurons.json'
        empty.write_text(json.dumps({'space_type': 'offline-counting-control', 'neurons': []}))
        for name, expected_total, recovered, forbidden_total in KITS:
            kit, gap = root / name, root / (name + '-gap')
            spec = json.loads((kit / 'eval.json').read_text())
            gap_spec = json.loads((gap / 'eval.json').read_text())
            assert spec['questions'] == gap_spec['questions']
            assert (kit / 'chunks.jsonl').read_bytes() == (gap / 'chunks.jsonl').read_bytes()
            expected, forbidden = units(spec, 'expected_facts'), units(spec, 'forbidden_facts')
            assert (len(expected[2]), len(forbidden[2])) == (expected_total, forbidden_total)
            for directory in (kit, gap):
                for p in sorted(directory.glob('*.json*')):
                    inputs[str(p.relative_to(root))] = sha(p)
            base = replay(binary, kit, empty, expected, forbidden)
            lesion = replay(binary, gap, empty, expected, forbidden)
            repaired = replay(binary, gap, gap / 'proposed.neurons.json', expected, forbidden)
            assert [r['expected_found'] for r in (base, lesion, repaired)] == [expected_total, 0, recovered]
            assert all(r['forbidden_triggered'] == 0 for r in (base, lesion, repaired))
            rows.append({'kit': name, 'questions': len(spec['questions']),
                'chunks': len((kit / 'chunks.jsonl').read_text().splitlines()),
                'expected_mentions': len(expected[0]), 'forbidden_mentions': len(forbidden[0]),
                'legacy_expected_after_adjacent_dedup': len(expected[1]),
                'legacy_forbidden_after_adjacent_dedup': len(forbidden[1]),
                'distinct_expected': len(expected[2]), 'distinct_forbidden': len(forbidden[2]),
                'cold': base, 'lesion': lesion, 'stored_proposals': repaired})
    totals = {key: sum(row[key] for row in rows) for key in ('questions', 'chunks',
        'expected_mentions', 'forbidden_mentions', 'legacy_expected_after_adjacent_dedup',
        'legacy_forbidden_after_adjacent_dedup', 'distinct_expected', 'distinct_forbidden')}
    totals['cold_found'] = sum(r['cold']['expected_found'] for r in rows)
    totals['lesion_found'] = sum(r['lesion']['expected_found'] for r in rows)
    totals['repaired_found'] = sum(r['stored_proposals']['expected_found'] for r in rows)
    totals['legacy_repaired_found'] = sum(r['stored_proposals']['legacy_adjacent_dedup_found'] for r in rows)
    assert (totals['expected_mentions'], totals['forbidden_mentions'], totals['legacy_expected_after_adjacent_dedup'],
        totals['distinct_expected'], totals['distinct_forbidden'], totals['cold_found'], totals['lesion_found'],
        totals['repaired_found'], totals['legacy_repaired_found']) == (17, 10, 17, 15, 10, 15, 0, 12, 14)
    for p, digest in inputs.items():
        assert sha(root / p) == digest, 'fixture changed during replay: ' + p
    report = {'issue': 'CG-24', 'revision': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=REPO, text=True).strip(),
        'binary_sha256': sha(binary), 'harness_sha256': sha(Path(__file__)), 'runtime_replays': 3,
        'counting_unit': 'Distinct parsed (source, relation, target) triples within each kit, summed across the retained space; not globally deduplicated across spaces',
        'legacy_unit': 'Question mentions with only consecutive duplicates removed; 17 raw expected mentions remain 17 legacy construction entries',
        'totals': totals, 'kits': rows, 'input_sha256': inputs,
        'limits': ['No LLM calls or new benchmark; stored proposals accepted only in disposable in-memory replay.',
            'Historical answer scores are not rerun; their denominators count per-question fact mentions.',
            'This verifies fixture counting and current symbolic replay, not independent corpus validity or model qualification.']}
    args.output.write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({'runtime_replays': 3, 'totals': totals}, indent=2))


if __name__ == '__main__':
    main()
