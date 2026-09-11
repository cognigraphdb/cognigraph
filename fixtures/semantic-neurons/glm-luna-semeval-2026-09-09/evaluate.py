"""Exact, directed supplied-pair scoring; protocol failures never count as abstention."""
from collections import Counter, defaultdict
import json
import random
import statistics
import unicodedata
from common import ROOT, digest, save, identity, metrics, previous_providers, usage_cost
from adapter import pair_input

INVALID='INVALID'
FAILED='FAILED'


def norm(s):
    return ' '.join(unicodedata.normalize('NFC',s).casefold().split())


def prediction(case, facts, relations, valid=True):
    if not valid:
        return FAILED
    selected=[f for f in facts if f['chunk_id']==case['id']]
    if not selected:
        return 'Other'
    if len(selected)!=1:
        return INVALID
    f=selected[0]; a,b=map(norm,case['pair'])
    pair=(norm(f['source']),norm(f['target']))
    if f['relation'] not in relations or pair not in [(a,b),(b,a)]:
        return INVALID
    return f['relation']+('(e1,e2)' if pair==(a,b) else '(e2,e1)')


def classification(rows, field, relations):
    by={}
    for relation in relations:
        tp=sum(r['gold']==r[field] and r['gold'].startswith(relation+'(') for r in rows)
        fp=sum(r[field].startswith(relation+'(') and r['gold']!=r[field] for r in rows)
        fn=sum(r['gold'].startswith(relation+'(') and r['gold']!=r[field] for r in rows)
        p=tp/(tp+fp) if tp+fp else 0
        recall=tp/(tp+fn) if tp+fn else 0
        by[relation]={'tp':tp,'fp':fp,'fn':fn,'precision':p,'recall':recall,'f1':2*p*recall/(p+recall) if p+recall else 0}
    negatives=[r for r in rows if r['gold']=='Other']
    correct=sum(r['gold']==r[field] for r in rows)
    return {'cases':len(rows),'correct':correct,'accuracy':correct/len(rows),
        'macro_f1_nine_relations':statistics.mean(b['f1'] for b in by.values()),
        'negative_cases':len(negatives),'correct_abstentions':sum(r[field]=='Other' for r in negatives),
        'false_assertions_on_other':sum(r[field] not in ['Other',FAILED] for r in negatives),
        'failed_cases':sum(r[field]==FAILED for r in rows),'invalid_pair_outputs':sum(r[field]==INVALID for r in rows),
        'direction_errors':sum(r['gold']!=r[field] and r['gold']!='Other' and r['gold'].split('(')[0]==r[field].split('(')[0] for r in rows),
        'by_relation':by,'confusion':[{'gold':g,'prediction':p,'count':n} for (g,p),n in sorted(Counter((r['gold'],r[field]) for r in rows).items())]}


def compare(arm_rows, field):
    left={ (r['repetition'],r['id']):r for r in arm_rows['luna-json']}
    right={ (r['repetition'],r['id']):r for r in arm_rows['glm-flash']}
    assert left.keys()==right.keys()
    # Resample paired 12-case batches, carrying both repetitions and providers together.
    groups=defaultdict(list)
    for key,l in left.items():
        r=right[key]
        groups[l['batch']].append(int(r[field]==r['gold'])-int(l[field]==l['gold']))
    values=[statistics.mean(v) for v in groups.values()]
    rng=random.Random(20260909)
    distribution=sorted(statistics.mean(rng.choices(values,k=len(values))) for _ in range(10000))
    per_case={}
    for r in arm_rows['luna-json']:
        key=(r['repetition'],r['id']); g=right[key]
        per_case.setdefault(r['id'],[]).append(int(g[field]==g['gold'])-int(r[field]==r['gold']))
    return {'glm_minus_luna_accuracy':statistics.mean(values),'paired_batch_bootstrap_95_percentile':[distribution[249],distribution[9749]],
        'bootstrap_resamples':10000,'paired_batches':len(values),'independent_texts':len(per_case),
        'glm_only_correct_observations':sum(l[field]!=l['gold'] and right[k][field]==right[k]['gold'] for k,l in left.items()),
        'luna_only_correct_observations':sum(l[field]==l['gold'] and right[k][field]!=right[k]['gold'] for k,l in left.items())}


def evaluate(directory):
    protocol=json.loads((ROOT/'protocol.json').read_text())
    corpus=json.loads((ROOT/'corpus.json').read_text())
    manifest=json.loads((directory/'manifest.json').read_text())
    assert manifest['finished'] and manifest['protocol_sha256']==digest(ROOT/'protocol.json')
    assert digest(ROOT/'corpus.json')==protocol['corpus_sha256']
    for name,h in manifest['artifact_sha256'].items():
        assert digest(directory/name)==h,name
    calls=json.loads((directory/'provider-calls.json').read_text())
    assert len(calls)==len(manifest['attempts'])
    cases={c['id']:c for c in corpus['cases']}
    relations=[r['relation'] for r in corpus['taxonomy']]
    ref={}; report={'mode':manifest['mode'],'arms':{},'all_planned_requests_completed':not manifest['halted_arms'] and len(calls)==202}
    arm_rows={}
    for arm in protocol['arms']:
        selected=[c for c in calls if c['context']['arm']==arm['id']]
        rows=[]; cost=[]; timings=[]; counts=Counter(); evidence_count=0; unknown_chunks=[]
        for call in selected:
            ctx=call['context']; batch_cases=corpus['preflight_cases'] if ctx['phase']=='preflight' else corpus['cases'][(ctx['batch']-1)*12:ctx['batch']*12]
            assert call['request_from_server']==pair_input(call['original_request_from_server'],batch_cases)
            assert call['request_to_provider']==previous_providers.adapt(call['request_from_server'],arm,protocol['max_completion_tokens'])
            outgoing=call['request_to_provider']; key=(ctx['phase'],ctx['batch'])
            assert outgoing['messages']==ref.setdefault(key,outgoing['messages'])
            if manifest['mode']=='live':
                assert all(set(c)=={'id','title','text','pair'} for c in ctx['cases'])
            raw=json.loads(call['raw_response'])
            if 'usage' in raw:
                item=dict(usage_cost(raw['usage'],arm,call['started_utc']),phase=ctx['phase'],repetition=ctx['repetition'],batch=ctx['batch'])
                regular=dict(arm,pricing={'regular':arm['pricing']['regular']})
                # Force a date beyond the promotion cutoff while preserving usage.
                item['regular_price_estimated_usd']=usage_cost(raw['usage'],regular,'2026-09-10T00:00:00Z')['estimated_usd']
                cost.append(item)
            else:
                counts['calls_without_usage']+=1
            observation=json.loads((directory/f"{arm['id']}-r{ctx['repetition']}-b{ctx['batch']}.json").read_text())
            assert observation['request']['chunks']==[{k:c[k] for k in ('id','title','text')} for c in batch_cases]
            assert observation['request']['taxonomy']==corpus['taxonomy']
            assert ctx['cases']==(batch_cases if manifest['mode']=='mock' else [{k:c[k] for k in ('id','title','text','pair')} for c in batch_cases])
            valid=observation['status']==200 and call['status']==200 and raw.get('model') in arm['accepted_returned_models'] and raw.get('choices',[{}])[0].get('finish_reason')=='stop'
            facts=[]
            if valid:
                facts=json.loads(raw['choices'][0]['message']['content'])['facts']
                assert observation['response']['proposed']==len(facts)
            if ctx['phase']=='preflight':
                counts['preflight_successes']+=int(valid)
                continue
            timings.append(observation['latency_seconds']); counts['measured_calls']+=1
            counts['successful_calls']+=int(valid); counts['nominations']+=len(facts)
            counts['accepted_occurrences']+=len(observation['facts'])
            counts['skipped_nominations']+=len(observation['response'].get('skips',[])) if isinstance(observation['response'],dict) else 0
            unknown_chunks += [f for f in facts if f['chunk_id'] not in {c['id'] for c in batch_cases}]
            entities={e['_key']:e['name'] for e in observation['entities']}
            local={c['id']:c for c in batch_cases}; accepted=[]
            for f in observation['facts']:
                chunk=f['evidence_chunk_id']; assert chunk in local
                text=unicodedata.normalize('NFC',local[chunk]['text']).encode()
                start,end=f['trigger_start'],f['trigger_end']
                assert 0<=start<end<=len(text) and text[start:end].decode()==f['trigger']
                assert f['reviewed_by']==f"directed:{arm['model']}@directed-policy-v1" and f['construction_schema']=='occurrence-v1'
                assert f['space_id']==f"semeval-{arm['id']}-r{ctx['repetition']}-b{ctx['batch']}"
                accepted.append({'chunk_id':chunk,'source':entities[f['_from'].removeprefix('entities/')],
                    'target':entities[f['_to'].removeprefix('entities/')],'relation':f['relation_type']})
                evidence_count+=1
            if valid:
                assert len(accepted)==observation['response']['facts_grounded']
            else:
                assert not accepted,'Unexpected committed facts after failed request'
            for c in batch_cases:
                row={'id':c['id'],'repetition':ctx['repetition'],'batch':ctx['batch'],'gold':c['label'],
                    'raw':prediction(c,facts,relations,valid),'accepted':prediction(c,accepted,relations,valid)}
                rows.append(row)
        summary={'model':arm['model'],'reasoning_effort':arm['reasoning_effort'],'counts':dict(counts),
            'verified_evidence_occurrences':evidence_count,'unknown_chunk_nominations':unknown_chunks,
            'usage_ledger':cost,'estimated_total_usd':sum(c['estimated_usd'] for c in cost),
            'estimated_measurement_usd':sum(c['estimated_usd'] for c in cost if c['phase']=='measurement'),
            'regular_price_measurement_usd':sum(c['regular_price_estimated_usd'] for c in cost if c['phase']=='measurement'),
            'http_latency_mean_seconds':statistics.mean(timings) if timings else None,
            'http_latency_p95_seconds':sorted(timings)[int(.95*(len(timings)-1))] if timings else None,
            'case_ledger':rows}
        report['arms'][arm['id']]=summary
        if len(rows)==1200:
            assert len({(r['id'],r['repetition']) for r in rows})==1200
            summary['repetitions']=[{'repetition':rep,**{f:classification([r for r in rows if r['repetition']==rep],f,relations) for f in ('raw','accepted')}} for rep in (1,2)]
            summary['pooled']={f:classification(rows,f,relations) for f in ('raw','accepted')}
            byid=defaultdict(list)
            for r in rows:
                byid[r['id']].append(r)
            summary['repeat_disagreements']={f:sum(v[0][f]!=v[1][f] for v in byid.values()) for f in ('raw','accepted')}
            arm_rows[arm['id']]=rows
    if len(arm_rows)==2:
        report['paired_comparison']={f:compare(arm_rows,f) for f in ('raw','accepted')}
    report['estimated_total_usd']=sum(a['estimated_total_usd'] for a in report['arms'].values())
    return report

if __name__=='__main__':
    import argparse
    from pathlib import Path
    parser=argparse.ArgumentParser(description=__doc__); parser.add_argument('directory',type=Path); parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args(); save(args.output,evaluate(args.directory))
