"""Deterministic sampling and mechanical conversion of external human annotations."""
from collections import Counter
import hashlib
import json
import re
import unicodedata
from common import ROOT, save, digest

SEED = 'cognigraph-glm-luna-independent-2026-09-09-v1'
# Written from the task's relation definitions, before inspecting sampled labels or outputs.
# Vocabulary is intentionally finite: post-gate results are a diagnostic of this policy.
DEFINITIONS = [
 ('Cause-Effect', 'Cause -> effect: an event or object causes another event or object.', ['cause','causes','caused','causing','result','results','resulted','produce','produces','produced','lead','leads','led','due','because','from','by','of']),
 ('Instrument-Agency', 'Instrument -> agent: an agent uses an instrument to act. A component of an object alone is Component-Whole.', ['use','uses','used','using','with','by','through','via']),
 ('Product-Producer', 'Product -> producer: a producer makes or creates a product.', ['make','makes','made','making','produce','produces','produced','create','created','write','wrote','written','build','built','by','of','from']),
 ('Content-Container', 'Content -> container: an object is physically stored in a delimited container, a static situation. Movement into a destination is Entity-Destination.', ['in','inside','contain','contains','contained','fill','filled','of','within','into']),
 ('Entity-Origin', 'Entity -> origin: an entity comes or is derived from an origin or source.', ['from','out of','origin','origins','source','sources','derived','of']),
 ('Entity-Destination', 'Entity -> destination: an entity moves towards a destination.', ['to','into','onto','toward','towards','enter','entered','arrive','arrived']),
 ('Component-Whole', 'Component -> whole: an object is a functional or structural part of a larger whole.', ['of','in','has','have','had','with','part','parts','component','components','include','includes','consists','comprises']),
 ('Member-Collection', 'Member -> collection: a member is a nonfunctional part of a collection or group, rather than a structural component.', ['of','in','member','members','group','groups','collection','collections','consists','comprises','include','includes']),
 ('Message-Topic', 'Message -> topic: a written or spoken message communicates about a topic.', ['about','on','of','discuss','discusses','discussed','describe','describes','described','regarding','concerning','address','addresses']),
]

def parse(path):
    lines = path.read_text(encoding='utf-8').splitlines()
    cases = []
    for i, line in enumerate(lines):
        match = re.fullmatch(r'(\d+)\t"(.*)"', line)
        if not match:
            continue
        source_id, tagged = match.groups()
        e1 = re.search(r'<e1>(.*?)</e1>', tagged).group(1)
        e2 = re.search(r'<e2>(.*?)</e2>', tagged).group(1)
        label = lines[i+1].strip()
        text = unicodedata.normalize('NFC', re.sub(r'</?e[12]>', '', tagged))
        e1, e2 = unicodedata.normalize('NFC', e1), unicodedata.normalize('NFC', e2)
        assert e1 and e2 and e1 in text and e2 in text
        category = label.split('(')[0]
        assert category in [d[0] for d in DEFINITIONS] + ['Other']
        gold = []
        if label != 'Other':
            a, b = (e1,e2) if label.endswith('(e1,e2)') else (e2,e1)
            gold = [{'source': a, 'relation': category, 'target': b}]
        cases.append({'id': 'semeval-'+source_id, 'source_id': source_id,
            'title': 'Relation benchmark excerpt', 'text': text, 'pair': [e1,e2],
            'label': label, 'category': category, 'gold': gold})
    return cases

def rank(case, phase):
    return hashlib.sha256((SEED+':'+phase+':'+case['source_id']).encode()).hexdigest()


def prepare():
    test = parse(ROOT/'source/test.txt'); train = parse(ROOT/'source/train.txt')
    assert len(test) == 2717 and len(train) == 8000
    def normalized(c):
        return ' '.join(c['text'].casefold().split())
    train_texts = {normalized(c) for c in train}
    eligible, excluded, seen = [], [], set()
    for c in sorted(test, key=lambda c:int(c['source_id'])):
        reason = ('train_text_overlap' if normalized(c) in train_texts else
                  'duplicate_test_text' if normalized(c) in seen else
                  'identical_nominals' if c['pair'][0].casefold()==c['pair'][1].casefold() else None)
        if reason:
            excluded.append({'id':c['id'],'reason':reason})
        else:
            eligible.append(c); seen.add(normalized(c))
    cases = sorted(eligible, key=lambda c: rank(c,'test'))[:600]
    preflight = sorted([c for c in train if c['pair'][0].casefold()!=c['pair'][1].casefold()], key=lambda c: rank(c,'preflight'))[:12]
    assert len({c['id'] for c in cases}) == 600
    assert not ({normalized(c) for c in cases} & {normalized(c) for c in preflight})
    profile = {'test_population':len(test),'train_population':len(train),
        'eligible_test_population':len(eligible),'excluded_before_sampling':excluded,'sample_cases':len(cases),'sample_unique_texts':len({normalized(c) for c in cases}),
        'sample_duplicate_text_ids':[c['id'] for c in cases if sum(normalized(c)==normalized(x) for x in cases)>1],
        'sample_train_text_overlap':sum(normalized(c) in {normalized(x) for x in train} for c in cases),
        'preflight_overlap':0,'test_population_categories':dict(Counter(c['category'] for c in test)),
        'sample_categories':dict(Counter(c['category'] for c in cases)),
        'sample_directed_labels':dict(Counter(c['label'] for c in cases)),
        'sample_text_characters':{'min':min(len(c['text']) for c in cases),'max':max(len(c['text']) for c in cases)},
        'missing_ids_pairs_text_labels':0, 'gold_provenance':'Published human annotations; no relabelling or LLM judge',
        'limitations':['Public 2010 benchmark: unknown model training overlap.',
          'Nominal pairs are supplied; not entity discovery or exhaustive extraction.',
          'No new domain-expert labels and no customer-document evidence.',
          'Repeated calls are paired stability observations, not additional independent examples.']}
    save(ROOT/'corpus.json',{'schema':'cognigraph-semeval-pair-v1','seed':SEED,
        'selection':'Exclude normalized train-text overlaps, identical nominal strings and duplicate test texts (keep lowest id); lowest 600 SHA256(seed:test:source_id); no class or model-outcome filter',
        'taxonomy':[{'relation':r,'description':d,'require_in_sentence':v} for r,d,v in DEFINITIONS],
        'preflight_cases':preflight,'cases':cases})
    save(ROOT/'data-quality.json',profile)
    return profile

if __name__ == '__main__':
    print(json.dumps(prepare(),indent=2))
