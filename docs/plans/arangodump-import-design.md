# ArangoDB dump import into Native — deferred design

- Status: Deferred optional backlog; no dump-import mode exists in CogniGraph 2.7.1
- Direction: [Native-only decision](../decisions/decision_native_only.md)
- Qualification: [CG-64](../issues/CG-64.md)
- Implementation: [CG-66](../issues/CG-66.md)

## Purpose and current behavior

The owner confirms that CogniGraph has never been provided to anyone and live
deployment starts only after Native-only readiness. This proposal is not part
of that active batch. CG-64/CG-66 may be reconsidered later for external users
moving data from Arango; no work here is required to remove the backend or start
the first live deployment. The requirements below apply only if this feature
is prioritized and qualified; they are not an implemented runtime contract.

Provide Community users with an offline path from an ArangoDB dump to a new
Native database. The proposed CLI mode is `cognigraph import --from-arangodump`;
its complete argument syntax would be frozen in CG-64 if resumed. This is not a runnable
command in the current release. It must also be available in Enterprise builds
without requiring Enterprise authority or an external database service.

Today, `cognigraph import snapshot.json` sends a CogniGraph JSON snapshot to
the authenticated [application import API](../operations/recovery.md). That
format is not an Arango dump, import is additive, and the CLI reads the payload
as a whole. Preserve that command's existing contract; a large dump must not be
implemented by wrapping it in one unbounded HTTP snapshot request.

## V1 target and qualification gate

Target unencrypted, single-database ArangoDB 3.11/3.12 JSON dumps with collection
structure and document/edge data. Plain JSON, gzip and split-file layouts must
each have an explicit supported/rejected entry in the CG-64 profile. A version
number alone is not a compatibility claim; exporter flags and the actual file
layout must match qualified fixtures. Exact supported versions and flags remain
pending that evidence.

Arango documents structure files containing collection parameters and indexes,
data markers, gzip output, optional encryption and 3.12 split files. Use its
[dump examples](https://docs.arango.ai/arangodb/stable/components/tools/arangodump/examples/)
as a starting reference, then capture bytes from the actual chosen exporter.
VPack/binary, encrypted, multi-database, structure-only and unknown layouts must
be rejected with actionable diagnostics unless a separately qualified extension
explicitly admits them. Do not infer support from filename suffixes alone.

## Data and identity rules

- Preserve accepted collection names, `_key`, `_from`, `_to`, Unicode strings
  and user JSON types/values. Validate `_id` against its collection/key; any
  reconstructed ID must be identical. Do not implicitly normalize, rename,
  merge duplicates, stringify JSON or invent missing edge targets.
- Preflight identifier compatibility with Native and public CGQL access. V1
  refuses incompatible identities with a report; implicit name remapping is
  not part of this batch. Record which supported dump markers are applied and
  reject ambiguous ordering/duplicate semantics rather than guessing.
- Validate all imported edge endpoints against the selected import scope.
  Unresolved endpoints block publication in V1. Document and edge collection
  types must be preserved, including empty collections.
- Arango `_rev` is not a Native revision. Define its explicit disposition in
  the qualified mapping and report it; do not promote it into Native storage
  authority. Never manufacture construction evidence or signed records.
- Source users, permissions, tokens, jobs, tenant/control collections and signed
  governance state are outside V1. Classify and report excluded system metadata;
  refuse a dump that requires unsupported application-state migration. This
  concerns future external input, not existing CogniGraph customers.
- Named graph definitions, views/analyzers, index definitions, uniqueness/schema
  constraints and other Arango metadata need an explicit disposition. Preflight
  reports must distinguish transferred graph data from unavailable behavior;
  unsupported constraints must not silently appear to remain enforced.

## Safe execution and reporting

A dry run validates the selected input profile, identities, references and
metadata without creating a destination or contacting a server. It reports
collection/document/edge counts, source fingerprints, exclusions, unsupported
behavior and errors without printing credentials or document bodies. Exit
nonzero when the requested migration cannot satisfy its declared contract.

The import path reads with bounded buffers and disk-backed staging as needed;
gzip expansion, record size, total work and filesystem traversal need explicit
limits. Define safe handling of symlinks, unexpected files and source changes.
Check input fingerprints again before publication so validation and import cannot
silently consume different source bytes.

Build and validate a temporary Native store, close and durably publish it to a
previously absent destination. Never overwrite or merge into an existing live
store, and never edit source dumps. Corruption, cancellation, disk errors or
process interruption must not expose a partial final database; recovery/cleanup
is limited to staging files owned by this invocation. Atomicity and crash behavior
must be proven on the supported publication path, not inferred from a rename.

Return a machine-readable migration report bound to importer version, source
file hashes and destination identity, including applied counts, metadata
dispositions, verification results and completion state. Specify report-write
failure semantics so a committed database cannot be misreported as absent and
a retry cannot overwrite it. Successful dry run is not successful import.

## Qualification and operational limits

Use newly captured synthetic fixtures with source provenance and expected data.
Include plain/compressed/split variants according to the final support matrix,
Unicode identities, nested JSON, empty collections, edge graphs and unsupported
metadata, plus malformed input, duplicate keys, unresolved references and
interrupted publication. Preserve old research and query evidence unchanged.

After importing, verify persisted data and graph queries with a real Native
server and repeat after restart in the supported storage modes. Bootstrap new
application credentials separately. Source dumps and the original Arango
deployment remain operator-owned recovery material. This command neither
performs online replication nor migrates live writes or guarantees rollback
of changes made after cutover.
