"""CG-27 offline artifact and validation replay; never calls a model or scores test.

Build: cargo build --release -p cognigraph-construct --bin webnlg-pilot \
  --bin webnlg-mine-rules --bin webnlg-score --bin webnlg-llm-run
Run: python3 docs/issues/evidence/webnlg-status-replay.py --output /tmp/cg27.json
Prepared data must already exist. Verification reads retained test-file hashes
and alignment; scoring/mining receive a separate root containing only authoring
files. Stored proposals are loaded explicitly, with no provider credentials.
"""
import argparse
from collections import Counter
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile

REPO = Path(__file__).resolve().parents[3]
ARTIFACTS = REPO / 'crates/cognigraph-construct/fixtures/webnlg'


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=REPO / 'data/webnlg-pilot')
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    data = args.root.resolve()
    scratch = Path(tempfile.mkdtemp(prefix='cognigraph-cg27-replay-'))
    print('Logs and temporary authoring root:', scratch, flush=True)
    snapshots = {str(p): sha(p) for p in sorted(data.rglob('*')) if p.is_file()}
    assert snapshots and (data / 'run.json').is_file(), 'prepared data is required'
    artifact_hashes = {p.name: sha(p) for p in sorted(ARTIFACTS.glob('*.json'))}
    raw = json.loads((ARTIFACTS / 'llm-proposals.json').read_text())
    pruned = json.loads((ARTIFACTS / 'llm-proposals-pruned.json').read_text())
    mined = json.loads((ARTIFACTS / 'neuron-ruleset.mined.json').read_text())
    assert raw.keys() == pruned.keys()
    assert all(not (Counter(pruned[k]) - Counter(raw[k])) for k in raw)
    counts = {'mined_predicates': len(mined['predicates']),
        'mined_templates': sum(map(len, mined['predicates'].values())),
        'proposal_predicates': len(raw), 'raw_entries': sum(map(len, raw.values())),
        'raw_unique_within_predicate': sum(len(set(v)) for v in raw.values()),
        'pruned_entries': sum(map(len, pruned.values()))}
    counts['removed_entries'] = counts['raw_entries'] - counts['pruned_entries']
    assert counts == {'mined_predicates':303,'mined_templates':1842,'proposal_predicates':25,
        'raw_entries':248,'raw_unique_within_predicate':247,'pruned_entries':202,'removed_entries':46}
    authoring = scratch / 'authoring'
    for kind in ('documents', 'oracle'):
        (authoring / kind).mkdir(parents=True)
        for split in ('train', 'validation'):
            shutil.copy2(data / kind / (split + '.jsonl'), authoring / kind / (split + '.jsonl'))
    assert not list(authoring.rglob('test*'))
    env = {k: os.environ[k] for k in ('PATH','HOME','TMPDIR') if k in os.environ}

    def run(label, binary, params):
        executable = REPO / 'target/release' / binary
        command = [str(executable), *map(str, params)]
        log = scratch / (label + '.log')
        with log.open('w') as stream:
            result = subprocess.run(command, cwd=scratch, env=env, stdout=stream,
                stderr=subprocess.STDOUT, timeout=900)
        assert result.returncode == 0, (label, result.returncode, str(log))
        print(label + ' passed', flush=True)
        return {'command': command, 'exit_code': result.returncode,
            'binary_sha256': sha(executable), 'log': str(log), 'log_sha256': sha(log),
            'output': log.read_text()}

    results = {}
    results['verify'] = run('verify', 'webnlg-pilot', ['--verify-only','--output',data])
    results['mine'] = run('mine', 'webnlg-mine-rules', ['--root',authoring,'--out',scratch/'mined.json'])
    assert (scratch/'mined.json').read_bytes() == (ARTIFACTS/'neuron-ruleset.mined.json').read_bytes()
    jobs = [('score','webnlg-score',['--root',authoring]),
        ('raw','webnlg-llm-run',['--root',authoring,'--load',ARTIFACTS/'llm-proposals.json']),
        ('pruned','webnlg-llm-run',['--root',authoring,'--load',ARTIFACTS/'llm-proposals-pruned.json'])]
    with ThreadPoolExecutor(max_workers=3) as pool:
        pending = [(name,pool.submit(run,name,binary,params)) for name,binary,params in jobs]
        for name,future in pending:
            results[name] = future.result()
    for p,digest in snapshots.items():
        assert sha(Path(p)) == digest, 'prepared input changed: ' + p
    for name,digest in artifact_hashes.items():
        assert sha(ARTIFACTS/name) == digest, 'retained artifact changed: ' + name
    report = {'issue':'CG-27','revision':subprocess.check_output(['git','rev-parse','HEAD'],cwd=REPO,text=True).strip(),
        'harness_sha256':sha(Path(__file__)),'artifact_sha256':artifact_hashes,'counts':counts,
        'prepared_files_verified_unchanged':len(snapshots),'prepared_file_sha256':snapshots,
        'test_files_present_in_scoring_root':False,'mined_artifact_byte_identical':True,
        'provider_credentials_passed':False,'model_calls':0,'test_scoring_runs':0,'runs':results,
        'limits':['Replays retained candidates on validation; not a new model benchmark or untouched holdout evaluation.',
            'Historical one-shot test score remains ledger evidence; test was not scored.',
            'Pruned JSON contains templates without signed reviewer identity or runtime neuron-review events.']}
    args.output.write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps({'counts':counts,'prepared_files_verified_unchanged':len(snapshots),'runs':len(results)}),flush=True)


if __name__ == '__main__':
    main()
