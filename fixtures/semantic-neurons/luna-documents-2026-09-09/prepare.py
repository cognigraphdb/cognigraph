"""Reproduce the frozen document cohort without model outputs or model calls."""
from collections import Counter
import hashlib
import json
from pathlib import Path
import statistics
import unicodedata

ROOT = Path(__file__).resolve().parent
REPO = ROOT.parents[2]


def normalize(text):
    return ' '.join(unicodedata.normalize('NFC', text).split()).casefold()


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def save(path, value):
    Path(path).write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n')


def canonical_relation(head, relation, tail, symmetric):
    if relation in symmetric:
        head, tail = sorted((head, tail))
    return head, relation, tail


def document(row, split, taxonomy):
    sentences = [unicodedata.normalize('NFC', ' '.join(tokens)) for tokens in row['sents']]
    assert sentences and all(sentence.strip() for sentence in sentences)
    text = '\n'.join(sentences)
    doc_id = split + '-' + hashlib.sha256(row['title'].encode()).hexdigest()[:16]
    entities, mismatches = [], []
    for index, mentions in enumerate(row['vertexSet']):
        assert mentions
        aliases, locations = set(), []
        for mention in mentions:
            sent = mention['sent_id']
            start, end = mention['pos']
            assert 0 <= sent < len(sentences)
            assert 0 <= start < end <= len(row['sents'][sent])
            surface = unicodedata.normalize('NFC', ' '.join(row['sents'][sent][start:end]))
            aliases.add(surface)
            if normalize(surface) != normalize(mention['name']):
                mismatches.append({'entity': index, 'sentence': sent,
                    'annotated_name': mention['name'], 'span_text': surface})
            locations.append({'sentence': sent, 'token_start': start, 'token_end': end,
                              'surface': surface, 'type': mention['type']})
        entities.append({'id': index, 'aliases': sorted(aliases), 'mentions': locations})
    mapping = {r['source_relation']: r['relation'] for r in taxonomy}
    symmetric = {r['relation'] for r in taxonomy if r['symmetric']}
    selected, evidence = set(), {}
    for label in row['labels']:
        assert 0 <= label['h'] < len(entities) and 0 <= label['t'] < len(entities)
        assert label['h'] != label['t']
        assert isinstance(label['r'], str)
        assert all(0 <= i < len(sentences) for i in label.get('evidence', []))
        if label['r'] not in mapping:
            continue
        triple = canonical_relation(label['h'], mapping[label['r']], label['t'], symmetric)
        selected.add(triple)
        evidence.setdefault(triple, set()).update(label.get('evidence', []))
    gold = {'id': doc_id, 'entities': entities,
        'relations': [{'head': h, 'relation': r, 'tail': t,
                       'source_evidence_sentences': sorted(evidence[(h, r, t)])}
                      for h, r, t in sorted(selected)],
        'source_annotation_name_mismatches': mismatches,
        'source_relation_annotations_total': len(row['labels'])}
    return {'id': doc_id, 'title': row['title'], 'text': text}, gold


def profile(inputs, gold):
    relation_counts = Counter(r['relation'] for d in gold for r in d['relations'])
    ambiguous, cross_sentence, with_evidence, endpoints = 0, 0, 0, 0
    for doc in gold:
        names = {}
        for entity in doc['entities']:
            for alias in entity['aliases']:
                names.setdefault(normalize(alias), set()).add(entity['id'])
        ambiguous += sum(len(ids) > 1 for ids in names.values())
        endpoints += len({r[side] for r in doc['relations'] for side in ['head', 'tail']})
        for relation in doc['relations']:
            head_sents = {m['sentence'] for m in doc['entities'][relation['head']]['mentions']}
            tail_sents = {m['sentence'] for m in doc['entities'][relation['tail']]['mentions']}
            cross_sentence += not bool(head_sents & tail_sents)
            with_evidence += bool(relation['source_evidence_sentences'])
    lengths = [len(doc['text'].split()) for doc in inputs]
    return {'documents': len(inputs), 'whole_document_tokens': {
        'minimum': min(lengths), 'median': statistics.median(lengths), 'maximum': max(lengths)},
        'sentences': sum(len(doc['text'].splitlines()) for doc in inputs),
        'reference_relations': sum(relation_counts.values()), 'relations_by_type': dict(sorted(relation_counts.items())),
        'documents_without_in_scope_reference_relations': sum(not d['relations'] for d in gold),
        'in_scope_reference_endpoint_entities': endpoints,
        'all_annotated_entities': sum(len(d['entities']) for d in gold),
        'relations_without_co_sentence_endpoints': cross_sentence,
        'relations_with_source_evidence_sentence_ids': with_evidence,
        'ambiguous_normalized_aliases': ambiguous,
        'source_annotation_name_mismatches': sum(len(d['source_annotation_name_mismatches']) for d in gold)}


def negative_review_candidates(inputs, gold, taxonomy, seed):
    """Blinded review queue, NOT negative gold; no prediction-based selection."""
    packets = []
    for doc, reference in zip(inputs, gold, strict=True):
        positives = {(r['head'], r['relation'], r['tail']) for r in reference['relations']}
        candidates = []
        for rule in taxonomy:
            for head in reference['entities']:
                for tail in reference['entities']:
                    if head['id'] == tail['id']:
                        continue
                    triple = canonical_relation(head['id'], rule['relation'], tail['id'],
                                                {rule['relation']} if rule['symmetric'] else set())
                    if triple in positives or (rule['symmetric'] and head['id'] > tail['id']):
                        continue
                    shared = {m['sentence'] for m in head['mentions']} & {m['sentence'] for m in tail['mentions']}
                    for sent in sorted(shared):
                        sentence = doc['text'].splitlines()[sent]
                        # Loose lexical retrieval deliberately creates candidates,
                        # never verdicts. Runtime gates use a different matcher.
                        if any(normalize(term) in normalize(sentence) for term in rule['require_in_sentence']):
                            key = f"{seed}|{doc['id']}|{triple}"
                            candidates.append((hashlib.sha256(key.encode()).hexdigest(), {
                                'document_id': doc['id'], 'head': head['id'], 'relation': rule['relation'],
                                'tail': tail['id'], 'head_aliases': head['aliases'], 'tail_aliases': tail['aliases'],
                                'candidate_sentence': sentence, 'sentence_id': sent, 'verdict': 'unreviewed',
                                'note': 'Absent from source annotations; not proof that the relation is false.'}))
                            break
        packets += [entry for _, entry in sorted(candidates)[:2]]
    return packets


def build():
    settings = json.loads((ROOT / 'settings.json').read_text())
    source = json.loads((ROOT / 'source-manifest.json').read_text())
    for name, metadata in source['files'].items():
        assert digest(ROOT / 'source' / name) == metadata['sha256'], name
    all_titles, all_texts, selected = set(), set(), {}
    source_counts = {}
    for split, name in settings['source_splits'].items():
        rows = json.loads((ROOT / 'source' / name).read_text())
        source_counts[split] = len(rows)
        # Validate and disjointness-check entire upstream split, not just sample.
        for row in rows:
            title = normalize(row['title'])
            text = normalize(' '.join(token for sentence in row['sents'] for token in sentence))
            assert title not in all_titles, 'Duplicate source title'
            assert text not in all_texts, 'Duplicate source text'
            all_titles.add(title)
            all_texts.add(text)
            document(row, split, settings['taxonomy'])
        rows.sort(key=lambda row: hashlib.sha256((settings['seed'] + '\n' +
            unicodedata.normalize('NFC', row['title'])).encode()).hexdigest())
        chosen = rows[:settings['counts'][split]]
        assert len(chosen) == settings['counts'][split]
        pairs = [document(row, split, settings['taxonomy']) for row in chosen]
        selected[split] = ([p[0] for p in pairs], [p[1] for p in pairs])
    # Near-duplicate diagnostic on selected full documents across the split.
    grams = lambda doc: set(zip(*[normalize(doc['text']).split()[i:] for i in range(5)]))
    dev = [(d['id'], grams(d)) for d in selected['development'][0]]
    hold = [(d['id'], grams(d)) for d in selected['holdout'][0]]
    near_duplicates = [(a, b) for a, x in dev for b, y in hold
                       if x and y and len(x & y) / len(x | y) >= .8]
    assert not near_duplicates, 'Cross-split near duplicate requires review before freezing'
    quality = {'source_documents_validated': sum(source_counts.values()), 'source_counts': source_counts,
        'source_titles_and_texts_unique': True, 'selected_cross_split_5gram_jaccard_at_least_0_8': near_duplicates,
        'splits': {split: profile(*data) for split, data in selected.items()},
        'scope': 'All published annotations for twelve predeclared relations; no claim of independently exhaustive truth.',
        'gold_origin': 'Upstream Re-DocRED human-reviewed labels; no CogniGraph/LLM-generated additions.',
        'source_limits': ['Complete benchmark documents, not full Wikipedia articles or long customer reports.',
            'Upstream recommend/revise annotation may still omit true relations; unmatched predictions need independent review.',
            'Source gold includes document-level inference; production prompting asks for explicit evidence.',
            'Some new source labels lack evidence sentence annotations.',
            'Public 2022 corpus may have appeared in model training.'],
        'holdout_status': 'Prepared mechanically; no model inference, tuning, or individual error inspection.'}
    return settings, selected, quality


def main():
    settings, selected, quality = build()
    for folder in ['inputs', 'gold', 'review']:
        (ROOT / folder).mkdir(exist_ok=True)
    for split, (inputs, gold) in selected.items():
        save(ROOT / 'inputs' / f'{split}.json', inputs)
        save(ROOT / 'gold' / f'{split}.json', gold)
    save(ROOT / 'review/development-negative-candidates.json',
         negative_review_candidates(*selected['development'], settings['taxonomy'], settings['seed']))
    save(ROOT / 'data-quality.json', quality)
    print(json.dumps(quality, indent=2))


if __name__ == '__main__':
    main()
