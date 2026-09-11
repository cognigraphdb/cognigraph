"""Bounded provider adapter. Preserve raw responses and the unchanged server request."""
import copy
from datetime import datetime, timezone
import json
import os
import time
import urllib.error
import urllib.request

from shared import REPO, BaseRecorder, NoRedirect, save

ENDPOINTS = {
    'deepseek': ('DEEPSEEK_API_KEY', 'https://api.deepseek.com/chat/completions'),
    'zai': ('ZHIPU_API_KEY', 'https://api.z.ai/api/paas/v4/chat/completions'),
    'openai': ('OPENAI_API_KEY', 'https://api.openai.com/v1/chat/completions'),
}


def key_from_env(name):
    value = os.environ.get(name, '').strip()
    if not value:
        for line in (REPO / '.env').read_text().splitlines():
            if line.strip().startswith(name + '='):
                value = line.split('=', 1)[1].strip().strip('\"\'')
                break
    if not value:
        raise RuntimeError('Missing credential: ' + name)
    return value


def adapt(incoming, arm, limit):
    messages = copy.deepcopy(incoming['messages'])
    schema = incoming['response_format']['json_schema']['schema']
    messages[0]['content'] += '\nReturn one JSON object conforming to this JSON Schema:\n' + json.dumps(
        schema, ensure_ascii=False, sort_keys=True, separators=(',', ':'))
    body = {'model': arm['model'], 'messages': messages,
            'response_format': {'type': 'json_object'}, 'reasoning_effort': arm['reasoning_effort']}
    if arm['provider'] == 'openai':
        body['max_completion_tokens'] = limit
    else:
        body.update(max_tokens=limit, thinking={'type': 'enabled'})
    return body


def rates_at(arm, started_utc):
    stamp = datetime.fromisoformat(started_utc.replace('Z', '+00:00'))
    if arm['provider'] == 'deepseek':
        peak = stamp.weekday() < 5 and (1 <= stamp.hour < 4 or 6 <= stamp.hour < 10)
        band = 'peak' if peak else 'off_peak'
    elif arm['model'] == 'glm-5.3-flash':
        band = 'promotion' if stamp < datetime(2026, 9, 9, 16, tzinfo=timezone.utc) else 'regular'
    else:
        band = 'regular'
    return band, arm['pricing'][band]


def usage_cost(raw_usage, arm, stamp):
    """Missing cache/reasoning counters stay unknown; no invented zero measurements."""
    band, rate = rates_at(arm, stamp)
    input_tokens, output_tokens = raw_usage['prompt_tokens'], raw_usage['completion_tokens']
    assert isinstance(input_tokens, int) and isinstance(output_tokens, int)
    assert min(input_tokens, output_tokens) >= 0
    cached = raw_usage.get('prompt_cache_hit_tokens')
    if cached is None:
        cached = (raw_usage.get('prompt_tokens_details') or {}).get('cached_tokens')
    if cached is not None:
        assert 0 <= cached <= input_tokens
    reasoning = (raw_usage.get('completion_tokens_details') or {}).get('reasoning_tokens')
    amount = ((input_tokens - (cached or 0)) * rate['input'] +
              (cached or 0) * rate['cached_input'] + output_tokens * rate['output']) / 1e6
    return {'pricing_band': band, 'rates_usd_per_million': rate,
            'prompt_tokens': input_tokens, 'completion_tokens': output_tokens,
            'cached_input_tokens': cached, 'reasoning_tokens': reasoning,
            'estimated_usd': amount,
            'cache_accounting': 'reported' if cached is not None else 'unreported; uncached upper estimate'}


class Recorder(BaseRecorder):
    def __init__(self, output, protocol, mode):
        # Reuse loopback HTTP plumbing only; never load another provider's key implicitly.
        super().__init__(output, protocol, 'mock')
        self.mode = mode
        self.keys = {p: key_from_env(ENDPOINTS[p][0]) for p in {a['provider'] for a in protocol['arms']}} if mode == 'live' else {}

    def forward(self, handler, incoming):
        context = self.context
        arm = next(a for a in self.protocol['arms'] if a['id'] == context['arm'])
        assert incoming['model'] == arm['model']
        outgoing = adapt(incoming, arm, self.protocol['max_completion_tokens'])
        body = json.dumps(outgoing, ensure_ascii=False).encode()
        max_input = max(p['input'] for p in arm['pricing'].values())
        max_output = max(p['output'] for p in arm['pricing'].values())
        reservation = ((len(body) + 4096) * max_input + self.protocol['max_completion_tokens'] * max_output) / 1e6
        entry = {'index': len(self.records), 'context': context, 'mode': self.mode,
            'provider': arm['provider'], 'provider_url': ENDPOINTS[arm['provider']][1],
            'request_from_server': incoming, 'request_to_provider': outgoing,
            'started_utc': datetime.now(timezone.utc).isoformat().replace('+00:00', 'Z'),
            'reserved_usd': reservation, 'sent_to_provider': False}
        self.records.append(entry)
        start = time.monotonic()
        if self.reserved + reservation > self.protocol['budget_usd']:
            status, raw = 429, b'{"error":{"message":"Local request budget exhausted"}}'
        else:
            self.reserved += reservation
            if self.mode == 'mock':
                facts = [dict(f, source_type='entity', target_type='entity', evidence=c['text'], chunk_id=c['id'])
                         for c in context['cases'] for f in c['gold']]
                status, raw = 200, json.dumps({'model': arm['model'], 'choices': [{'finish_reason': 'stop',
                    'message': {'content': json.dumps({'facts': facts})}}],
                    'usage': {'prompt_tokens': 0, 'completion_tokens': 0, 'total_tokens': 0}}).encode()
            else:
                entry['sent_to_provider'] = True
                request = urllib.request.Request(entry['provider_url'], data=body,
                    headers={'Authorization': 'Bearer ' + self.keys[arm['provider']], 'Content-Type': 'application/json'})
                try:
                    response = urllib.request.build_opener(NoRedirect()).open(request, timeout=110)
                except urllib.error.HTTPError as error:
                    response = error
                except (OSError, urllib.error.URLError):
                    response = None
                if response is None:
                    status, raw = 502, b'{"error":{"message":"Transport failure; no retry"}}'
                else:
                    with response:
                        status, raw = response.status, response.read()
                        entry['request_id'] = response.headers.get('x-request-id') or response.headers.get('x-log-id')
        text = raw.decode('utf-8')
        for key in self.keys.values():
            text = text.replace(key, '[REDACTED_CREDENTIAL]')
        entry.update(status=status, raw_response=text, latency_seconds=time.monotonic() - start)
        save(self.output / 'provider-calls.json', self.records)
        handler.respond(status, text.encode())
