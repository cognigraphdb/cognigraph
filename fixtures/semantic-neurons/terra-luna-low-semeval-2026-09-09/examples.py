"""First run-one case in each fixed error category; illustrative, never relabelled."""
import json
from common import ROOT, save
from compare import OLD


def main():
    current = json.loads((ROOT/'results.json').read_text())['arms']
    previous = json.loads((OLD/'results.json').read_text())['arms']
    arms = {'luna-none': previous['luna-json'], 'glm-flash-low': previous['glm-flash'], **current}
    rows = {name: {r['id']: r for r in arm['case_ledger'] if r['repetition'] == 1} for name, arm in arms.items()}
    corpus = json.loads((ROOT/'corpus.json').read_text())
    categories = {
        'terra_correct_luna_low_wrong': lambda c, p: p['terra-low'] == c['label'] and p['luna-low'] != c['label'],
        'luna_low_correct_terra_wrong': lambda c, p: p['luna-low'] == c['label'] and p['terra-low'] != c['label'],
        'terra_correct_glm_wrong': lambda c, p: p['terra-low'] == c['label'] and p['glm-flash-low'] != c['label'],
        'all_four_wrong_complete_responses': lambda c, p: all(v != c['label'] and v not in ('FAILED', 'INVALID') for v in p.values()),
        'terra_false_assertion_on_other': lambda c, p: c['label'] == 'Other' and p['terra-low'] not in ('Other', 'FAILED'),
    }
    examples = []
    for category, predicate in categories.items():
        selected = None
        for case in corpus['cases']:
            predictions = {name: by_id[case['id']]['raw'] for name, by_id in rows.items()}
            if predicate(case, predictions):
                selected = {'category': category, 'id': case['id'], 'text': case['text'], 'pair': case['pair'],
                            'human_gold': case['label'], 'raw_predictions': predictions,
                            'post_gate_predictions': {name: by_id[case['id']]['accepted'] for name, by_id in rows.items()},
                            'repetition': 1, 'batch': rows['terra-low'][case['id']]['batch']}
                break
        examples.append(selected or {'category': category, 'matching_case': None})
    save(ROOT/'illustrative-errors.json', {'selection': 'First matching case in frozen corpus order, repetition one; no statistical subsample or gold relabelling.', 'examples': examples})


if __name__ == '__main__':
    main()
