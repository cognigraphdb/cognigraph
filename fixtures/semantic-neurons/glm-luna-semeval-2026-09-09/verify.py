"""Offline replay, evidence integrity and independently sourced gold checks."""
import copy
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import sys
import platform
from common import ROOT,REPO,digest,save
from prepare import parse,rank
import prepare as preparation
from evaluate import evaluate


def main():
    protocol=json.loads((ROOT/'protocol.json').read_text())
    for name,h in protocol['code_sha256'].items():
        assert digest(REPO/name)==h,name
    assert digest(REPO/'target/release/cognigraph-server')==protocol['binary_sha256']
    for name,h in protocol['source_sha256'].items():
        assert digest(ROOT/'source'/name)==h,name
    assert digest(ROOT/'corpus.json')==protocol['corpus_sha256']
    assert digest(ROOT/'data-quality.json')==protocol['data_quality_sha256']
    first=ROOT/'attempt-1'
    old_protocol=json.loads((first/'protocol.json').read_text())
    old_manifest=json.loads((first/'live/manifest.json').read_text())
    assert old_manifest['protocol_sha256']==digest(first/'protocol.json')
    for name,h in old_protocol['code_sha256'].items():
        path=first/'run.py' if name==str((ROOT/'run.py').relative_to(REPO)) else REPO/name
        assert digest(path)==h,name
    for name,h in old_manifest['artifact_sha256'].items():
        assert digest(first/'live'/name)==h,name
    assert not old_manifest['finished'] and len(old_manifest['attempts'])==4
    assert len(json.loads((first/'live/provider-calls.json').read_text()))==5
    corpus=json.loads((ROOT/'corpus.json').read_text())
    source={c['id']:c for c in parse(ROOT/'source/test.txt')}
    legal={'Other'} | {r['relation']+direction for r in corpus['taxonomy'] for direction in ('(e1,e2)','(e2,e1)')}
    assert all(c['label'] in legal for c in source.values())
    with tempfile.TemporaryDirectory(prefix='cg-semeval-source-replay-') as temp:
        temp=Path(temp);shutil.copytree(ROOT/'source',temp/'source')
        original_root=preparation.ROOT
        try:
            preparation.ROOT=temp;preparation.prepare()
        finally:
            preparation.ROOT=original_root
        assert digest(temp/'corpus.json')==protocol['corpus_sha256']
        assert digest(temp/'data-quality.json')==protocol['data_quality_sha256']
    assert all(c==source[c['id']] for c in corpus['cases'])
    assert len({c['text'].casefold() for c in corpus['cases']})==600
    assert [rank(c,'test') for c in corpus['cases']]==sorted(rank(c,'test') for c in corpus['cases'])
    report=evaluate(ROOT/'live')
    assert report==json.loads((ROOT/'results.json').read_text()),'Score replay drift'
    assert report['all_planned_requests_completed']
    assert all(not a['unknown_chunk_nominations'] for a in report['arms'].values())
    # Compare against the dataset's official scorer where every prediction is a legal label.
    official=[]
    with tempfile.TemporaryDirectory(prefix='cg-semeval-official-') as temp:
        temp=Path(temp)
        for name,arm in report['arms'].items():
            for rep in (1,2):
                rows=[r for r in arm['case_ledger'] if r['repetition']==rep]
                for field in ('raw','accepted'):
                    for kind in ('gold',field):
                        (temp/kind).write_text(''.join(f"{r['id'].removeprefix('semeval-')}\t{r[kind]}\n" for r in sorted(rows,key=lambda r:int(r['id'].removeprefix('semeval-'))) if kind=='gold' or r[kind] not in ['INVALID','FAILED']))
                    result=subprocess.run(['perl',str(ROOT/'source/official-scorer.pl'),str(temp/field),str(temp/'gold')],capture_output=True,text=True,check=True)
                    match=re.search(r'official score.*macro-averaged F1 = ([0-9.]+)%',result.stdout)
                    assert match,result.stdout[-1000:]
                    expected=arm['repetitions'][rep-1][field]['macro_f1_nine_relations']*100
                    assert abs(float(match[1])-expected)<0.0051,(field,match[1],expected)
                    official.append({'arm':name,'repetition':rep,'field':field,'official_macro_f1_percent':float(match[1]),'skipped_invalid_or_failed_predictions':sum(r[field] in ['INVALID','FAILED'] for r in rows)})
    # Tamper copies, never captures or the frozen corpus. Rehash two inner payloads
    # to demonstrate semantic validation beyond an outer file digest.
    probes=[]
    for name in ['outer_digest','protocol_digest','gold_in_prompt','evidence_span','missing_attempt']:
        with tempfile.TemporaryDirectory(prefix='cg-semeval-integrity-') as temp:
            target=Path(temp)/'live';shutil.copytree(ROOT/'live',target)
            manifest=json.loads((target/'manifest.json').read_text())
            if name=='outer_digest':
                p=target/'provider-calls.json';p.write_text(p.read_text()+' ')
            elif name=='protocol_digest':
                manifest['protocol_sha256']='0'*64
            elif name=='missing_attempt':
                manifest['attempts'].pop()
            elif name=='gold_in_prompt':
                p=target/'provider-calls.json';calls=json.loads(p.read_text())
                calls[0]['request_to_provider']['messages'][0]['content']+=' GOLD LABEL LEAK'
                save(p,calls);manifest['artifact_sha256'][p.name]=digest(p)
            else:
                p=next(p for p in sorted(target.glob('*-r1-*.json')) if json.loads(p.read_text())['facts'])
                observation=json.loads(p.read_text());observation['facts'][0]['trigger_end']+=1
                save(p,observation);manifest['artifact_sha256'][p.name]=digest(p)
            save(target/'manifest.json',manifest)
            try:
                evaluate(target)
            except (AssertionError,KeyError,ValueError,FileNotFoundError):
                probes.append({'probe':name,'rejected':True})
            else:
                raise AssertionError('Tampered capture was accepted: '+name)
    result={'status':'passed','python_runtime':sys.version,'platform':platform.platform(),'code_files_verified':len(protocol['code_sha256']),
        'source_members_verified':len(protocol['source_sha256']),'human_gold_rows_verified':600,
        'binary_sha256':protocol['binary_sha256'],'live_requests':202,
        'accepted_occurrences_verified':sum(a['verified_evidence_occurrences'] for a in report['arms'].values()),
        'exact_score_replay':True,'official_scorer_comparisons':official,'negative_integrity_probes':probes,
        'verifier_sha256':digest(Path(__file__)),'results_sha256':digest(ROOT/'results.json')}
    save(ROOT/'validation.json',result)
    print(json.dumps({k:v for k,v in result.items() if k not in ['official_scorer_comparisons']},indent=2))

if __name__=='__main__':main()
