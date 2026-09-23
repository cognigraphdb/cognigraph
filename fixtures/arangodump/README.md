# ArangoDB dump fixtures (CG-64)

Real `arangodump` output of invented data, used to qualify the
[dump import contract](../../docs/reference/arangodump-import.md). No real
person, organization or record appears in any file.

| Path | Content |
|---|---|
| `3.11/`, `3.12/` | Dumps produced by the exporter version named, one directory per option variant |
| `derived/` | Copies of a real dump with one recorded corruption each |
| `expected/` | Collections, documents, carried constraints and not-carried items implied by the dataset definitions alone |
| `manifest.json` | Image digests, exporter versions, exact options or mutation, expected outcome and the size and SHA-256 of every file |

`python3 scripts/check-arangodump-fixtures.py` verifies all of it without
Docker and runs in CI.

## Provenance

- 3.11: `arangodb:3.11.14`, the last Apache-2.0 community line.
- 3.12: `arangodb/enterprise:3.12.11`. ArangoDB no longer publishes 3.12
  community images; the Enterprise image runs unlicensed in its evaluation
  mode, which is sufficient to produce dumps. The files are dumps of our own
  synthetic data, not ArangoDB software.

`manifest.json` records the image digests actually used.

## Regenerating

Requires Docker and about 2 GB for the two images.

```sh
docker pull arangodb:3.11.14
docker pull arangodb/enterprise:3.12.11
python3 scripts/arangodump_fixtures.py
python3 scripts/check-arangodump-fixtures.py
```

The generator loads [`scripts/arangodump_dataset.py`](../../scripts/arangodump_dataset.py)
over ArangoDB's HTTP API. It deliberately avoids `arangosh
--javascript.execute-string`, whose option parser expands `@name@` as an
environment variable and silently corrupted strings such as e-mail
addresses during exploration. Each run produces new revision ids and ticks,
so file hashes change while every expected outcome must stay the same. Review
the manifest's `observations` against the new exporter output when changing
versions.
