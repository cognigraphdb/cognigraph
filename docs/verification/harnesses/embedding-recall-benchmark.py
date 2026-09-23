#!/usr/bin/env python3
"""CG-98: embedding recall benchmark for side-view retrieval.

Compares embedding models on the same DailyMed corpus, the same side-view
texts and the same queries, through the product's own hybrid search
(`POST /api/search/hybrid`, RRF over BM25, chunk vectors and side-view
vectors). Queries are lay descriptions with drug names removed, in English
and translated into other languages, each with one target label.

Subcommands, in order:

    select     choose the labels from the local DailyMed pilot corpus and
               write selection.json into --inputs
    brief      write per-target excerpts used to author the queries
    sideviews  generate side views once (model calls) and freeze their text
               in sideviews.jsonl in --inputs
    run        one embedder: load labels, embed chunks and side views, run
               every query under every configuration (embedding calls)
    report     metrics, 95% bootstrap intervals and paired differences
    preflight  the whole run path on a subset against a local fake embedder

Frozen inputs default to the private evidence package next to this checkout
(--inputs overrides). The server runs with --env-dir as its working directory and loads provider
keys from `.env` there itself; this harness never reads that file. Providers
and models are pinned per embedder and checked against the server's own
report, and storage roots are blanked or temporary.

    python3 docs/verification/harnesses/embedding-recall-benchmark.py select --data data/dailymed-pilot
    python3 docs/verification/harnesses/embedding-recall-benchmark.py run --bin DIR --env-dir CODE_DIR \\
        --data data/dailymed-pilot --embedder openai-small --out OUT
"""
import argparse
from concurrent.futures import ThreadPoolExecutor
from contextlib import contextmanager
import datetime
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import math
import os
from pathlib import Path
import random
import re
import signal
import socket
import subprocess
import tempfile
import threading
import time
import unicodedata
from urllib.error import HTTPError
from urllib.request import Request, urlopen

REPO = Path(__file__).resolve().parents[3]
# Frozen inputs (selection, queries, side views) are labelled evaluation
# data, so they live in the private evidence repository (evidence policy).
FIXTURE = REPO.parent / 'evidence/runs/2026-09-23-cg-98-embedding-recall/inputs'
SEED = 'cognigraph-cg98-embedding-recall-v1'
LABELS, TARGETS = 200, 100
INDICATIONS = '34067-9'
PASSWORD = 'synthetic-cg98-password'
SECRET = 'synthetic-cg98-jwt-secret-0123456789'
SECRET_SHAPED = re.compile(r'\b(?:(?:sk|pk|rk)-[A-Za-z0-9_-]{16,}|AIza[0-9A-Za-z_-]{30,})')
ANSI = re.compile(r'\x1b\[[0-9;]*m')
TRANSIENT = re.compile(r'429|too many requests|resource exhausted|rate limit|timed out|timeout|overloaded|'
                       r'temporarily unavailable', re.IGNORECASE)

EMBEDDERS = {
    'openai-small': ('openai', 'text-embedding-3-small'),
    'openai-large': ('openai', 'text-embedding-3-large'),
    'gemini-2': ('gemini', 'gemini-embedding-2'),
    'qwen3-8b': ('ollama', 'qwen3-embedding:8b'),
    'bge-m3': ('ollama', 'bge-m3'),
}
PROVIDER_LOG = {'openai': 'Embedding provider: OpenAI', 'gemini': 'Embedding provider: Gemini',
                'ollama': 'Embedding provider: Ollama'}
COMMON = {'documents_collection': 'labels', 'embeddings_collection': 'label_chunks',
          'side_views_collection': 'sv', 'search_fields': ['content', 'title'],
          'threshold': -1.0, 'limit': 10}
CONFIGS = {
    'bm25': {'bm25_weight': 1.0, 'vector_weight': 0.0, 'include_side_views': False},
    'vector': {'bm25_weight': 0.0, 'vector_weight': 1.0, 'include_side_views': False},
    'vector+sv': {'bm25_weight': 0.0, 'vector_weight': 0.5, 'include_side_views': True, 'side_view_weight': 0.5},
    'hybrid': {'bm25_weight': 0.5, 'vector_weight': 0.5, 'include_side_views': False},
    'hybrid+sv': {'bm25_weight': 0.5, 'vector_weight': 0.5, 'include_side_views': True, 'side_view_weight': 0.5},
    'hybrid+sv0.35': {'bm25_weight': 0.5, 'vector_weight': 0.5, 'include_side_views': True,
                      'side_view_weight': 0.35},
}
FORM_WORDS = set('''
TABLET TABLETS CAPSULE CAPSULES CREAM OINTMENT GEL LOTION SOLUTION SUSPENSION INJECTION INJECTABLE
ORAL ORALLY TOPICAL FILM COATED EXTENDED DELAYED RELEASE CHEWABLE DISINTEGRATING POWDER KIT SPRAY
NASAL OPHTHALMIC OTIC PATCH SYRUP ELIXIR LIQUID CONCENTRATE EMULSION AEROSOL INHALATION INHALANT
FOAM SHAMPOO SUPPOSITORY VAGINAL RECTAL TRANSDERMAL AUGMENTED HYDROCHLORIDE SODIUM POTASSIUM
CALCIUM MAGNESIUM SULFATE ACETATE CITRATE PHOSPHATE MALEATE TARTRATE BITARTRATE SUCCINATE FUMARATE
MESYLATE BESYLATE BROMIDE CHLORIDE DIHYDRATE MONOHYDRATE ANHYDROUS WITH INTRAVENOUS INTRAMUSCULAR
SUBCUTANEOUS SINGLE DOSE VIAL VIALS PREFILLED SYRINGE UNIT BULK PACKAGE PHARMACY DIRECT FOR USE
SUSTAINED CONTROLLED ACTION GRANULE GRANULES PELLETS PELLET SHEET SHEETS TROCHE LOZENGE WAFER
STRIP STRIPS DROPS DROP METERED PUMP SWAB SWABS CLOTH DRESSING IMPLANT RING INSERT SYSTEM DEVICE
MICRONIZED HEMIHYDRATE TRIHYDRATE HYCLATE DISODIUM FREE BASE ACID SALT DELIVERY ONCE DAILY
'''.split())


# ---------------------------------------------------------------- pure logic

def _head(title):
    return title.split('[')[0]


def drug_key(title):
    words = re.findall(r'[A-Za-z][A-Za-z0-9-]*', _head(title))
    return words[0].upper() if words else ''


def name_tokens(title):
    return {word.upper() for word in re.findall(r'[A-Za-z][A-Za-z-]*', _head(title))
            if len(word) >= 4 and word.upper() not in FORM_WORDS}


def select_labels(rows, seed, count, eligible):
    ranked = sorted(rows, key=lambda row: hashlib.sha256(f'{seed}:{row["set_id"]}'.encode()).hexdigest())
    chosen, seen = [], set()
    for row in ranked:
        key = drug_key(row['title'])
        if not key or key in seen or not eligible(row):
            continue
        seen.add(key)
        chosen.append(row['set_id'])
        if len(chosen) == count:
            return chosen
    raise ValueError(f'only {len(chosen)} eligible distinct labels, {count} requested')


def _pieces(text, limit):
    """Greedy packing: sentences, then words, then hard slices."""
    units = []
    for sentence in re.split(r'(?<=[.!?])\s+', text.strip()):
        if len(sentence) <= limit:
            units.append(sentence)
            continue
        for word in sentence.split():
            units.extend(word[i:i + limit] for i in range(0, len(word), limit))
    pieces, current = [], ''
    for unit in units:
        if not unit:
            continue
        candidate = f'{current} {unit}' if current else unit
        if len(candidate) <= limit:
            current = candidate
        else:
            pieces.append(current)
            current = unit
    if current:
        pieces.append(current)
    return pieces


def chunk_label(doc, max_chars=2000):
    chunks = []
    for section in doc.get('sections', []):
        text = (section.get('text') or '').strip()
        if not text:
            continue
        title = (section.get('title') or '').strip()
        prefix = f'{title}\n'
        for piece in _pieces(text, max_chars - len(prefix)):
            chunks.append({'_key': f'{doc["set_id"]}-{len(chunks):03d}', 'document_id': f'labels/{doc["set_id"]}',
                           'section': title, 'text': prefix + piece})
    return chunks


def _fold(text):
    return ''.join(c for c in unicodedata.normalize('NFKD', text) if not unicodedata.combining(c)).upper()


def leaks(text, tokens):
    words = re.findall(r'[A-Z0-9]+', _fold(text))
    return sorted(token for token in tokens if any(word.startswith(token) for word in words))


def rank_of(target, results):
    position = 0
    for result in results:
        if not result.get('score'):
            continue
        position += 1
        if result.get('document_id') == f'labels/{target}':
            return position
    return None


def transient(code, text):
    """Provider throttling surfaces as HTTP 500 with the provider's message."""
    return code in (429, 502, 503, 504) or (code == 500 and bool(TRANSIENT.search(text)))


def reciprocal(rank, cutoff=10):
    return 1.0 / rank if rank and rank <= cutoff else 0.0


def metrics(ranks):
    n = len(ranks)
    result = {'n': n}
    for k in (1, 5, 10):
        result[f'recall@{k}'] = sum(1 for r in ranks if r and r <= k) / n if n else 0.0
    result['mrr@10'] = sum(reciprocal(r) for r in ranks) / n if n else 0.0
    return result


def bootstrap_ci(values, iters=2000, seed=0, alpha=0.05):
    if len(set(values)) <= 1:
        value = values[0] if values else 0.0
        return value, value
    rng = random.Random(seed)
    n = len(values)
    means = sorted(sum(values[rng.randrange(n)] for _ in range(n)) / n for _ in range(iters))
    return means[int(iters * alpha / 2)], means[int(iters * (1 - alpha / 2)) - 1]


def paired_delta(a, b, iters=2000, seed=0):
    if len(a) != len(b):
        raise ValueError('paired comparison needs the same queries')
    diffs = [x - y for x, y in zip(a, b)]
    low, high = bootstrap_ci(diffs, iters=iters, seed=seed)
    return sum(diffs) / len(diffs), low, high


# ---------------------------------------------------------------- corpus

def load_rows(data):
    rows = [json.loads(line) for line in (data / 'manifest-rx.jsonl').read_text().splitlines() if line.strip()]
    return [{'set_id': row['set_id'], 'title': row['title']} for row in rows]


def load_doc(data, set_id):
    return json.loads((data / 'documents' / f'{set_id}.json').read_text())


def indications(doc):
    return next((s['text'] for s in doc.get('sections', []) if s.get('code') == INDICATIONS), '')


def selection():
    return json.loads((FIXTURE / 'selection.json').read_text())


def corpus(data):
    chosen = selection()
    docs = []
    for label in chosen['labels']:
        doc = load_doc(data, label['set_id'])
        if doc['text_sha256'] != label['text_sha256']:
            raise SystemExit(f'{label["set_id"]}: local text differs from the frozen selection')
        docs.append(doc)
    return chosen, docs


def cmd_select(args):
    rows = load_rows(args.data)

    def eligible(row):
        doc = load_doc(args.data, row['set_id'])
        return len(indications(doc)) >= 200

    chosen = select_labels(rows, SEED, LABELS, eligible)
    labels = []
    for index, set_id in enumerate(chosen):
        doc = load_doc(args.data, set_id)
        labels.append({'set_id': set_id, 'title': doc['title'], 'drug_key': drug_key(doc['title']),
                       'target': index < TARGETS, 'text_sha256': doc['text_sha256'],
                       'char_count': doc['char_count'], 'chunks': len(chunk_label(doc))})
    run = json.loads((args.data / 'run.json').read_text()) if (args.data / 'run.json').exists() else {}
    FIXTURE.mkdir(parents=True, exist_ok=True)
    (FIXTURE / 'selection.json').write_text(json.dumps({
        'seed': SEED, 'labels_requested': LABELS, 'targets': TARGETS,
        'source': 'DailyMed human prescription labels from the local pilot corpus (docs/research/dailymed/pilot.md)',
        'source_snapshot': run.get('catalog_dates'), 'source_generated_at': run.get('generated_at'),
        'rule': 'SHA-256 rank of seed:set_id; first label per distinct first title word; indications >= 200 chars',
        'labels': labels}, indent=2) + '\n')
    print(f'selected {len(labels)} labels, {sum(l["chunks"] for l in labels)} chunks')


def cmd_brief(args):
    chosen, docs = corpus(args.data)
    out = []
    for label, doc in zip(chosen['labels'], docs):
        if not label['target']:
            continue
        sections = {s.get('title', ''): s.get('text', '') for s in doc['sections']}
        out.append({'set_id': label['set_id'], 'title': doc['title'],
                    'forbidden': sorted(name_tokens(doc['title'])),
                    'indications': indications(doc)[:1500],
                    'section_titles': list(sections)[:25]})
    args.out.write_text(''.join(json.dumps(row, ensure_ascii=False) + '\n' for row in out))
    print(f'{len(out)} briefs -> {args.out}')


def load_queries():
    rows = [json.loads(line) for line in (FIXTURE / 'queries.jsonl').read_text().splitlines() if line.strip()]
    chosen = {label['set_id']: label for label in selection()['labels']}
    for row in rows:
        label = chosen.get(row['target'])
        if not label or not label['target']:
            raise SystemExit(f'query {row["id"]}: target {row["target"]} is not a selected target label')
        found = leaks(row['text'], name_tokens(label['title']))
        if found:
            raise SystemExit(f'query {row["id"]} ({row["lang"]}) names the drug: {found}')
    return rows


# ---------------------------------------------------------------- server

def free_port():
    with socket.socket() as sock:
        sock.bind(('127.0.0.1', 0))
        return sock.getsockname()[1]


class Api:
    def __init__(self, base):
        self.base, self.token, self.retries = base, None, 0

    def call(self, method, path, body=None, status=(200,), headers=None, timeout=900):
        request_headers = {'Content-Type': 'application/json', **(headers or {})}
        if self.token:
            request_headers['Authorization'] = 'Bearer ' + self.token
        request = Request(self.base + path, method=method, headers=request_headers,
                          data=None if body is None else json.dumps(body).encode())
        attempts = 8
        for attempt in range(attempts):
            try:
                response = urlopen(request, timeout=timeout)
            except HTTPError as error:
                response = error
            except (ConnectionError, TimeoutError, OSError):
                if attempt == attempts - 1:
                    raise
                time.sleep(min(60, 2 ** (attempt + 1)))
                continue
            with response:
                text = response.read().decode()
                code = response.status
            if code not in status and transient(code, text) and attempt < attempts - 1:
                self.retries += 1
                time.sleep(min(60, 2 ** (attempt + 1)))
                continue
            if code not in status:
                raise RuntimeError(f'{method} {path} -> {code}: {SECRET_SHAPED.sub("[redacted]", text[:400])}')
            return json.loads(text) if text else None


@contextmanager
def server(binary, cwd, config):
    listen = free_port()
    store = tempfile.TemporaryDirectory()
    env = {key: os.environ[key] for key in ('PATH', 'HOME', 'TMPDIR') if key in os.environ}
    env.update({
        # Explicit settings win over `.env` in cwd (dotenv never overrides).
        'COGNIGRAPH_PORT': str(listen), 'COGNIGRAPH_HOST': '127.0.0.1',
        'COGNIGRAPH_AUTH_ENABLED': 'true', 'COGNIGRAPH_ADMIN_PASSWORD': PASSWORD,
        'COGNIGRAPH_HOST_ADMIN_PASSWORD': PASSWORD, 'COGNIGRAPH_JWT_SECRET': SECRET,
        'COGNIGRAPH_NATIVE_PATH': str(Path(store.name) / 'cg98.redb'),
        'COGNIGRAPH_DATA_DIR': '', 'COGNIGRAPH_ARTIFACT_CAS_ROOT': '', 'COGNIGRAPH_ARTIFACT_SOURCE': 'disabled',
        'COGNIGRAPH_M26_LIVE_FIXTURE_DIR': '', 'COGNIGRAPH_GOVERNANCE_ROOT_PUBLIC_KEY': '',
        'COGNIGRAPH_STORAGE_MODE': 'resident', 'COGNIGRAPH_VECTOR_MODE': 'embedded',
        'COGNIGRAPH_QUERY_CACHE_ENABLED': 'false', 'COGNIGRAPH_RATE_LIMIT_PER_MINUTE': '0',
        'COGNIGRAPH_REQUEST_TIMEOUT_SECS': '1800', 'COGNIGRAPH_LOG_FORMAT': 'text',
        'COGNIGRAPH_JUDGE_MODEL': '', 'COGNIGRAPH_JUDGE_PARTNER_MODEL': '', 'COGNIGRAPH_JWT_TTL_SECS': '86400',
        'COGNIGRAPH_CGQL_MAX_SOURCE_ROWS': '0', 'COGNIGRAPH_CGQL_TIME_BUDGET_MS': '0',
        **config,
    })
    log = tempfile.TemporaryFile(mode='w+')
    process = subprocess.Popen([str(binary)], cwd=cwd, env=env, stdout=log, stderr=log)
    base = f'http://127.0.0.1:{listen}'

    def log_lines():
        log.flush()
        log.seek(0)
        return [SECRET_SHAPED.sub('[redacted]', ANSI.sub('', line).strip()) for line in log.read().splitlines()]

    try:
        for _ in range(300):
            if process.poll() is not None:
                raise RuntimeError('server exited: ' + ' | '.join(log_lines()[-5:]))
            try:
                urlopen(base + '/health/database', timeout=2).read()
                break
            except OSError:
                time.sleep(0.1)
        else:
            raise RuntimeError('server readiness timed out')
        yield base, log_lines
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


def login(base):
    api = Api(base)
    api.token = api.call('POST', '/api/auth/login', {'username': 'admin', 'password': PASSWORD})['token']
    return api


def put_labels(api, docs):
    for doc in docs:
        api.call('POST', '/api/documents', {'collection': 'labels', '_key': doc['set_id'],
                                            'title': doc['title'], 'content': doc['text']})


def embed(api, collection, items, expected_model, batch=100, provider_batch=32):
    models = set()
    for start in range(0, len(items), batch):
        response = api.call('POST', '/api/documents/embed', {
            'collection': collection, 'items': items[start:start + batch], 'batch_size': provider_batch})
        models.add(response.get('model'))
    if expected_model and models != {expected_model}:
        raise RuntimeError(f'{collection} embedded with {models}, expected {expected_model}')
    return sorted(m for m in models if m)


# ---------------------------------------------------------------- side views

def cmd_sideviews(args):
    chosen, docs = corpus(args.data)
    # The job embeds each side view as it stores it; only the text is kept
    # here (every embedder re-embeds it), so a high-throughput embedder is
    # pinned for this step.
    config = {'COGNIGRAPH_COMPLETION_PROVIDER': 'openai', 'COGNIGRAPH_COMPLETION_MODEL': args.model,
              'COGNIGRAPH_SIDEVIEWS_PROVIDER': 'openai', 'COGNIGRAPH_SIDEVIEWS_MODEL': args.model,
              'COGNIGRAPH_EMBEDDING_PROVIDER': 'openai', 'COGNIGRAPH_EMBEDDING_MODEL': 'text-embedding-3-small'}
    with server(args.bin / 'cognigraph-server', args.env_dir, config) as (base, log_lines):
        lanes = [line for line in log_lines() if 'provider configured' in line]
        if not any('Side-view generation provider configured' in line and args.model in line for line in lanes):
            raise RuntimeError(f'side-view lane is not {args.model}: {lanes}')
        api = login(base)
        for doc in docs:
            api.call('POST', '/api/documents', {'collection': 'labels', '_key': doc['set_id'],
                                                'title': doc['title'], 'text': doc['text']})
        submitted = api.call('POST', '/api/sideviews/generate', {'collection': 'labels', 'count': args.count},
                             status=(202,), headers={'Idempotency-Key': 'cg98-sideviews'})
        job_id = submitted['job']['id']
        resumes = 0
        while True:
            job = api.call('GET', f'/api/jobs/{job_id}')
            if job['status'] == 'failed' and (job.get('error') or {}).get('retryable') and resumes < 6:
                resumes += 1
                print(f'  resuming after: {(job.get("error") or {}).get("message", "")[:160]}', flush=True)
                time.sleep(60)
                api.call('POST', f'/api/jobs/{job_id}/retry', {'mode': 'resume', 'reason': 'transient provider error'},
                         headers={'Idempotency-Key': f'cg98-sideviews-resume-{resumes}'})
                continue
            if job['status'] in ('succeeded', 'failed', 'canceled'):
                break
            print(f'  side views: {job.get("progress", {}).get("completed")}/{len(docs)}', flush=True)
            time.sleep(15)
        if job['status'] != 'succeeded':
            raise RuntimeError(f'side-view job {job["status"]}: {job.get("error")}')
        rows = api.call('POST', '/api/search/query', {'query': (
            'FOR s IN side_views RETURN {id: s._key, document_id: s.document_id, '
            'question: s.question, answer: s.answer}')})['results']
    rows.sort(key=lambda row: (row['document_id'], row['question'], row['id']))
    FIXTURE.mkdir(parents=True, exist_ok=True)
    (FIXTURE / 'sideviews.jsonl').write_text(''.join(json.dumps(row, ensure_ascii=False) + '\n' for row in rows))
    (FIXTURE / 'sideviews-provenance.json').write_text(json.dumps({
        'model': args.model, 'count_requested': args.count, 'labels': len(docs), 'side_views': len(rows),
        'job_progress': job.get('progress'), 'provider_lines': lanes, 'job_resumes': resumes,
        'http_retries': api.retries,
        'generated_at': datetime.datetime.now(datetime.timezone.utc).isoformat()}, indent=2) + '\n')
    print(f'froze {len(rows)} side views')


# ---------------------------------------------------------------- run

def run_embedder(api, docs, sideviews, queries, expected_model, workers, provider_batch=32):
    put_labels(api, docs)
    chunks = [chunk for doc in docs for chunk in chunk_label(doc)]
    chunk_models = embed(api, 'label_chunks', chunks, expected_model, provider_batch=provider_batch)
    sv_items = [{'_key': row['id'], 'document_id': row['document_id'], 'text': row['question'],
                 'answer': row['answer']} for row in sideviews]
    sv_models = embed(api, 'sv', sv_items, expected_model, provider_batch=provider_batch)
    # CG-99: the first text searches after a write race while the persistent
    # text index rebuilds. One serial search after the last write rebuilds it
    # before the concurrent phase; every result is still checked below.
    api.call('POST', '/api/search/text', {'collection': 'labels', 'query': 'warm up', 'fields': ['content', 'title'],
                                          'limit': 1})
    jobs = [(query, name) for query in queries for name in CONFIGS]

    def search(job):
        query, name = job
        response = api.call('POST', '/api/search/hybrid', {'query': query['text'], **COMMON, **CONFIGS[name]})
        results = response['results']
        return {'id': query['id'], 'lang': query['lang'], 'target': query['target'], 'config': name,
                'rank': rank_of(query['target'], results),
                'top': [[r['document_id'].split('/', 1)[-1], round(r['score'], 6)] for r in results],
                'skipped': {k: response[k] for k in ('bm25', 'side_views') if k in response}}

    with ThreadPoolExecutor(max_workers=workers) as pool:
        rows = list(pool.map(search, jobs))
    degraded = [row for row in rows if row['skipped']]
    if degraded:
        raise RuntimeError(f'{len(degraded)} searches lost a leg, first: {degraded[0]["skipped"]}')
    return {'chunks': len(chunks), 'side_views': len(sv_items), 'models': sorted(set(chunk_models + sv_models)),
            'searches': rows}


def cmd_run(args):
    chosen, docs = corpus(args.data)
    sideviews = [json.loads(line) for line in (FIXTURE / 'sideviews.jsonl').read_text().splitlines() if line]
    queries = load_queries()
    provider, model = EMBEDDERS[args.embedder]
    config = {'COGNIGRAPH_EMBEDDING_PROVIDER': provider, 'COGNIGRAPH_EMBEDDING_MODEL': model}
    if provider == 'ollama':
        config['OLLAMA_BASE_URL'] = args.ollama_url
    started = datetime.datetime.now(datetime.timezone.utc).isoformat()
    with server(args.bin / 'cognigraph-server', args.env_dir, config) as (base, log_lines):
        lines = [line for line in log_lines() if 'provider' in line.lower()]
        if not any(PROVIDER_LOG[provider] in line for line in lines):
            raise RuntimeError(f'embedding provider is not {provider}: {lines}')
        # The server's Ollama client gives each request 120 s (CG-100); large
        # local models need smaller provider batches to finish in time.
        provider_batch = 8 if provider == 'ollama' else 32
        result = run_embedder(login(base), docs, sideviews, queries, model, args.workers, provider_batch)
    args.out.mkdir(parents=True, exist_ok=True)
    result.update(embedder=args.embedder, provider=provider, model=model, started_at=started,
                  finished_at=datetime.datetime.now(datetime.timezone.utc).isoformat(), provider_lines=lines,
                  labels=len(docs), queries=len(queries), provider_batch=provider_batch)
    (args.out / f'results-{args.embedder}.json').write_text(json.dumps(result, ensure_ascii=False) + '\n')
    summary = summarize(result['searches'])
    print(json.dumps({args.embedder: {k: v['mrr@10'] for k, v in summary.items() if k.startswith('en/')}}))


# ---------------------------------------------------------------- report

def summarize(searches):
    groups = {}
    for row in searches:
        groups.setdefault(f'{row["lang"]}/{row["config"]}', []).append(row)
    out = {}
    for key, rows in sorted(groups.items()):
        rows.sort(key=lambda row: row['id'])
        ranks = [row['rank'] for row in rows]
        m = metrics(ranks)
        low, high = bootstrap_ci([reciprocal(r) for r in ranks], seed=11)
        m['mrr@10_ci'] = [round(low, 4), round(high, 4)]
        low, high = bootstrap_ci([1.0 if r and r <= 5 else 0.0 for r in ranks], seed=13)
        m['recall@5_ci'] = [round(low, 4), round(high, 4)]
        out[key] = {k: (round(v, 4) if isinstance(v, float) else v) for k, v in m.items()}
    return out


def cmd_report(args):
    results = {path.stem.removeprefix('results-'): json.loads(path.read_text())
               for path in sorted(args.results.glob('results-*.json'))}
    report = {'embedders': {}, 'paired_vs_' + args.reference: {}}
    per_query = {}
    for name, result in results.items():
        report['embedders'][name] = {'model': result['model'], 'chunks': result['chunks'],
                                     'side_views': result['side_views'], 'metrics': summarize(result['searches'])}
        for row in result['searches']:
            per_query.setdefault((name, row['lang'], row['config']), {})[row['id']] = reciprocal(row['rank'])
    for (name, lang, config), values in sorted(per_query.items()):
        reference = per_query.get((args.reference, lang, config))
        if name == args.reference or not reference:
            continue
        ids = sorted(values)
        delta, low, high = paired_delta([values[i] for i in ids], [reference[i] for i in ids], seed=17)
        report['paired_vs_' + args.reference][f'{name}/{lang}/{config}'] = [round(delta, 4), round(low, 4),
                                                                             round(high, 4)]
    report['side_view_lift'] = {}
    for (name, lang, config), values in sorted(per_query.items()):
        if not config.endswith('+sv'):
            continue
        base = per_query.get((name, lang, config.removesuffix('+sv')))
        ids = sorted(values)
        delta, low, high = paired_delta([values[i] for i in ids], [base[i] for i in ids], seed=19)
        report['side_view_lift'][f'{name}/{lang}/{config}'] = [round(delta, 4), round(low, 4), round(high, 4)]
    (args.results / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    langs = sorted({key.split('/')[0] for r in report['embedders'].values() for key in r['metrics']})
    lines = []
    for config in CONFIGS:
        lines.append(f'\n{config}: MRR@10 by language')
        lines.append('| embedder | ' + ' | '.join(langs) + ' |')
        lines.append('|---|' + '---|' * len(langs))
        for name, data in report['embedders'].items():
            cells = [f'{data["metrics"].get(f"{lang}/{config}", {}).get("mrr@10", "-")}' for lang in langs]
            lines.append(f'| {name} | ' + ' | '.join(cells) + ' |')
    (args.results / 'report.md').write_text('\n'.join(lines) + '\n')
    print('\n'.join(lines))


# ---------------------------------------------------------------- preflight

class FakeEmbeddings(BaseHTTPRequestHandler):
    """OpenAI-compatible embeddings from hashed word counts: deterministic,
    meaningful enough to rank, and free."""

    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
        inputs = body['input'] if isinstance(body['input'], list) else [body['input']]
        data = []
        for index, text in enumerate(inputs):
            vector = [0.0] * 64
            for word in re.findall(r'[a-z]+', _fold(text).lower()):
                vector[int(hashlib.md5(word.encode()).hexdigest(), 16) % 64] += 1.0
            norm = math.sqrt(sum(v * v for v in vector)) or 1.0
            data.append({'embedding': [v / norm for v in vector], 'index': index})
        payload = json.dumps({'data': data, 'model': body.get('model'),
                              'usage': {'prompt_tokens': 1, 'total_tokens': 1}}).encode()
        self.send_response(200)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Content-Length', str(len(payload)))
        self.end_headers()
        self.wfile.write(payload)

    def log_message(self, *_):
        pass


def cmd_preflight(args):
    chosen, docs = corpus(args.data)
    docs = docs[:args.labels]
    keys = {doc['set_id'] for doc in docs}
    fake = ThreadingHTTPServer(('127.0.0.1', 0), FakeEmbeddings)
    threading.Thread(target=fake.serve_forever, daemon=True).start()
    queries = [{'id': f'p{i}', 'lang': 'en', 'target': doc['set_id'],
                'text': indications(doc)[:160]} for i, doc in enumerate(docs[:args.queries])]
    sideviews = [{'id': f'sv-{doc["set_id"]}', 'document_id': f'labels/{doc["set_id"]}',
                  'question': f'What is it used for? {indications(doc)[:80]}', 'answer': 'n/a'} for doc in docs]
    config = {'COGNIGRAPH_EMBEDDING_PROVIDER': 'openai', 'COGNIGRAPH_EMBEDDING_MODEL': 'fake-hash-64',
              'OPENAI_API_KEY': 'preflight-not-a-key', 'OPENAI_BASE_URL': f'http://127.0.0.1:{fake.server_port}',
              'GEMINI_API_KEY': ''}
    try:
        with server(args.bin / 'cognigraph-server', tempfile.mkdtemp(), config) as (base, log_lines):
            result = run_embedder(login(base), docs, sideviews, queries, 'fake-hash-64', args.workers)
    finally:
        fake.shutdown()
    summary = summarize(result['searches'])
    for key in ('en/bm25', 'en/vector', 'en/vector+sv', 'en/hybrid+sv'):
        print(key, summary[key])
    assert all(row['target'] in keys for row in result['searches'])
    assert summary['en/bm25']['recall@10'] > 0.5, 'BM25 over indications excerpts must find most targets'
    assert summary['en/vector']['recall@10'] > 0.0, 'the vector leg returned nothing'
    assert summary['en/vector+sv']['mrr@10'] >= summary['en/vector']['mrr@10'] - 0.2
    print(f'PASS: preflight ({len(docs)} labels, {result["chunks"]} chunks, {len(queries)} queries)')


def main():
    global FIXTURE
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--inputs', type=Path, default=FIXTURE, help='frozen benchmark inputs directory')
    sub = parser.add_subparsers(dest='command', required=True)
    for name in ('select', 'brief', 'sideviews', 'run', 'preflight'):
        command = sub.add_parser(name)
        command.add_argument('--data', type=Path, default=REPO / 'data/dailymed-pilot')
        if name in ('sideviews', 'run', 'preflight'):
            command.add_argument('--bin', type=Path, required=True)
            command.add_argument('--workers', type=int, default=4)
        if name in ('sideviews', 'run'):
            command.add_argument('--env-dir', type=Path, required=True)
    sub.choices['brief'].add_argument('--out', type=Path, required=True)
    sub.choices['sideviews'].add_argument('--model', default='gpt-5.6-luna')
    sub.choices['sideviews'].add_argument('--count', type=int, default=10)
    sub.choices['run'].add_argument('--embedder', choices=sorted(EMBEDDERS), required=True)
    sub.choices['run'].add_argument('--out', type=Path, required=True)
    sub.choices['run'].add_argument('--ollama-url', default='http://127.0.0.1:11434')
    sub.choices['preflight'].add_argument('--labels', type=int, default=20)
    sub.choices['preflight'].add_argument('--queries', type=int, default=20)
    report = sub.add_parser('report')
    report.add_argument('--results', type=Path, required=True)
    report.add_argument('--reference', default='gemini-2')
    args = parser.parse_args()
    FIXTURE = args.inputs.resolve()
    for name in ('data', 'bin', 'env_dir', 'out', 'results'):
        if getattr(args, name, None):
            setattr(args, name, getattr(args, name).resolve())
    {'select': cmd_select, 'brief': cmd_brief, 'sideviews': cmd_sideviews, 'run': cmd_run,
     'report': cmd_report, 'preflight': cmd_preflight}[args.command](args)


if __name__ == '__main__':
    main()
