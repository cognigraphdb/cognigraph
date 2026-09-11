"""Paired, interleaved, frozen benchmark through isolated real release servers."""
import argparse
from datetime import datetime,timezone
import json
from pathlib import Path
from common import ROOT,REPO,Server,digest,save
from adapter import Recorder


def utc():
    return datetime.now(timezone.utc).isoformat().replace('+00:00','Z')


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--mode',choices=['mock','live'],required=True)
    p.add_argument('--output',type=Path,required=True)
    args=p.parse_args()
    protocol=json.loads((ROOT/'protocol.json').read_text())
    for name,h in protocol['code_sha256'].items():
        assert digest(REPO/name)==h,name
    assert digest(ROOT/'corpus.json')==protocol['corpus_sha256']
    binary=REPO/'target/release/cognigraph-server'
    assert digest(binary)==protocol['binary_sha256']
    assert not args.output.exists(),'Never overwrite an attempt'
    args.output.mkdir(parents=True)
    corpus=json.loads((ROOT/'corpus.json').read_text())
    manifest={'mode':args.mode,'started_utc':utc(),'finished':False,'protocol_sha256':digest(ROOT/'protocol.json'),
              'attempts':[],'halted_arms':{},'binary_sha256':digest(binary)}
    recorder=Recorder(args.output,protocol,args.mode)
    failures={a['id']:0 for a in protocol['arms']}
    schedule=[('preflight',0,0,corpus['preflight_cases'])]
    schedule += [('measurement',rep,b//12+1,corpus['cases'][b:b+12]) for rep in (1,2) for b in range(0,600,12)]
    try:
        for phase,rep,batch,cases in schedule:
            arms=list(protocol['arms'])
            if (rep+batch)%2:
                arms.reverse()
            for arm in arms:
                if arm['id'] in manifest['halted_arms']:
                    continue
                # Provider-visible context is constructed from explicit input fields only.
                public_cases=[{k:c[k] for k in ('id','title','text','pair')} for c in cases]
                recorder.context={'phase':phase,'arm':arm['id'],'repetition':rep,'batch':batch,
                                  'cases': cases if args.mode=='mock' else public_cases}
                server=Server(binary,arm['model'],recorder)
                try:
                    space=f"semeval-{arm['id']}-r{rep}-b{batch}"
                    body={'space_type':space,'taxonomy':corpus['taxonomy'],
                          'chunks':[{k:c[k] for k in ('id','title','text')} for c in cases]}
                    status,response,seconds=server.call('/api/construct/directed',body)
                    # Always inspect store, including failures; fresh store prevents stale facts.
                    observation={'phase':phase,'arm':arm['id'],'repetition':rep,'batch':batch,
                        'request':body,'status':status,'response':response,'latency_seconds':seconds,
                        'facts':server.rows('facts'),'entities':server.rows('entities'),'chunks':server.rows('chunks')}
                    name=f"{arm['id']}-r{rep}-b{batch}.json"
                    save(args.output/name,observation); manifest['attempts'].append(name)
                    raw=json.loads(recorder.records[-1]['raw_response'])
                    ok=status==200 and raw.get('model') in arm['accepted_returned_models'] and raw.get('choices',[{}])[0].get('finish_reason')=='stop'
                    failures[arm['id']]=0 if ok else failures[arm['id']]+1
                    if not ok and (phase=='preflight' or failures[arm['id']]>=3):
                        manifest['halted_arms'][arm['id']]={'phase':phase,'repetition':rep,'batch':batch,'status':status}
                    print(f"{arm['id']} {phase} r{rep} b{batch}: HTTP {status}; complete={ok}",flush=True)
                    save(args.output/'manifest.json',manifest)
                finally:
                    server.close()
        manifest['finished']=True
    finally:
        recorder.close()
        manifest.update(finished_utc=utc(),reserved_usd=recorder.reserved,
            artifact_sha256={p.name:digest(p) for p in sorted(args.output.glob('*.json')) if p.name!='manifest.json'})
        save(args.output/'manifest.json',manifest)

if __name__=='__main__':
    main()
