#!/usr/bin/env python3
"""Read-only dependency freshness: current direct releases and fresh resolutions."""
from concurrent.futures import ThreadPoolExecutor
from datetime import date, datetime, timezone
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import tomllib
from urllib.parse import quote
from urllib.request import Request, urlopen

ROOT = Path(__file__).resolve().parents[1]
AGENT = 'CogniGraph dependency gate (https://github.com/cognigraphdb/cognigraph)'


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def run(command, cwd, log):
    result = subprocess.run(command, cwd=cwd, text=True, capture_output=True, timeout=600)
    log.write_text(result.stdout + '\n' + result.stderr)
    require(result.returncode == 0, f'{command[0]} failed ({result.returncode}); see {log}')
    return result.stdout


def latest(ecosystem, package):
    base = 'https://crates.io/api/v1/crates/' if ecosystem == 'cargo' else 'https://registry.npmjs.org/'
    suffix = '' if ecosystem == 'cargo' else '/latest'
    request = Request(base + quote(package, safe='') + suffix,
                      headers={'User-Agent': AGENT, 'Accept': 'application/json'})
    with urlopen(request, timeout=30) as response:
        data = json.load(response)
    # Some explicitly adopted libraries (ort) publish only prereleases.
    version = (data['crate']['max_stable_version'] or data['crate']['max_version']) \
        if ecosystem == 'cargo' else data['version']
    require(isinstance(version, str) and version.strip(), f'No published release for {package}')
    return version


def cargo_direct(metadata):
    members = set(metadata['workspace_members'])
    packages = {p['id']: p for p in metadata['packages']}
    ids = {dep['pkg'] for node in metadata['resolve']['nodes'] if node['id'] in members
           for dep in node['deps']} - members
    result = set()
    for identity in ids:
        package = packages[identity]
        source = package.get('source')
        require(source in (None, 'registry+https://github.com/rust-lang/crates.io-index'),
                f"Unsupported dependency registry/source for {package['name']}: {source}")
        # Include reviewed local patches such as Tantivy in upstream freshness checks.
        result.add(('cargo', package['name'], package['version']))
    return result


def cargo_resolution(lock):
    return {(p['name'], p['version'], p.get('source', 'workspace-or-patch'))
            for p in tomllib.loads(lock.read_text())['package']}


def bun_lock(path, reports, label):
    return json.loads(run(['bun', '-e',
        'console.log(JSON.stringify(Bun.JSONC.parse(await Bun.file(process.argv[1]).text())))',
        str(path)], ROOT, reports / f'{label}-lock.log'))


def npm_direct(manifest, lock):
    names = set().union(*(manifest.get(k, {}) for k in
                         ('dependencies', 'devDependencies', 'optionalDependencies')))
    result = set()
    for name in names:
        entry = lock['packages'][name][0]
        package, separator, version = entry.rpartition('@')
        require(separator and package == name and version, f'Unsupported npm lock entry: {name}')
        result.add(('npm', name, version))
    return result


def npm_resolution(lock):
    return {(name, entry[0]) for name, entry in lock['packages'].items()}


def resolutions(reports):
    metadata = json.loads(run(['cargo', 'metadata', '--locked', '--format-version', '1',
                               '--all-features'], ROOT, reports / 'cargo-metadata.log'))
    direct = cargo_direct(metadata)
    current_cargo = cargo_resolution(ROOT / 'Cargo.lock')
    current_bun = bun_lock(ROOT / 'ui/bun.lock', reports, 'current-bun')
    direct |= npm_direct(json.loads((ROOT / 'ui/package.json').read_text()), current_bun)
    # Resolve updates in a disposable copy; never update the candidate or run scripts.
    with tempfile.TemporaryDirectory(prefix='cognigraph-dependencies-') as folder:
        scratch = Path(folder)
        for name in ('Cargo.toml', 'Cargo.lock'):
            shutil.copy2(ROOT / name, scratch / name)
        for name in ('crates', 'vendor'):
            shutil.copytree(ROOT / name, scratch / name,
                            ignore=shutil.ignore_patterns('target', '.git', '__pycache__'))
        run(['cargo', 'update'], scratch, reports / 'cargo-resolution.log')
        new_cargo = cargo_resolution(scratch / 'Cargo.lock')
        ui = scratch / 'ui'
        ui.mkdir()
        for name in ('package.json', 'bun.lock'):
            shutil.copy2(ROOT / 'ui' / name, ui / name)
        run(['bun', 'update', '--lockfile-only', '--ignore-scripts', '--no-cache'], ui,
            reports / 'bun-resolution.log')
        new_bun = bun_lock(ui / 'bun.lock', reports, 'resolved-bun')
    return direct, {
        'cargo': {'removed': sorted(current_cargo - new_cargo),
                  'added': sorted(new_cargo - current_cargo)},
        'npm': {'removed': sorted(npm_resolution(current_bun) - npm_resolution(new_bun)),
                'added': sorted(npm_resolution(new_bun) - npm_resolution(current_bun))}}


def exceptions(path, today):
    records = json.loads(path.read_text())
    require(isinstance(records, list), 'Dependency exceptions must be a list')
    seen = set()
    for record in records:
        require(set(record) == {'ecosystem', 'package', 'current', 'latest', 'reason',
                                'decision', 'expires'}, 'Invalid dependency exception fields')
        require(all(isinstance(v, str) and v.strip() for v in record.values()),
                'Incomplete dependency exception')
        require(record['ecosystem'] in ('cargo', 'npm'), 'Unknown exception ecosystem')
        require(date.fromisoformat(record['expires']) >= today, 'Expired dependency exception')
        decision = (ROOT / record['decision']).resolve()
        require(decision.is_relative_to((ROOT / 'docs').resolve()) and decision.is_file()
                and decision.suffix == '.md', 'Exception requires a local engineering decision')
        key = tuple(record[k] for k in ('ecosystem', 'package', 'current', 'latest'))
        require(key not in seen, 'Duplicate dependency exception')
        seen.add(key)
    return records


def assess(direct, versions, changes, records):
    pending, reviewed, used = [], [], set()
    for ecosystem, package, current in sorted(direct):
        newest = versions[(ecosystem, package)]
        if current == newest:
            continue
        update = dict(ecosystem=ecosystem, package=package, current=current, latest=newest)
        match = next((i for i, r in enumerate(records)
                      if all(r[k] == value for k, value in update.items())), None)
        if match is None:
            pending.append(update)
        else:
            reviewed.append({**update, 'decision': records[match]})
            used.add(match)
    require(used == set(range(len(records))), 'Unused or stale dependency exception; review it')
    drift = any(change['added'] or change['removed'] for change in changes.values())
    return {'direct_updates': pending, 'reviewed_pins': reviewed,
            'resolution_changes': changes, 'passed': not pending and not drift}


def main():
    parent = ROOT / 'target/ci/dependencies'
    parent.mkdir(parents=True, exist_ok=True)
    reports = Path(tempfile.mkdtemp(prefix='run-', dir=parent))
    inputs = [ROOT / 'Cargo.toml', ROOT / 'Cargo.lock', ROOT / 'ui/package.json', ROOT / 'ui/bun.lock',
              ROOT / 'docs/operations/dependency-exceptions.json',
              *sorted((ROOT / 'crates').glob('*/Cargo.toml')),
              *sorted((ROOT / 'vendor').glob('*/Cargo.toml'))]
    before = {str(p.relative_to(ROOT)): hashlib.sha256(p.read_bytes()).hexdigest() for p in inputs}
    summary = {'checked_at': datetime.now(timezone.utc).isoformat(), 'inputs': before}
    try:
        direct, changes = resolutions(reports)
        summary['resolution_changes'] = changes
        names = sorted({(ecosystem, package) for ecosystem, package, _ in direct})
        versions, errors = {}, []
        with ThreadPoolExecutor(max_workers=2) as pool:
            futures = {name: pool.submit(latest, *name) for name in names}
            for name, future in futures.items():
                try:
                    versions[name] = future.result()
                except (OSError, ValueError, RuntimeError, KeyError) as error:
                    errors.append({'ecosystem': name[0], 'package': name[1], 'error': str(error)})
        summary['direct_inventory'] = [{'ecosystem': e, 'package': p, 'current': v,
                                        'latest': versions.get((e, p))} for e, p, v in sorted(direct)]
        summary['registry_errors'] = errors
        require(not errors, 'Registry lookups failed; see the dependency report')
        records = exceptions(ROOT / 'docs/operations/dependency-exceptions.json',
                             datetime.now(timezone.utc).date())
        summary.update(assess(direct, versions, changes, records))
        require(all(hashlib.sha256(p.read_bytes()).hexdigest() == before[str(p.relative_to(ROOT))]
                    for p in inputs), 'Candidate dependency inputs changed during freshness checks')
        for row in summary['direct_updates']:
            print(f"UPDATE {row['ecosystem']} {row['package']}: {row['current']} -> {row['latest']}")
        for name, change in changes.items():
            print(f"{name} resolution: {len(change['removed'])} removed, {len(change['added'])} added identities")
        require(summary['passed'], 'Dependency freshness blocks integration; update and requalify')
        print(f'PASS: {len(direct)} direct dependency identities are current or explicitly reviewed')
    except Exception as error:
        summary.update(passed=False, error=str(error))
        raise
    finally:
        (reports / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
        print(f'Dependency report: {reports}', flush=True)


if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError, RuntimeError, KeyError, subprocess.SubprocessError) as error:
        print(f'Dependency gate failed: {error}', file=sys.stderr)
        raise SystemExit(1)
