#!/usr/bin/env python3
"""CG-97: measure side views as a gap source for neuron proposals on the
public forensics kits, under trigger ablation.

Every public kit already constructs all of its expected facts, so this
harness blanks the triggers of exactly the relation rules behind a kit's
expected facts in a disposable copy of the space type. It then, per kit and
repetition, on a fresh Enterprise server with an in-memory store:

1. installs the ablated space type (admin snapshot import), grounds the kit's
   chunks and records the baseline evaluation;
2. stores the chunks as documents and generates side views for them;
3. runs the side-view detector as a dry run at min_support 1 and 2;
4. proposes neurons from the side views (min_support 1);
5. reviews every stored proposal against the evaluation: accept it alone,
   re-ground, evaluate, retire it, re-ground. A proposal survives when it adds
   expected facts and introduces no forbidden fact;
6. accepts all survivors together and records the repaired evaluation.

Model calls are made only without --preflight. The server then runs with
--env-dir as its working directory and loads provider keys from the `.env`
file there itself; this harness never reads that file. Providers and models
are pinned explicitly, storage paths are blanked or temporary, and the run
aborts unless the server log confirms both generation lanes use --model.

    python3 docs/verification/harnesses/sideview-gap-measurement.py \\
        --bin DIR_WITH_ENTERPRISE_cognigraph-server --preflight --out OUT
    python3 docs/verification/harnesses/sideview-gap-measurement.py \\
        --bin DIR --env-dir CODE_DIR --runs 3 --out OUT
"""
import argparse
from contextlib import contextmanager
import copy
import datetime
import json
import os
from pathlib import Path
import re
import signal
import socket
import statistics
import subprocess
import tempfile
import time
from urllib.error import HTTPError
from urllib.parse import quote
from urllib.request import Request, urlopen

REPO = Path(__file__).resolve().parents[3]
KITS_DIR = REPO / 'fixtures/semantic-neurons'
KITS = {
    'alpha': ('digital_forensics', 'forensics_case_alpha', 'digital_forensics_case_alpha'),
    'bravo': ('digital_forensics', 'forensics_case_bravo', 'digital_forensics_case_bravo'),
}
SOURCE = 'kit_documents'
PASSWORD = 'synthetic-cg97-password'
SECRET = 'synthetic-cg97-jwt-secret-0123456789'
FACT = re.compile(r'^\s*(.+?)\s*--([A-Za-z0-9_]+)-->\s*(.+?)\s*$')
SECRET_SHAPED = re.compile(r'\b(?:(?:sk|pk|rk)-[A-Za-z0-9_-]{16,}|AIza[0-9A-Za-z_-]{30,})')
PROVIDER_MARKERS = ('provider configured', 'Embedding provider', 'Review judge')
ANSI = re.compile(r'\x1b\[[0-9;]*m')


# ---------------------------------------------------------------- pure logic

def parse_fact(line):
    match = FACT.match(line)
    if not match:
        raise ValueError(f'not `A --REL--> B`: {line!r}')
    return match.groups()


def eval_facts(spec, field):
    seen = []
    for question in spec.get('questions', []):
        for line in question.get(field, []):
            if line not in seen:
                seen.append(line)
    return seen


def ablate(space, spec):
    """Copy `space` with no triggers on the rules behind expected facts."""
    expected = {parse_fact(line): line for line in eval_facts(spec, 'expected_facts')}
    ablated = copy.deepcopy(space)
    hit = set()
    count = 0
    for rule in ablated.get('relation_rules', []):
        key = (rule['source'], rule['relation'], rule['target'])
        if key in expected:
            rule['when_any'] = []
            hit.add(key)
            count += 1
    return ablated, {
        'ablated_rules': count,
        'expected_facts': len(expected),
        'expected_without_rule': [line for key, line in expected.items() if key not in hit],
    }


def classify(fact, spec, missing):
    """Place a candidate fact relative to the kit's evaluation."""
    if fact in eval_facts(spec, 'forbidden_facts'):
        return 'forbidden'
    if fact in eval_facts(spec, 'expected_facts'):
        return 'missing_expected' if fact in missing else 'present_expected'
    return 'outside_evaluation'


def judge(baseline, after):
    """Evaluation-backed review of one accepted proposal."""
    added = [fact for fact in baseline['missing'] if fact not in after['missing']]
    new = [fact for fact in after['violations'] if fact not in baseline['violations']]
    return {'added_expected': added, 'new_violations': new, 'survived': bool(added) and not new}


def provider_lines(log):
    """Only the provider configuration lines, with secret-shaped tokens masked."""
    lines = (ANSI.sub('', line).strip() for line in log.splitlines())
    return [SECRET_SHAPED.sub('[redacted]', line) for line in lines
            if any(marker in line for marker in PROVIDER_MARKERS)]


def models_match(lines, model):
    lanes = ('Completion provider configured', 'Side-view generation provider configured')
    for lane in lanes:
        found = [line for line in lines if lane in line]
        if not found or not all(re.search(rf'model="?{re.escape(model)}"?(\s|$|,)', line) for line in found):
            return False
    return True


def summarize(runs):
    summary = {}
    for run in runs:
        kit = summary.setdefault(run['kit'], {})
        for name, value in run['metrics'].items():
            kit.setdefault(name, {'runs': []})['runs'].append(value)
    for kit in summary.values():
        for metric in kit.values():
            values = metric['runs']
            metric.update(min=min(values), max=max(values), mean=round(statistics.fmean(values), 3))
    return summary


# ---------------------------------------------------------------- live run

def free_port():
    with socket.socket() as sock:
        sock.bind(('127.0.0.1', 0))
        return sock.getsockname()[1]


class Api:
    def __init__(self, base, record):
        self.base, self.record, self.token = base, record, None

    def call(self, method, path, body=None, status=(200,), headers=None, timeout=900):
        request_headers = {'Content-Type': 'application/json', **(headers or {})}
        if self.token:
            request_headers['Authorization'] = 'Bearer ' + self.token
        request = Request(self.base + path, method=method, headers=request_headers,
                          data=None if body is None else json.dumps(body).encode())
        try:
            response = urlopen(request, timeout=timeout)
        except HTTPError as error:
            response = error
        with response:
            text = response.read().decode()
            code = response.status
        result = json.loads(text) if text else None
        login = 'auth/login' in path
        self.record.append({'method': method, 'path': path, 'status': code,
                            'request': {'username': body['username']} if login else body,
                            'response': {'token': '[redacted]'} if login else result})
        if code not in status:
            raise RuntimeError(f'{method} {path} -> {code}: {text[:500]}')
        return result


@contextmanager
def server(binary, cwd, providers, model):
    listen = free_port()
    store = tempfile.TemporaryDirectory()
    env = {key: os.environ[key] for key in ('PATH', 'HOME', 'TMPDIR') if key in os.environ}
    env.update({
        # Explicit settings win over the env directory's `.env` (dotenv never
        # overrides). Storage roots are blanked or temporary so the owner's
        # data can never be opened.
        'COGNIGRAPH_PORT': str(listen), 'COGNIGRAPH_HOST': '127.0.0.1',
        'COGNIGRAPH_AUTH_ENABLED': 'true', 'COGNIGRAPH_ADMIN_PASSWORD': PASSWORD,
        'COGNIGRAPH_HOST_ADMIN_PASSWORD': PASSWORD, 'COGNIGRAPH_JWT_SECRET': SECRET,
        'COGNIGRAPH_NATIVE_PATH': str(Path(store.name) / 'cg97.redb'),
        'COGNIGRAPH_DATA_DIR': '', 'COGNIGRAPH_ARTIFACT_CAS_ROOT': '', 'COGNIGRAPH_ARTIFACT_SOURCE': 'disabled',
        'COGNIGRAPH_M26_LIVE_FIXTURE_DIR': '', 'COGNIGRAPH_GOVERNANCE_ROOT_PUBLIC_KEY': '',
        'COGNIGRAPH_STORAGE_MODE': 'resident', 'COGNIGRAPH_VECTOR_MODE': 'embedded',
        'COGNIGRAPH_QUERY_CACHE_ENABLED': 'false', 'COGNIGRAPH_RATE_LIMIT_PER_MINUTE': '0',
        'COGNIGRAPH_REQUEST_TIMEOUT_SECS': '900', 'COGNIGRAPH_JUDGE_MODEL': '',
        'COGNIGRAPH_JUDGE_PARTNER_MODEL': '', 'COGNIGRAPH_LOG_FORMAT': 'text',
    })
    if providers:
        env.update({'COGNIGRAPH_COMPLETION_PROVIDER': 'openai', 'COGNIGRAPH_COMPLETION_MODEL': model,
                    'COGNIGRAPH_SIDEVIEWS_PROVIDER': 'openai', 'COGNIGRAPH_SIDEVIEWS_MODEL': model})
        # The embedding lane stays as the env directory configures it: side
        # views need embeddings to be stored, but the detector reads only
        # their text. The configured provider is recorded from the log.
    else:
        # Preflight runs outside the env directory; blank keys leave both
        # generation lanes unconfigured.
        env.update({'COGNIGRAPH_EMBEDDING_PROVIDER': 'none', 'OPENAI_API_KEY': '', 'GEMINI_API_KEY': ''})
    log = tempfile.TemporaryFile(mode='w+')
    process = subprocess.Popen([str(binary)], cwd=cwd, env=env, stdout=log, stderr=log)
    base = f'http://127.0.0.1:{listen}'

    def log_text():
        log.flush()
        log.seek(0)
        return log.read()

    try:
        for _ in range(200):
            if process.poll() is not None:
                raise RuntimeError('server exited: ' + ' | '.join(
                    SECRET_SHAPED.sub('[redacted]', line) for line in log_text().splitlines()[-5:]))
            try:
                urlopen(base + '/health/database', timeout=2).read()
                break
            except OSError:
                time.sleep(0.1)
        else:
            raise RuntimeError('server readiness timed out')
        yield base, log_text
    finally:
        if process.poll() is None:
            process.send_signal(signal.SIGTERM)
            try:
                process.wait(timeout=30)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
        log.close()
        store.cleanup()


def read_json(relative):
    return json.loads((KITS_DIR / relative).read_text())


def kit_chunks(name):
    return [json.loads(line) for line in (KITS_DIR / f'chunks/{name}.chunks.jsonl').read_text().splitlines()
            if line.strip()]


def evaluation(api, space_id, spec):
    result = api.call('POST', '/api/construct/evaluate', {'space_type': space_id, 'eval': spec})
    return {'found': result['recall']['found'], 'total': result['recall']['total'],
            'missing': sorted(result['missing']), 'violations': sorted(result['violations'])}


def ground(api, space_id, chunks):
    return api.call('POST', '/api/construct/ingest', {'space_type': space_id, 'chunks': chunks})


def wait_job(api, location, deadline=1800):
    started = time.monotonic()
    while time.monotonic() - started < deadline:
        job = api.call('GET', location)
        if job['status'] in ('succeeded', 'failed', 'canceled'):
            return job
        time.sleep(3)
    raise RuntimeError(f'job {location} did not finish')


def run_kit(args, kit, repetition):
    partial = {}
    try:
        return measure_kit(args, kit, repetition, partial)
    except Exception as error:
        error.partial = partial
        raise


def measure_kit(args, kit, repetition, partial):
    space_name, eval_name, chunk_name = KITS[kit]
    space = read_json(f'space_types/{space_name}.json')
    spec = read_json(f'evaluations/{eval_name}.json')
    chunks = kit_chunks(chunk_name)
    ablated, ablation = ablate(space, spec)
    space_id = ablated['id']
    record = []
    result = {'kit': kit, 'repetition': repetition, 'preflight': args.preflight, 'ablation': ablation,
              'started_at': datetime.datetime.now(datetime.timezone.utc).isoformat(), 'exchanges': record}
    partial.update(result=result)
    cwd = tempfile.mkdtemp() if args.preflight else args.env_dir
    with server(args.bin / 'cognigraph-server', cwd, not args.preflight, args.model) as (base, log_text):
        api = Api(base, record)
        lines = provider_lines(log_text())
        result['providers'] = lines
        if not args.preflight and not models_match(lines, args.model):
            raise RuntimeError(f'server is not configured for {args.model}: {lines}')
        if not args.preflight and not any('Embedding provider:' in line for line in lines):
            raise RuntimeError(f'no embedding provider configured: {lines}')
        api.token = api.call('POST', '/api/auth/login', {'username': 'admin', 'password': PASSWORD})['token']
        api.call('POST', '/api/admin/import', {'collections': {'space_types': {
            'type': 'document', 'documents': {space_id: {'_key': space_id, **ablated}}}}})
        ground(api, space_id, chunks)
        baseline = evaluation(api, space_id, spec)
        result['baseline'] = baseline
        for chunk in chunks:
            api.call('POST', '/api/documents', {'collection': SOURCE, '_key': chunk['id'],
                                                'title': chunk.get('title', ''), 'text': chunk['text']})
        if args.preflight:
            api.call('POST', '/api/sideviews/generate', {'collection': SOURCE}, status=(400,),
                     headers={'Idempotency-Key': f'cg97-{kit}-{repetition}'})
            api.call('POST', '/api/construct/propose',
                     {'space_type': space_id, 'side_views': {'collection': SOURCE}}, status=(400, 500, 503))
        else:
            submitted = api.call('POST', '/api/sideviews/generate', {'collection': SOURCE, 'count': args.count},
                                 status=(202,), headers={'Idempotency-Key': f'cg97-{kit}-{repetition}'})
            job = wait_job(api, f"/api/jobs/{quote(submitted['job']['id'], safe='')}")
            result['sideview_job'] = {'status': job['status'], 'progress': job.get('progress'),
                                      'error': job.get('error')}
            if job['status'] != 'succeeded':
                raise RuntimeError(f'side-view job {job["status"]}: {job.get("error")}')
        dry = {}
        for support in (1, 2):
            response = api.call('POST', '/api/construct/propose', {'space_type': space_id, 'side_views': {
                'collection': SOURCE, 'min_support': support, 'max_candidates': 100, 'dry_run': True}})
            dry[support] = response['side_views']
        result['dry_run'] = dry
        missing = set(baseline['missing'])
        for report in dry.values():
            for candidate in report['candidates']:
                candidate['class'] = classify(candidate['fact'], spec, missing)
        metrics = {
            'side_views': dry[1]['scanned'],
            'baseline_found': baseline['found'], 'expected_total': baseline['total'],
            'candidates_s1': len(dry[1]['candidates']), 'candidates_s2': len(dry[2]['candidates']),
            'candidates_missing_expected_s1': sum(c['class'] == 'missing_expected' for c in dry[1]['candidates']),
            'candidates_forbidden_s1': sum(c['class'] == 'forbidden' for c in dry[1]['candidates']),
            'candidates_outside_eval_s1': sum(c['class'] == 'outside_evaluation' for c in dry[1]['candidates']),
            'candidates_missing_expected_s2': sum(c['class'] == 'missing_expected' for c in dry[2]['candidates']),
            'candidates_forbidden_s2': sum(c['class'] == 'forbidden' for c in dry[2]['candidates']),
        }
        for reason in ('no_matching_rule', 'ambiguous_relation', 'endpoints_connected', 'below_min_support'):
            metrics[f'skipped_{reason}_s1'] = sum(s['reason'] == reason for s in dry[1]['skipped'])
        if not args.preflight and dry[1]['candidates']:
            proposal = api.call('POST', '/api/construct/propose', {'space_type': space_id, 'side_views': {
                'collection': SOURCE, 'min_support': 1, 'max_candidates': 100}})
            result['proposal'] = proposal
            listed = api.call('GET', f'/api/neurons?space_type={quote(space_id)}&status=proposed')
            neurons = {doc['_key']: doc for doc in listed['neurons']}
            result['proposed_neurons'] = list(neurons.values())
            reviews = []
            for stored in proposal['proposed']:
                key = stored['id']
                accepted = api.call('POST', f'/api/neurons/{quote(key, safe="")}/accept',
                                    {'note': 'CG-97 evaluation-backed review'}, status=(200, 400))
                if record[-1]['status'] != 200:
                    reviews.append({'id': key, 'fact': stored['fact'], 'accept_refused': accepted,
                                    'survived': False})
                    continue
                ground(api, space_id, chunks)
                verdict = judge(baseline, evaluation(api, space_id, spec))
                api.call('POST', f'/api/neurons/{quote(key, safe="")}/retire', {'note': 'CG-97 review done'})
                ground(api, space_id, chunks)
                restored = evaluation(api, space_id, spec)
                if restored != baseline:
                    raise RuntimeError(f'retiring {key} did not restore the baseline: {restored}')
                reviews.append({'id': key, 'fact': stored['fact'], 'triggers': stored.get('triggers'),
                                'class': classify(stored['fact'], spec, missing), **verdict})
            result['reviews'] = reviews
            survivors = [review for review in reviews if review['survived']]
            metrics.update(
                proposals_stored=proposal['stored'], proposal_skips=len(proposal['skipped']),
                survived=len(survivors),
                survived_forbidden=sum(review.get('class') == 'forbidden' for review in survivors))
            if survivors:
                # Retired neurons cannot be accepted again, so the combined
                # repair re-authors every survivor under a new id with the
                # same content, accepts them together and re-evaluates.
                for review in survivors:
                    source = neurons[review['id']]
                    copy_key = f"{review['id']}-combined"
                    api.call('POST', '/api/neurons', {
                        'space_type': space_id, 'id': copy_key, 'type': source['type'],
                        'source': source['source'], 'relation': source['relation'], 'target': source['target'],
                        'triggers': source.get('triggers', []), 'evidence': source.get('evidence', []),
                        'confidence': source.get('confidence', 0.5)})
                    api.call('POST', f'/api/neurons/{quote(copy_key, safe="")}/accept',
                             {'note': 'CG-97 combined repair'})
                ground(api, space_id, chunks)
                repaired = evaluation(api, space_id, spec)
                result['repaired'] = {**repaired, **judge(baseline, repaired)}
                metrics['repaired_found'] = repaired['found']
                metrics['repaired_violations'] = len(repaired['violations'])
            else:
                metrics['repaired_found'] = baseline['found']
                metrics['repaired_violations'] = len(baseline['violations'])
        elif not args.preflight:
            metrics.update(proposals_stored=0, proposal_skips=0, survived=0, survived_forbidden=0,
                           repaired_found=baseline['found'], repaired_violations=len(baseline['violations']))
        result['metrics'] = metrics
        result['providers'] = provider_lines(log_text())
    result['finished_at'] = datetime.datetime.now(datetime.timezone.utc).isoformat()
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--bin', type=Path, required=True, help='directory with the Enterprise cognigraph-server')
    parser.add_argument('--env-dir', type=Path, help='server working directory whose .env holds provider keys')
    parser.add_argument('--model', default='gpt-5.6-luna')
    parser.add_argument('--kits', default='alpha,bravo')
    parser.add_argument('--runs', type=int, default=3)
    parser.add_argument('--count', type=int, default=12, help='side views requested per document')
    parser.add_argument('--preflight', action='store_true', help='no providers and no model calls')
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    if not args.preflight and not args.env_dir:
        parser.error('--env-dir is required unless --preflight')
    args.bin = args.bin.resolve()
    args.env_dir = args.env_dir.resolve() if args.env_dir else None
    args.out.mkdir(parents=True, exist_ok=True)
    runs = []
    for repetition in range(1, (1 if args.preflight else args.runs) + 1):
        for kit in args.kits.split(','):
            try:
                result = run_kit(args, kit, repetition)
            except Exception as error:
                failed = {'kit': kit, 'repetition': repetition, 'error': str(error),
                          'failed_at': datetime.datetime.now(datetime.timezone.utc).isoformat(),
                          **getattr(error, 'partial', {})}
                (args.out / f'failed-{kit}-{repetition}.json').write_text(
                    json.dumps(failed, indent=2, ensure_ascii=False) + '\n')
                raise
            name = f'{"preflight" if args.preflight else "run"}-{kit}-{repetition}.json'
            (args.out / name).write_text(json.dumps(result, indent=2, ensure_ascii=False) + '\n')
            runs.append(result)
            print(f'{kit} #{repetition}: {json.dumps(result["metrics"])}', flush=True)
    summary = {'model': None if args.preflight else args.model, 'count': args.count, 'runs': len(runs),
               'kits': summarize(runs)}
    (args.out / ('preflight-summary.json' if args.preflight else 'summary.json')).write_text(
        json.dumps(summary, indent=2) + '\n')
    for text in (path.read_text() for path in args.out.glob('*.json')):
        if SECRET_SHAPED.search(text):
            raise SystemExit('a secret-shaped token reached the output; do not publish it')
    print('PASS: preflight' if args.preflight else 'PASS: measurement recorded')


if __name__ == '__main__':
    main()
