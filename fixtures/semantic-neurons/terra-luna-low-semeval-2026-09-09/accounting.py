"""Published standard token tariffs; missing counters remain explicitly unknown."""


def usage_cost(raw_usage, arm, stamp):
    del stamp  # These two OpenAI arms use a single frozen standard tariff.
    assert arm['provider'] == 'openai'
    rate = arm['pricing']['regular']
    prompt = raw_usage['prompt_tokens']
    completion = raw_usage['completion_tokens']
    details = raw_usage.get('prompt_tokens_details') or {}
    cached = details.get('cached_tokens')
    written = details.get('cache_write_tokens')
    reasoning = (raw_usage.get('completion_tokens_details') or {}).get('reasoning_tokens')
    for value in (prompt, completion):
        assert type(value) is int and value >= 0
    for value in (cached, written):
        assert value is None or (type(value) is int and 0 <= value <= prompt)
    assert (cached or 0) + (written or 0) <= prompt
    assert reasoning is None or (type(reasoning) is int and 0 <= reasoning <= completion)
    assert rate['cached_input'] <= rate['input'] <= rate['cache_write']
    # Missing counters are not measured zeroes. Bound all compatible allocations.
    known = (cached or 0) + (written or 0)
    residual = prompt - known
    possible_rates = [rate['input']]
    if cached is None:
        possible_rates.append(rate['cached_input'])
    if written is None:
        possible_rates.append(rate['cache_write'])
    fixed = (cached or 0) * rate['cached_input'] + (written or 0) * rate['cache_write']
    lower = (fixed + residual * min(possible_rates) + completion * rate['output']) / 1e6
    upper = (fixed + residual * max(possible_rates) + completion * rate['output']) / 1e6
    # Retain an upper estimate for the existing report interface; expose the range.
    legacy = ((prompt - (cached or 0)) * rate['input'] + (cached or 0) * rate['cached_input'] + completion * rate['output']) / 1e6
    return {
        'pricing_band': 'regular', 'rates_usd_per_million': rate,
        'prompt_tokens': prompt, 'completion_tokens': completion,
        'cached_input_tokens': cached, 'cache_write_tokens': written,
        'reasoning_tokens': reasoning, 'estimated_usd': upper,
        'estimated_usd_lower': lower, 'estimated_usd_upper': upper,
        'legacy_without_cache_write_premium_usd': legacy,
        'cache_accounting': 'reported' if cached is not None and written is not None else 'incomplete; bounded estimate',
        'note': 'Reasoning is included in completion tokens. Standard token tariff estimate, not an invoice.',
    }
