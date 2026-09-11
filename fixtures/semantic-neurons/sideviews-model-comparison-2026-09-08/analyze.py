"""Summarize captured real-provider measurements without issuing API calls.

Lexical metrics match the Rust harness. Costs are estimates using the recorded
standard text-token prices; they are not account invoices or a free-tier claim.
"""
import argparse
import json
import math
from pathlib import Path
import re
import statistics

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('run', type=Path)
args = parser.parse_args()
ROOT = Path(__file__).resolve().parents[3]
DOCS = {doc['id']: doc['text'] for doc in json.loads((ROOT / 'crates/cognigraph-construct/fixtures/sideviews/benchmark-docs.json').read_text())}
ROWS = json.loads((args.run / 'requests.json').read_text())
MANIFEST = json.loads((args.run / 'manifest.json').read_text())
assert MANIFEST['result'] == 'complete', MANIFEST
STOP = set('the and for was were with that this from are has had have its his her their them which what who whom when where why how did does into than then they she him'.split())
META = ['the passage', 'this passage', 'the text', 'this text', 'the document', 'this document', 'the article', 'the author', 'mentioned', 'described', 'above', 'according to the']
RATES = {
    'openai': {'input': 0.20, 'cached_input': 0.02, 'cache_write': 0.25, 'output': 1.20},
    'gemini': {'input': 0.75, 'cached_input': 0.075, 'output': 3.75},
}


def words(text):
    return {word.lower() for word in re.split(r'[^\w]|_', text) if len(word.encode()) >= 3 and word.lower() not in STOP}


def norm(text):
    return ' '.join(text.lower().split()).rstrip('?.!')


def usage(row):
    raw = row['usage']
    if row['provider'] == 'openai':
        inp = raw['prompt_tokens']
        output = raw['completion_tokens']
        cached = raw.get('prompt_tokens_details', {}).get('cached_tokens', 0)
        written = raw.get('prompt_tokens_details', {}).get('cache_write_tokens', 0)
        reasoning = raw.get('completion_tokens_details', {}).get('reasoning_tokens', 0)
    else:
        inp = raw['promptTokenCount']
        reasoning = raw.get('thoughtsTokenCount', 0)
        output = raw.get('candidatesTokenCount', 0) + reasoning
        cached = raw.get('cachedContentTokenCount', 0)
        written = 0
    rate = RATES[row['provider']]
    assert inp >= cached + written and output >= reasoning
    cost = ((inp - cached - written) * rate['input'] + cached * rate['cached_input']
        + written * rate.get('cache_write', 0) + output * rate['output']) / 1_000_000
    return {'input': inp, 'cached_input': cached, 'cache_write': written, 'output_including_reasoning': output,
        'reasoning': reasoning, 'estimated_cost_usd': cost}


def summarize(rows):
    latencies = [row['upstream_latency_ms'] for row in rows]
    pairs = [pair for row in rows for pair in row['output']['pairs']]
    exact_schema = [set(row['output']) == {'pairs'} and all(set(pair) == {'question', 'answer'}
        and all(isinstance(pair[field], str) and pair[field].strip() for field in ('question', 'answer'))
        for pair in row['output']['pairs']) for row in rows]
    lexical_sum = 0
    distinct = []
    for row in rows:
        source_words = words(DOCS[row['passage']])
        current = row['output']['pairs']
        distinct.append(len({norm(pair['question']) for pair in current}) / len(current))
        for pair in current:
            answer_words = words(pair['answer'])
            lexical_sum += len(answer_words & source_words) / len(answer_words) if answer_words else 0
    tokens = [usage(row) for row in rows]
    return {'calls': len(rows), 'http_successes': sum(row['http_status'] == 200 for row in rows),
        'valid_schema': sum(exact_schema), 'total_pairs': len(pairs),
        'exact_target_calls': sum(len(row['output']['pairs']) == 12 for row in rows),
        'distinct_question_pct': 100 * statistics.mean(distinct),
        'self_contained_lexical_pct': 100 * sum(not any(term in pair['question'].lower() for term in META) for pair in pairs) / len(pairs),
        'answer_source_word_overlap_pct': 100 * lexical_sum / len(pairs),
        'mean_answer_words': statistics.mean(len(pair['answer'].split()) for pair in pairs),
        'latency_ms': {'mean': statistics.mean(latencies), 'median': statistics.median(latencies),
            'p95_nearest_rank': sorted(latencies)[math.ceil(0.95 * len(latencies)) - 1], 'min': min(latencies), 'max': max(latencies)},
        'tokens': {key: sum(item[key] for item in tokens) for key in tokens[0] if key != 'estimated_cost_usd'},
        'estimated_cost_usd': sum(item['estimated_cost_usd'] for item in tokens),
        'estimated_cost_per_1000_passages_usd': 1000 * statistics.mean(item['estimated_cost_usd'] for item in tokens),
        'returned_models': sorted({row['response_model'] for row in rows})}


measured = [row for row in ROWS if row['phase'] == 'measurement']
assert len(measured) == 30 and len(ROWS) == 32
summary = {'rates_per_million_tokens_usd': RATES,
    'pricing_checked': '2026-09-08',
    'pricing_sources': ['https://developers.openai.com/api/docs/models/gpt-5.6-luna', 'https://ai.google.dev/gemini-api/docs/pricing#gemini-3.8-flash'],
    'pricing_note': 'Standard short-context text rates. Gemini introductory rates through 2026-12-31. Account discounts, free-tier allowances, tax, and invoices are not observed.',
    'design': 'Five public passages x three repetitions x two configured models; preflights excluded; fixed prompts and schemas; provider order alternates and passage order uses recorded seeded permutations.',
    'limitations': 'Small fixture; lexical overlap is not entailment, semantic coverage, or retrieval recall; repeated calls are not independent documents; Gemini default thinking and Luna none compare configured defaults, not equal reasoning budgets.',
    'models': {}, 'per_round': {}, 'all_calls_estimated_cost_usd': sum(usage(row)['estimated_cost_usd'] for row in ROWS)}
for provider in ('openai', 'gemini'):
    selected = [row for row in measured if row['provider'] == provider]
    assert len(selected) == 15
    assert sorted((row['repeat'], row['passage']) for row in selected) == [(repeat, doc) for repeat in range(1, 4) for doc in sorted(DOCS)]
    summary['models'][provider] = summarize(selected)
    summary['per_round'][provider] = {str(repeat): summarize([row for row in selected if row['repeat'] == repeat]) for repeat in range(1, 4)}
(args.run / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
print(json.dumps(summary['models'], indent=2))
