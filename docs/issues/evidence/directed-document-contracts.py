"""Release HTTP reproductions for endpoint boundaries and chunk-ID schema scope."""
import argparse
import json
from pathlib import Path
import sys

REPO = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPO / 'fixtures/semantic-neurons/luna-baseline-2026-09-09'))
from transport import Recorder as HttpRecorder, Server, digest, save


class Recorder(HttpRecorder):
    def __init__(self, output):
        super().__init__(output, {}, 'mock')
        self.proposal = None

    def forward(self, handler, incoming):
        raw = json.dumps({'model': incoming['model'], 'choices': [{'message': {
            'content': json.dumps({'facts': [self.proposal]})}}]}).encode()
        self.records.append({'request': incoming, 'proposal': self.proposal})
        handler.respond(200, raw)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    binary = REPO / 'target/release/cognigraph-server'
    import tempfile
    with tempfile.TemporaryDirectory(prefix='cognigraph-directed-contracts-') as temporary:
        recorder = Recorder(Path(temporary))
        evidence = {'date': '2026-09-09', 'binary_sha256': digest(binary),
                    'harness_sha256': digest(__file__), 'provider': 'synthetic loopback; no external calls', 'cases': []}
        cases = [
            ('inside-word-endpoint', 'Joanne owns Acme.', 'Ann', 'source-1', 1),
            ('complete-endpoint-control', 'Ann owns Acme.', 'Ann', 'source-1', 1),
            ('wrong-chunk-prefix', 'Ann owns Acme.', 'Ann', 'chunk source-1', 0),
        ]
        try:
            for name, text, source, chunk, observed_count in cases:
                recorder.proposal = {'source': source, 'source_type': 'person', 'target': 'Acme',
                    'target_type': 'organization', 'relation': 'OWNS', 'evidence': text, 'chunk_id': chunk}
                server = Server(binary, '', recorder)
                try:
                    body = {'space_type': 'directed-contract', 'taxonomy': [{'relation': 'OWNS',
                        'description': 'The source owns the target.', 'require_in_sentence': ['owns']}],
                        'chunks': [{'id': 'source-1', 'text': text}]}
                    status, response, _ = server.call('/api/construct/directed', body)
                    assert status == 200 and response['facts_grounded'] == observed_count
                    schema = recorder.records[-1]['request']['response_format']['json_schema']['schema']
                    chunk_schema = schema['properties']['facts']['items']['properties']['chunk_id']
                    assert chunk_schema == {'type': 'string'}
                    evidence['cases'].append({'name': name, 'http_status': status, 'request': body,
                        'proposal': recorder.proposal, 'response': response, 'facts': server.rows('facts'),
                        'entities': server.rows('entities'), 'chunk_id_schema': chunk_schema})
                finally:
                    server.close()
            evidence['result'] = 'Reproduced both limitations and positive control'
        finally:
            recorder.close()
        save(args.output, evidence)
        print(json.dumps({'result': evidence['result'], 'cases': len(evidence['cases'])}))


if __name__ == '__main__':
    main()
