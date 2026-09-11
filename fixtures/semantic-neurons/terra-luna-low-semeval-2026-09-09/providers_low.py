"""OpenAI-only comparison adapter; exact prior prompt, cache-aware accounting."""
from datetime import datetime, timezone
import json
import time
import urllib.error
import urllib.request
from shared import BaseRecorder, NoRedirect, save
from providers import adapt, key_from_env
from accounting import usage_cost

ENDPOINTS = {'openai': ('OPENAI_API_KEY', 'https://api.openai.com/v1/chat/completions')}


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
        max_input = max(max(p['input'], p['cache_write']) for p in arm['pricing'].values())
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
                    'usage': {'prompt_tokens': 0, 'completion_tokens': 0, 'total_tokens': 0, 'prompt_tokens_details': {'cached_tokens': 0, 'cache_write_tokens': 0}, 'completion_tokens_details': {'reasoning_tokens': 0}}}).encode()
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
