"""One development-only fixed-output replay or live Luna-low candidate pass.

No retries; fresh release-server Native database per document. Only development
text, fixed taxonomy and historical request messages are parsed before live
generation. Published gold is hashed but never parsed in this runner.
"""
import argparse
from datetime import datetime, timezone
import json
from pathlib import Path
import sys
import time
import urllib.error
import urllib.request

from common import ROOT, BASE, REPO, digest, save, verify_protocol, expected_wire

sys.path.insert(0, str(ROOT.parent / 'luna-baseline-2026-09-09'))
from transport import Recorder as HttpRecorder, Server, NoRedirect


class Recorder(HttpRecorder):
    def __init__(self, output, protocol, mode, settings):
        super().__init__(output, protocol, 'mock' if mode == 'replay' else 'live')
        self.mode, self.settings = mode, settings
        self.expected, self.replayed_raw = None, None

    def forward(self, handler, incoming):
        assert incoming == self.expected, 'Production wire differs beyond the two planned enums'
        assert incoming['reasoning_effort'] == 'low'
        assert incoming['response_format']['json_schema']['strict'] is True
        outgoing = dict(incoming, max_completion_tokens=self.settings['max_completion_tokens'])
        body = json.dumps(outgoing, ensure_ascii=False).encode()
        pricing = self.protocol['arm']['pricing']['regular']
        reserve = ((len(body) + 4096) * pricing['cache_write'] +
            self.settings['max_completion_tokens'] * pricing['output']) / 1e6 if self.mode == 'live' else 0
        assert len(self.records) < 40
        assert self.reserved + reserve <= self.settings['budget_usd'], 'Frozen budget exhausted'
        entry = {'index': len(self.records), 'context': self.context, 'mode': self.mode,
            'request_from_server': incoming, 'request_to_provider': outgoing,
            'started_utc': datetime.now(timezone.utc).isoformat().replace('+00:00', 'Z'),
            'provider_url': 'https://api.openai.com/v1/chat/completions',
            'reserved_usd': reserve, 'sent_to_provider': False}
        self.records.append(entry)
        self.reserved += reserve
        save(self.output / 'provider-calls.json', self.records)
        start = time.monotonic()
        if self.mode == 'replay':
            response = json.loads(self.replayed_raw)
            # Preserve original proposals verbatim; replay incurs no token spend.
            response['usage'] = {'prompt_tokens': 0, 'completion_tokens': 0, 'total_tokens': 0,
                'prompt_tokens_details': {'cached_tokens': 0, 'cache_write_tokens': 0},
                'completion_tokens_details': {'reasoning_tokens': 0}}
            status, raw = 200, json.dumps(response).encode()
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
    parser.add_argument('--mode', choices=['replay', 'live'], required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--binary', type=Path, default=REPO / 'target/release/cognigraph-server')
    args = parser.parse_args()
    protocol = verify_protocol(current_code=True)
    assert digest(args.binary) == protocol['binary_sha256']
    settings = json.loads((BASE / 'settings.json').read_text())
    docs = json.loads((BASE / 'inputs/development.json').read_text())
    prior = {c['context']['document_id']: c for c in json.loads((BASE / 'live/provider-calls.json').read_text())}
    if args.mode == 'live':
        validation = json.loads((ROOT / 'replay-validation.json').read_text())
        assert validation['status'] == 'PASS' and validation['protocol_sha256'] == digest(ROOT / 'protocol.json')
        assert validation['manifest_sha256'] == digest(ROOT / 'replay/manifest.json')
    args.output.mkdir(parents=True, exist_ok=False)
    manifest = {'mode': args.mode, 'split': 'development', 'complete': False,
        'protocol_sha256': digest(ROOT / 'protocol.json'), 'binary_sha256': digest(args.binary),
        'started_utc': datetime.now(timezone.utc).isoformat(), 'attempts': [],
        'holdout_inference_calls': 0, 'storage': 'Fresh disposable Native database per document'}
    save(args.output / 'manifest.json', manifest)
    recorder = Recorder(args.output, protocol, args.mode, settings)
    try:
        for number, doc in enumerate(docs, 1):
            recorder.context = {'document_id': doc['id']}
            recorder.expected = expected_wire(prior[doc['id']]['request_from_server'], doc, settings)
            if args.mode == 'replay':
                recorder.replayed_raw = prior[doc['id']]['raw_response']
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
