"""Reference reader for the ArangoDB dump import contract (CG-64).

Executable form of docs/reference/arangodump-import.md. It is the oracle the
CG-66 importer is qualified against: for every fixture both must reach the
same outcome, error codes, collections, documents, carried constraints and
reported dispositions. It favors clarity over speed and holds one collection
in memory at a time; the importer streams.
"""
import gzip
import hashlib
import json
from pathlib import Path
import re
import zlib

ROOT = Path(__file__).resolve().parents[1]
REPORT_SCHEMA = 'cognigraph-arangodump-report-v1'
LIMITS = {'max_record_bytes': 16 << 20, 'max_expanded_bytes': 64 << 30, 'max_files': 100_000}
STRUCTURE = re.compile(r'^(?P<stem>.+_[0-9a-f]{32})\.structure\.json$')
DATA = re.compile(r'^(?P<stem>.+_[0-9a-f]{32})(?:\.(?P<part>\d+))?\.data\.json(?P<gz>\.gz)?$')
VPACK = re.compile(r'^.+_[0-9a-f]{32}(?:\.\d+)?\.data\.vpack(?:\.gz)?$')
VIEW = re.compile(r'^(?P<name>.+)\.view\.json$')
IDENTIFIER = re.compile(r'^[A-Za-z][A-Za-z0-9_]*$')
IMPLICIT_INDEXES = {'primary', 'edge'}
UNIQUE_TYPES = {'persistent', 'hash', 'skiplist'}
MARKER_DOCUMENT = 2300


def reserved_names():
    """CGQL reserved words and server-owned collection names, read from the Rust
    sources so the contract cannot drift from what the server refuses."""
    validation = (ROOT / 'crates/cognigraph-query/src/validation.rs').read_text()
    body = validation.split('fn is_reserved_word', 1)[1].split('\n}', 1)[0]
    words = {word.lower() for word in re.findall(r'"([A-Z_]+)"', body)}
    system = (ROOT / 'crates/cognigraph-server/src/system_collections.rs').read_text()
    owned = set()
    for block in re.findall(r'const (?:MANAGED|GENERATED)_COLLECTIONS[^=]*=\s*\[(.*?)\];', system, re.S):
        owned |= set(re.findall(r'"([a-z_]+)"', block))
        for constant in re.findall(r'\b([A-Z_]+_COLLECTION)\b', block):
            owned |= set(re.findall(rf'{constant}\s*:\s*&str\s*=\s*"([a-z_]+)"', system + validation))
    return words, owned


class Rejected(Exception):
    pass


def error(code, **context):
    return {'code': code, **{k: v for k, v in context.items() if v is not None}}


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def lines_of(path, limits, budget):
    """Yield (line number, raw bytes) with record and expansion bounds."""
    opener = gzip.open if path.name.endswith('.gz') else open
    try:
        with opener(path, 'rb') as handle:
            for number, raw in enumerate(handle, 1):
                budget['expanded'] += len(raw)
                if len(raw) > limits['max_record_bytes']:
                    raise Rejected(error('record_too_large', file=path.name, line=number))
                if budget['expanded'] > limits['max_expanded_bytes']:
                    raise Rejected(error('dump_too_large', file=path.name))
                if raw.strip():
                    yield number, raw
    except (OSError, EOFError, zlib.error) as failure:
        raise Rejected(error('corrupt_data_file', file=path.name, detail=type(failure).__name__))


def value_key(document, fields, sparse):
    values = []
    for field in fields:
        current = document
        for part in field.split('.'):
            current = current.get(part) if isinstance(current, dict) else None
        if current is None and sparse:
            return None
        values.append(current)
    return json.dumps(values, sort_keys=True, separators=(',', ':'), ensure_ascii=False)


def classify(directory, limits):
    entries = sorted(directory.iterdir())
    if len(entries) > limits['max_files']:
        raise Rejected(error('dump_too_large', detail='file count'))
    for entry in entries:
        if entry.is_symlink():
            raise Rejected(error('unsafe_path', file=entry.name))
        if entry.is_dir():
            code = 'multiple_databases' if (entry / 'dump.json').exists() else 'unknown_layout'
            raise Rejected(error(code, file=entry.name))
    names = {entry.name for entry in entries}
    marker = directory / 'ENCRYPTION'
    if marker.exists() and marker.read_text(errors='replace').strip() != 'none':
        raise Rejected(error('encrypted_unsupported'))
    if 'dump.json' not in names:
        raise Rejected(error('missing_dump_metadata'))
    try:
        metadata = json.loads((directory / 'dump.json').read_text())
    except (UnicodeDecodeError, json.JSONDecodeError):
        raise Rejected(error('corrupt_dump_metadata'))
    if metadata.get('useVPack') or any(VPACK.match(name) for name in names):
        raise Rejected(error('vpack_unsupported'))
    layout = {'structures': {}, 'data': {}, 'views': [], 'ignored': []}
    for name in sorted(names - {'dump.json', 'ENCRYPTION'}):
        if match := STRUCTURE.match(name):
            layout['structures'][match['stem']] = name
        elif match := DATA.match(name):
            layout['data'].setdefault(match['stem'], []).append((int(match['part'] or -1), name))
        elif match := VIEW.match(name):
            layout['views'].append(match['name'])
        elif name.startswith('.'):
            layout['ignored'].append(name)
        else:
            raise Rejected(error('unknown_layout', file=name))
    orphans = sorted(set(layout['data']) - set(layout['structures']))
    if orphans:
        raise Rejected(error('unknown_layout', detail=f'data without structure: {orphans[0]}'))
    return metadata, layout


def check_name(name, reserved):
    words, owned = reserved
    if not IDENTIFIER.match(name):
        return error('incompatible_collection_name', collection=name)
    if name.lower() in words or name in owned:
        return error('reserved_collection_name', collection=name)
    return None


def indexes_of(name, kind, definitions, report):
    carried = []
    for index in definitions:
        if index.get('type') in IMPLICIT_INDEXES:
            continue
        fields = index.get('fields') or []
        plain = all(isinstance(f, str) and '[*]' not in f for f in fields)
        if index.get('type') in UNIQUE_TYPES and plain and index.get('unique'):
            if kind == 'edge':
                reason = 'edge_unique_unsupported'
            else:
                carried.append({'collection': name, 'unique': True, 'fields': fields,
                                'sparse': bool(index.get('sparse'))})
                continue
        elif index.get('type') in UNIQUE_TYPES and plain:
            reason = 'non_unique_index'
        else:
            reason = 'unsupported_index_type'
        report['not_carried'].append({'collection': name, 'kind': 'index',
                                      'detail': index.get('name'), 'reason': reason})
    return carried


def read(directory, limits=None):
    """Return the import report for one dump directory; never raises for input
    problems, which become `status: rejected` with error codes."""
    directory, limits = Path(directory), {**LIMITS, **(limits or {})}
    report = {'schema': REPORT_SCHEMA, 'status': 'accepted', 'database': None, 'collections': {},
              'constraints': [], 'not_carried': [], 'excluded': [], 'ignored': [],
              'warnings': [], 'dropped': {'_rev': 0}, 'errors': [], 'files': []}
    try:
        metadata, layout = classify(directory, limits)
    except Rejected as rejected:
        return {**report, 'status': 'rejected', 'errors': [rejected.args[0]]}
    report['database'] = metadata.get('database')
    report['ignored'] = layout['ignored']
    report['files'] = [{'path': p.name, 'bytes': p.stat().st_size, 'sha256': sha256(p)}
                       for p in sorted(directory.iterdir()) if p.name not in layout['ignored']]
    envelope = bool(metadata.get('useEnvelope'))
    split = any(part >= 0 for parts in layout['data'].values() for part, _ in parts)
    reserved, budget, errors = reserved_names(), {'expanded': 0}, report['errors']
    for view in layout['views']:
        report['not_carried'].append({'collection': None, 'kind': 'view', 'detail': view,
                                      'reason': 'unsupported_metadata'})
    keys, failed = {}, set()
    for stem, structure_name in sorted(layout['structures'].items()):
        try:
            structure = json.loads((directory / structure_name).read_text())
            parameters = structure['parameters']
            name, kind = parameters['name'], {2: 'document', 3: 'edge'}[parameters['type']]
        except (UnicodeDecodeError, json.JSONDecodeError, KeyError, TypeError):
            errors.append(error('corrupt_structure_file', file=structure_name))
            continue
        if name.startswith('_') or parameters.get('isSystem'):
            report['excluded'].append({'collection': name, 'reason': 'system_collection'})
            continue
        if problem := check_name(name, reserved):
            errors.append(problem)
            continue
        for field, reason in (('schema', 'unsupported_metadata'), ('computedValues', 'unsupported_metadata')):
            if parameters.get(field):
                report['not_carried'].append({'collection': name, 'kind': 'schema' if field == 'schema'
                                              else 'computed_values', 'detail': 'validation rule'
                                              if field == 'schema' else field, 'reason': reason})
        key_type = (parameters.get('keyOptions') or {}).get('type', 'traditional')
        if key_type != 'traditional':
            report['not_carried'].append({'collection': name, 'kind': 'key_generator',
                                          'detail': key_type, 'reason': 'unsupported_metadata'})
        parts = sorted(layout['data'].get(stem, []))
        if not parts and not split:
            errors.append(error('missing_data_file', collection=name))
            failed.add(name)
            continue
        documents = {}
        # Dispositions are decided up front so unique constraints are checked
        # inline: within a collection the first problem in file order wins,
        # which a streaming importer can reproduce exactly.
        pending = {'not_carried': []}
        constraints = indexes_of(name, kind, structure.get('indexes', []), pending)
        held = [dict() for _ in constraints]
        try:
            for _, data_name in parts:
                for number, raw in lines_of(directory / data_name, limits, budget):
                    try:
                        record = json.loads(raw)
                    except (UnicodeDecodeError, json.JSONDecodeError):
                        raise Rejected(error('corrupt_data_file', file=data_name, line=number))
                    if envelope:
                        if not isinstance(record, dict) or record.get('type') != MARKER_DOCUMENT:
                            raise Rejected(error('unsupported_marker', file=data_name, line=number))
                        record = record.get('data')
                    key = record.get('_key') if isinstance(record, dict) else None
                    if not isinstance(key, str) or not key:
                        raise Rejected(error('invalid_document', file=data_name, line=number))
                    if key in documents:
                        raise Rejected(error('duplicate_key', collection=name, key=key))
                    if record.get('_id', f'{name}/{key}') != f'{name}/{key}':
                        raise Rejected(error('identity_mismatch', collection=name, key=key))
                    if kind == 'edge' and not all(
                            isinstance(record.get(end), str) and '/' in record[end] for end in ('_from', '_to')):
                        raise Rejected(error('invalid_edge', collection=name, key=key))
                    for constraint, seen in zip(constraints, held):
                        value = value_key(record, constraint['fields'], constraint['sparse'])
                        if value is not None and value in seen:
                            raise Rejected(error('unique_violation', collection=name, key=key))
                        if value is not None:
                            seen[value] = key
                    report['dropped']['_rev'] += '_rev' in record
                    documents[key] = {k: v for k, v in record.items() if k not in ('_id', '_rev')}
        except Rejected as rejected:
            errors.append(rejected.args[0])
            failed.add(name)
            continue
        report['collections'][name] = {'type': kind, 'documents': documents}
        keys[name] = set(documents)
        report['not_carried'] += pending['not_carried']
        report['constraints'] += constraints
    for name, collection in report['collections'].items():
        if collection['type'] != 'edge':
            continue
        for key, edge in collection['documents'].items():
            for end in ('_from', '_to'):
                target, _, target_key = edge[end].partition('/')
                # A collection that failed to load already carries its own
                # error; its edges are not reported again as unresolved.
                if target not in failed and target_key not in keys.get(target, ()):
                    errors.append(error('unresolved_edge', collection=name, key=key))
                    break
    if not errors and report['collections'] and not any(
            c['documents'] for c in report['collections'].values()):
        report['warnings'].append('no_documents')
    if errors:
        report['status'] = 'rejected'
    return report
