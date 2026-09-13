# Diagnose and repair legacy Unicode references

CG-33 preserves exact reference identity for new writes. It does not recover
references normalized by earlier releases. For example, a source keyed
`cafe\u0301` could have been referenced as `caf\u00e9`; if both keys exist,
that reference resolves successfully to the wrong document.

The offline commands below operate on the Native JSON snapshot format returned
by the admin export endpoint. They never connect to a server or mutate a live
store. Read the [identity contract](../decisions/decision_exact_reference_identity.md)
before choosing a mapping.

## Prepare and inspect

1. Use the corrected release. Prevent concurrent writes and new job submissions;
   complete or cancel active jobs before the snapshot. Keep the affected tenant
   quiescent through import, including direct database writers. An import replaces
   snapshot state; importing an old snapshot after new writes can discard them.
2. Export the affected Native store using `cognigraph export --out snapshot.json`
   with the usual server URL/token configuration. Preserve that original and its
   access restrictions. Work on a bounded export, at most 64 MiB and 100,000
   documents. Do not split a snapshot arbitrarily: referenced targets must remain
   present for exact validation, and importing a partial snapshot can remove data.
3. Run the audit locally:

   ```bash
   cognigraph references audit snapshot.json
   ```

   The JSON report includes `snapshot_sha256`, unresolved or ambiguous references,
   exact non-NFC reference notices, and identity notices. `truncated: true` means
   not every finding is included; `findings_total` still counts all findings.
   Each finding lists at most 20 canonical candidates and their total count.
   A resolving NFC target can still be wrong. An empty report cannot prove
   historical correctness when the original spelling or target was lost.

The scan covers only root `_from`, `_to`, `document_id`, and a durable job's
`_execution.collection`/`keys`. It does not infer arbitrary application fields,
repair text, or validate every snapshot invariant. Collection names containing
`/` are flagged as unaddressable through a full handle. Exact non-NFC references
that resolve without competing forms should be preserved.

## Make a reviewed mapping

Choose the intended target using independent source evidence. Canonical
equivalence alone cannot select between two legitimate records. Save an
explicit plan such as the following, replacing the hash with the audit's actual
64-character hash and the collection/key with the affected record:

```json
{
  "snapshot_sha256": "<SHA-256 from the audit>",
  "changes": [
    {
      "collection": "legacy_edges",
      "key": "edge-key",
      "field": "_from",
      "from": "docs/caf\u00e9",
      "to": "docs/cafe\u0301"
    }
  ]
}
```

Run:

```bash
cognigraph references repair snapshot.json plan.json --out repaired.json
```

The output path must not exist. Each change requires the exact current `from`
value and an existing, distinct, canonically equivalent `to` handle. Plans allow
1–1,000 changes, only the shown fields, and no duplicate location. `_from` and
`_to` require an edge collection; `document_id` is the other allowed root field.
The command preserves every other JSON value and key, writes privately and
atomically without overwrite, and prints the source/output hashes and count.
JSON formatting may differ. It never renames or merges identities.

Compare the parsed source and output: only the approved fields should differ.
Audit the output again, retaining both reports and the plan with the backup.
Ambiguity notices can remain after a correct mapping because both exact keys
still exist. When the mapping and unchanged values are verified, import the new
snapshot into the same still-quiescent tenant using `cognigraph import repaired.json`.
Verify the exact documents, traversal/parent relationships, and persistence
after restart before admitting writes again. Admin import enforces its own
validation and job-quiescence checks; this repair command cannot waive them.

## Protected records and missing intent

The repair command refuses underscore-prefixed system collections and managed
`space_types`, `neurons`, `review_policies`, `eval_specs`, `entities`, `chunks`,
`mentions`, `facts`, `side_views`, and `fact_semantics`. It never edits signed
authority, custody, generated projections, or durable job payloads there.

Cancel affected queued/running jobs and resubmit from the exact source under a
fresh idempotency key. Completed jobs remain historical records; upgrading does
not turn an old zero-result job into a repaired run. Regenerate side-views from
verified exact parents, and use normal re-ingestion or governed re-derivation,
approval, and deployment for construction data. Do not patch frozen job payloads
or signed/custodied records. Application-defined signed data in ordinary
collections must also be excluded by the operator; the generic tool cannot
recognize an arbitrary application's signature scheme.

If no independent evidence establishes the intended exact target, retain the
finding for investigation. Creating a new target or choosing a canonical peer
by guesswork is outside this repair command.
