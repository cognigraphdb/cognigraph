"""Development-only release-server capture. No holdout execution or retries."""
import argparse
from datetime import datetime, timezone
import json
from pathlib import Path
import sys
import time
import urllib.error
import urllib.request

from prepare import ROOT, REPO, digest, normalize, save
from score import aliases

sys.path.insert(0, str(ROOT.parent / 'luna-baseline-2026-09-09'))
from transport import Recorder as HttpRecorder, Server, NoRedirect


def control_proposals(doc, reference):
    names = aliases(reference)
    chosen = {}
    for entity in reference['entities']:
        unique = [a for a in entity['aliases'] if len(names[normalize(a)]) == 1]
        if unique:
            chosen[entity['id']] = max(unique, key=lambda a: (len(a), a))
    return [{'source': chosen[r['head']], 'target': chosen[r['tail']],
        'source_type': 'entity', 'target_type': 'entity', 'relation': r['relation'],
        'evidence': doc['text'], 'chunk_id': doc['id']} for r in reference['relations']
        if r['head'] in chosen and r['tail'] in chosen]


class Recorder(HttpRecorder):
    def __init__(self, output, protocol, mode, settings, controls):
        super().__init__(output, protocol, mode)
        self.settings, self.controls, self.control_facts = settings, controls, None

    def forward(self, handler, incoming):
        assert incoming['model'] == self.settings['model']
        assert incoming['reasoning_effort'] == 'low'
        assert incoming['response_format']['type'] == 'json_schema'
        assert incoming['response_format']['json_schema']['strict'] is True
        if self.mode == 'live':
            assert incoming == self.controls[self.context['document_id']], 'Input differs from frozen gold control'
        # Sole benchmark transport change: bound output spending. Production
        # model, effort, prompt, schema and strictness pass through unchanged.
        outgoing = dict(incoming, max_completion_tokens=self.settings['max_completion_tokens'])
        body = json.dumps(outgoing, ensure_ascii=False).encode()
        pricing = self.protocol['arm']['pricing']['regular']
        reserve = ((len(body) + 4096) * pricing['cache_write'] +
            self.settings['max_completion_tokens'] * pricing['output']) / 1e6
        entry = {'index': len(self.records), 'context': self.context, 'mode': self.mode,
            'request_from_server': incoming, 'request_to_provider': outgoing,
            'started_utc': datetime.now(timezone.utc).isoformat().replace('+00:00', 'Z'),
            'provider_url': 'https://api.openai.com/v1/chat/completions',
            'reserved_usd': reserve, 'sent_to_provider': False}
        assert len(self.records) < self.settings['counts']['development']
        assert self.reserved + reserve <= self.settings['budget_usd'], 'Frozen budget exhausted'
        self.records.append(entry)
        self.reserved += reserve
        save(self.output / 'provider-calls.json', self.records)
        start = time.monotonic()
        if self.mode == 'mock':
            status, raw = 200, json.dumps({'model': self.settings['model'],
                'choices': [{'finish_reason': 'stop', 'message': {'content': json.dumps({'facts': self.control_facts})}}],
                'usage': {'prompt_tokens': 0, 'completion_tokens': 0, 'total_tokens': 0,
                    'prompt_tokens_details': {'cached_tokens': 0, 'cache_write_tokens': 0},
                    'completion_tokens_details': {'reasoning_tokens': 0}}}).encode()
        else:
            entry['sent_to_provider'] = True
            request = urllib.request.Request(entry['provider_url'], data=body,
                headers={'Authorization': 'Bearer ' + self.key, 'Content-Type': 'application/json'})
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
                    entry['request_id'] = response.headers.get('x-request-id')
        text = raw.decode()
        if self.key:
            text = text.replace(self.key, '[REDACTED_CREDENTIAL]')
        entry.update(status=status, raw_response=text, latency_seconds=time.monotonic() - start)
        save(self.output / 'provider-calls.json', self.records)
        handler.respond(status, text.encode())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--mode', choices=['mock', 'live'], required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--binary', type=Path, default=REPO / 'target/release/cognigraph-server')
    args = parser.parse_args()
    protocol = json.loads((ROOT / 'protocol.json').read_text())
    assert digest(args.binary) == protocol['binary_sha256']
    for name, expected in protocol['package_sha256'].items():
        assert digest(ROOT / name) == expected, name
    for name, expected in protocol['code_sha256'].items():
        assert digest(REPO / name) == expected, name
    settings = json.loads((ROOT / 'settings.json').read_text())
    docs = json.loads((ROOT / 'inputs/development.json').read_text())
    # Live generation hashes gold bytes for integrity but never parses gold, alias maps or review packets.
    references = {d['id']: d for d in json.loads((ROOT / 'gold/development.json').read_text())} if args.mode == 'mock' else {}
    controls = {}
    if args.mode == 'live':
        seal = json.loads((ROOT / 'control-validation.json').read_text())
        assert seal['status'] == 'PASS' and seal['protocol_sha256'] == digest(ROOT / 'protocol.json')
        assert digest(ROOT / 'control/provider-calls.json') == seal['provider_calls_sha256']
        controls = {c['context']['document_id']: c['request_from_server']
                    for c in json.loads((ROOT / 'control/provider-calls.json').read_text())}
    args.output.mkdir(parents=True, exist_ok=False)
    manifest = {'mode': args.mode, 'split': 'development', 'complete': False,
        'protocol_sha256': digest(ROOT / 'protocol.json'), 'binary_sha256': digest(args.binary),
        'started_utc': datetime.now(timezone.utc).isoformat(), 'attempts': [],
        'holdout_inference_calls': 0, 'storage': 'fresh disposable Native database per document'}
    save(args.output / 'manifest.json', manifest)
    recorder = Recorder(args.output, protocol, args.mode, settings, controls)
    try:
        for number, doc in enumerate(docs, 1):
            recorder.context = {'document_id': doc['id']}
            if args.mode == 'mock':
                recorder.control_facts = control_proposals(doc, references[doc['id']])
            server = Server(args.binary.resolve(), '', recorder)
            try:
                request = {'space_type': 'document-trial-' + doc['id'],
                    'taxonomy': [{k: r[k] for k in ['relation', 'description', 'require_in_sentence']}
                                 for r in settings['taxonomy']], 'chunks': [doc]}
                status, result, elapsed = server.call('/api/construct/directed', request)
                observation = {'document_id': doc['id'], 'request': request, 'status': status,
                    'response': result, 'latency_seconds': elapsed, 'facts': server.rows('facts'),
                    'entities': server.rows('entities'), 'chunks': server.rows('chunks')}
                filename = f"{doc['id']}.json"
                save(args.output / filename, observation)
                manifest['attempts'].append(filename)
                save(args.output / 'manifest.json', manifest)
                print(f"{args.mode} {number}/{len(docs)}: HTTP {status}, stored {len(observation['facts'])}", flush=True)
            finally:
                server.close()
        manifest['complete'] = True
    finally:
        recorder.close()
        manifest['reserved_usd'] = recorder.reserved
        manifest['finished_utc'] = datetime.now(timezone.utc).isoformat()
        manifest['artifact_sha256'] = {p.name: digest(p) for p in sorted(args.output.glob('*.json'))
                                       if p.name != 'manifest.json'}
        save(args.output / 'manifest.json', manifest)


if __name__ == '__main__':
    main()
