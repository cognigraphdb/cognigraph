"""Add historical contrasts and corrected costs without mutating frozen evidence."""
import copy
import json
from common import ROOT, digest, save, usage_cost
from evaluate import compare, classification

OLD = ROOT.parent / 'glm-luna-semeval-2026-09-09'


def build():
    protocol = json.loads((ROOT/'protocol.json').read_text())
    inherited = protocol['inherited_package']
    assert digest(OLD/'package-manifest.json') == inherited['manifest_sha256']
    for name, value in json.loads((OLD/'package-manifest.json').read_text())['sha256'].items():
        assert digest(OLD/name) == value, name
    previous = json.loads((OLD/'results.json').read_text())
    current = json.loads((ROOT/'results.json').read_text())
    assert previous['all_planned_requests_completed'] and current['all_planned_requests_completed']
    assert digest(ROOT/'corpus.json') == digest(OLD/'corpus.json')
    old_protocol = json.loads((OLD/'protocol.json').read_text())
    for field in ('binary_sha256', 'batch_size', 'repetitions', 'max_completion_tokens',
                  'primary_endpoint', 'gate_policy', 'input_separation'):
        assert protocol[field] == old_protocol[field], field
    old_calls = json.loads((OLD/'live/provider-calls.json').read_text())
    new_calls = json.loads((ROOT/'live/provider-calls.json').read_text())
    prompt_ref = {}
    for call in old_calls + new_calls:
        ctx = call['context']
        key = (ctx['phase'], ctx['batch'])
        body = call['request_to_provider']
        assert body['messages'] == prompt_ref.setdefault(key, body['messages'])
        assert body['response_format'] == {'type': 'json_object'}
        assert body.get('max_completion_tokens', body.get('max_tokens')) == 8192
    arms = {'luna-none': copy.deepcopy(previous['arms']['luna-json']),
            'glm-flash-low': copy.deepcopy(previous['arms']['glm-flash']),
            **copy.deepcopy(current['arms'])}
    costs = {}
    for name, arm in arms.items():
        historical = name in ('luna-none', 'glm-flash-low')
        old_id = {'luna-none': 'luna-json', 'glm-flash-low': 'glm-flash'}.get(name, name)
        config = next(a for a in (old_protocol if historical else protocol)['arms'] if a['id'] == old_id)
        calls = [c for c in (old_calls if historical else new_calls) if c['context']['arm'] == old_id]
        ledger = []
        for call in calls:
            ctx = call['context']
            raw = json.loads(call['raw_response'])
            accounting = None
            if 'usage' in raw:
                if config['provider'] == 'openai':
                    priced = copy.deepcopy(config)
                    priced['pricing']['regular']['cache_write'] = priced['pricing']['regular']['input'] * 1.25
                    accounting = usage_cost(raw['usage'], priced, call['started_utc'])
                else:
                    entry = next(x for x in arm['usage_ledger'] if all(x[k] == ctx[k] for k in ('phase', 'repetition', 'batch')))
                    amount = entry['regular_price_estimated_usd']
                    accounting = {'estimated_usd_lower': amount, 'estimated_usd_upper': amount,
                                  'cache_accounting': entry['cache_accounting'],
                                  'observed_promotion_estimated_usd': entry['estimated_usd'],
                                  'note': 'Historical GLM usage repriced at the frozen regular tariff; timeout usage remains unknown.'}
            ledger.append({'phase': ctx['phase'], 'repetition': ctx['repetition'], 'batch': ctx['batch'],
                           'started_utc': call['started_utc'], 'usage': raw.get('usage'),
                           'service_tier': raw.get('service_tier'), 'accounting': accounting})
        measured = [x for x in ledger if x['phase'] == 'measurement']
        counters = {'prompt_tokens': [], 'completion_tokens': [], 'reasoning_tokens': [],
                    'cached_input_tokens': [], 'cache_write_tokens': []}
        for item in measured:
            usage = item['usage'] or {}
            details = usage.get('prompt_tokens_details') or {}
            counters['prompt_tokens'].append(usage.get('prompt_tokens'))
            counters['completion_tokens'].append(usage.get('completion_tokens'))
            counters['reasoning_tokens'].append((usage.get('completion_tokens_details') or {}).get('reasoning_tokens'))
            counters['cached_input_tokens'].append(usage.get('prompt_cache_hit_tokens', details.get('cached_tokens')))
            counters['cache_write_tokens'].append(details.get('cache_write_tokens'))
        costs[name] = {
            'historical_capture': historical,
            'previously_reported_measured_estimate_usd': arm['estimated_measurement_usd'],
            'regular_known_measured_lower_usd': sum(x['accounting']['estimated_usd_lower'] for x in measured if x['accounting']),
            'regular_known_measured_upper_usd': sum(x['accounting']['estimated_usd_upper'] for x in measured if x['accounting']),
            'known_schedule_lower_usd': sum(x['accounting']['estimated_usd_lower'] for x in ledger if x['accounting']),
            'known_schedule_upper_usd': sum(x['accounting']['estimated_usd_upper'] for x in ledger if x['accounting']),
            'calls_without_usage': sum(x['accounting'] is None for x in ledger),
            'measured_calls_without_usage': sum(x['accounting'] is None for x in measured),
            'measured_token_counters': {key: {'known_total': sum(v for v in values if v is not None),
                                            'reported_calls': sum(v is not None for v in values),
                                            'unreported_calls': sum(v is None for v in values)}
                                        for key, values in counters.items()},
            'ledger': ledger,
        }
    contrasts = {}
    for left, right in [('luna-low', 'terra-low'), ('luna-none', 'luna-low'),
                        ('glm-flash-low', 'terra-low'), ('glm-flash-low', 'luna-low')]:
        contrasts[right + '_minus_' + left] = {
            'left': left, 'right': right, 'historical_contrast': left not in current['arms'],
            **{field: compare({'luna-low': arms[left]['case_ledger'], 'terra-low': arms[right]['case_ledger']}, field)
               for field in ('raw', 'accepted')},
        }
    excluded = {r['id'] for a in arms.values() for r in a['case_ledger'] if r['raw'] == 'FAILED'}
    relations = [x['relation'] for x in json.loads((ROOT/'corpus.json').read_text())['taxonomy']]
    sensitivity = {'policy': protocol['sensitivity_policy'], 'excluded_ids': sorted(excluded),
                   'remaining_unique_sentences': 600-len(excluded),
                   'arms': {name: classification([r for r in a['case_ledger'] if r['id'] not in excluded], 'raw', relations)
                            for name, a in arms.items()}}
    result = {'protocol_sha256': digest(ROOT/'protocol.json'),
              'historical_results_sha256': digest(OLD/'results.json'),
              'new_results_sha256': digest(ROOT/'results.json'),
              'all_404_request_messages_match': True,
              'historical_note': protocol['historical_comparison_note'],
              'arms': {name: {k: v for k, v in a.items() if k not in ('case_ledger', 'usage_ledger')}
                       for name, a in arms.items()},
              'contrasts': contrasts, 'sensitivity': sensitivity,
              'costs': {name: {k: v for k, v in c.items() if k != 'ledger'} for name, c in costs.items()}}
    return result, costs


if __name__ == '__main__':
    result, costs = build()
    save(ROOT/'comparison.json', result)
    save(ROOT/'accounting.json', costs)
