# Governance

## Signed evaluation, Semantic Repair authority, and deployment (M18-M26)

M18 turns deterministic `construct.evaluate` results into governed selection
evidence; it does not deploy a binary, rewrite a graph, or switch an external
consumer. M19 wraps that unchanged evaluation contract in signed,
three-principal authorization. M20 adds a separate Artifact Attestor and
content-addressed exact-byte manifest claims for corpus, graph, oracle, scorer,
and verifier inputs. M21 verifies and consumes those exact staged bytes for a
new context generation. M22 additionally replays a pinned derivation from
verified prepared chunks and a verified construction candidate to the claimed
evaluation-fact graph. M23 adds a fresh generation that reproduces those
prepared chunks from exact verified UTF-8 plain-text document bytes before
running the unchanged M22 derivation. Promotion operations are tenant-local and
fail closed
unless auth is enabled. An auth-disabled server may still run legacy diagnostic
evaluations, but it cannot create evidence or make promotion decisions. The
M19, M20, and M21 adversarial suites and release-binary Native/live-Arango
lifecycles are verified in their decision records. M22's final persistent
Native and configured live-ArangoDB release-binary probes are also verified.
The M21 and M22 probes include restart/recovery, CAS tamper, prospective
Artifact Attestor revocation, and exact cleanup; M22 additionally rejects a
signed score-equivalent graph with forged evidence.
M23's authenticated release-binary lifecycle is verified on persistent Native
and configured live ArangoDB Enterprise 3.12.9-1. Both probes cover
restart/recovery, signed inconsistent prepared-output rejection,
`documents.json` CAS-tamper rejection, prospective Artifact Attestor
revocation/history fencing, and isolated cleanup.

M24 adds offline custody bundle/restore around those external bytes. M25 binds
the exact construction candidate to a PolicyAuthor-signed revision,
independent PolicyApprover review, and the unchanged current promotion head.
M26 consumes only that current M22/M23 plus M25 authority into an explicit
verified immutable occurrence generation, then requires a separate signed
Promoter deployment act for a Native atomic target switch. Neither promotion
nor generation build activates graph rows automatically.

A promotable evaluation submission includes a versioned `promotion_context`
inside the job input. The closed schema freezes the target
`{space_type, channel}`, candidate artifact, effective construction/evaluation
configuration, canonical EvalSpec and case-manifest identities, externally
attested corpus/graph/oracle revisions, clean deterministic reproduction
description, resolved integer policy, and oracle-separation attestation. The
server rejects unknown fields, empty or malformed expected/forbidden sets,
duplicate question IDs, overlapping facts, dirty/provider-backed runs, mutable
revision declarations, and manifest counts or digests that disagree with the
resolved EvalSpec. Legacy jobs without the context remain inspectable but are
never promotable.

M18 v1 has deliberately closed semantics:

- exclusions are unsupported: `case_manifest.exclusions` and
  `policy.exclusions.allowed_reason_codes` must be empty,
  `policy.exclusions.max_count` must be `0`, and the exclusions digest must be
  the canonical digest of the empty list;
- `allowed_candidate_differences` is a non-empty, sorted, unique allowlist
  drawn only from `candidate_identity`, `construction_configuration`,
  `graph_revision`, and `resolved_configuration`; every other
  candidate/baseline dimension remains equal;
- the scorer and oracle-verifier ids, versions, and hashes are pinned
  operator-trusted **semantics identities**. CogniGraph checks those exact
  values, but they are not hashes of a fetched scorer/verifier executable.
  M20 preserves that meaning and binds separate content-addressed manifests to
  the exact scorer/verifier usage subjects;
- the oracle policy and attestation contain exactly the ordered stages
  `candidate_build`, `candidate_tuning`, `construction`, and `evaluation`.
  Oracle reads must be zero in the first three stages; evaluation may read the
  oracle. Status must be `verified`, the pinned verifier identity must match,
  and document, case, content, and artifact overlap counters must all be zero.

M19 adds authority around those gates without weakening them:

1. An external root signs a purpose-bound registration for each tenant user.
   The server stores only the public registration. An Admin submits
   registrations and prospective revocations but cannot use the registered
   principal's signing authority.
2. The `policy-author` user submits a signed immutable resolved-policy
   revision. The `policy-approver` user, with a different stable
   `principal_id`, signs exactly that revision, target, and digest.
3. `GET /api/governance/bindings/{approval_id}` returns the exact public
   governance binding to place in `PromotionContext` schema version 2. All
   four candidate/baseline original/replay jobs must freeze the same binding.
4. The `promoter` user, whose `principal_id` differs from both policy
   principals, registers the resulting evidence and supplies a signed intent
   for promote, reject, or rollback. The signature does not bypass any M18
   gate or stale-head compare-and-set.

M20 adds exact external artifact authority around that workflow:

1. Create a same-tenant `artifact-attestor` user. An Admin submits an
   externally root-signed, proof-of-possession key registration using purpose
   `artifact_attestor` and domain `cognigraph.key-registration.v2`. The stable
   attestor principal cannot also hold the author, approver, or promoter duty.
2. Outside CogniGraph, hash every byte stream and build a canonical manifest
   for exactly one kind: `corpus`, `graph`, `oracle`, `scorer`, or `verifier`.
   Entries must be sorted by unique portable NFC logical path and bind media
   type, byte length, SHA-256 blob digest, and executable status. Declared
   counts and total bytes must match.
3. Bind the manifest to its exact kind-specific evaluation subject and one to
   eight location observations, then sign the complete
   `cognigraph.artifact-attestation.v1` statement with the Artifact Attestor
   key. Locations must not contain userinfo, queries, fragments, whitespace,
   or credentials.
4. Submit the five immutable attestations and call
   `POST /api/governance/artifact-bindings/resolve` with their ids. The response
   is the exact five-slot set to freeze in `PromotionContext` schema version 3
   alongside the M19 policy-governance binding.
5. Run candidate and baseline original/replay jobs. Each original/replay pair
   must reuse its exact set. Candidate and baseline share corpus, oracle,
   scorer, and verifier bindings; only a policy-authorized `graph_revision`
   difference may carry another graph binding. Evidence stores both sets and
   the promoter's signed intent binds their aggregate authority digest.

The manifest limits are 100,000 entries, 1 TiB declared total bytes, and 16 MiB
of canonical manifest JSON. Artifact-attestation HTTP bodies alone are capped
at 17 MiB so the signed statement envelope fits around a maximum-size canonical
manifest; other governance routes keep the default request bound. Each tenant
incarnation may store at most 10,000 immutable artifact attestations and 64 MiB
of canonical manifest JSON in aggregate. These are admission and recovery
limits; they do not make CogniGraph an artifact store.

Artifact-attestation lists are keyset-paginated at most 50 compact summaries
per page. A summary excludes the potentially large manifest, signed subject,
location observations, and detached signature; use the single-record detail
route or `governance artifact show` when those fields are required.

The server validates the canonical manifest, natural identity, signature,
active key lifecycle, timestamps, and exact promotion subject bindings. It
never fetches or independently rehashes a signed location. The attestor's
signature authenticates the exact-byte claim but does not establish semantic
truth, current availability, custody, or freshness. `construct.evaluate` still
reads the live tenant graph, so M20 also does not prove that evaluation consumed
the attested graph bytes.

M21 adds an opt-in consumption path without changing the M20 location rule:

1. Stage every manifest blob outside CogniGraph under the tenant-incarnation
   local-CAS scope. For tenant `T` and incarnation `I`, compute lowercase
   `sha256(T || NUL || I)` and place a blob with digest `sha256:<hex>` at
   `<root>/tenants/<scope>/sha256/<hex[0:2]>/<hex[2:64]>`. Create the complete
   normal-directory hierarchy before starting the server. Do not use a signed
   logical path or location URI as a filesystem path.
2. Start the server with `COGNIGRAPH_ARTIFACT_SOURCE=local-cas`, an existing
   absolute `COGNIGRAPH_ARTIFACT_CAS_ROOT`, and a positive byte budget. The
   server performs no writes below this custody boundary; mount it read-only.
   There is no staging API.
3. Use M20 attestations with the exact M21 formats. Corpus uses
   `cognigraph.corpus.v1`. Graph and oracle each contain exactly one
   non-executable `application/json` entry named `graph.json` or `oracle.json`,
   with formats `cognigraph.evaluation-graph.v1` and
   `cognigraph.promotion-oracle.v1`. Scorer and verifier each contain exactly
   one executable `application/octet-stream` entry named `cognigraph-server`
   with format `cognigraph.server-executable.v1`.
4. Submit `construct.evaluate` with context schema version 4, the complete M19
   governance binding and M20 artifact set, the server's exact supported
   consumption plan, and `reproducibility.backend = "artifact-snapshot"`.
   Job schema version 2 is selected automatically. Admission fails when the
   local CAS is disabled or the authority/plan is invalid.
5. The worker checked-sums at most 10,000 entries and admits at most 4,096
   unique `(digest, declared length)` pairs across the five manifests, then
   streams and rehashes their blobs under one evaluation budget. An
   already verified `(digest, declared length)` is reused when its cached bytes
   satisfy the next retention requirement; a later graph/oracle retention need
   may require another read after an earlier hash-only occurrence. Corpus bytes
   are verified as provenance but are not parsed or used to reconstruct the
   graph. The parsed verified `graph.json` supplies the actual distinct fact set
   and the parsed verified `oracle.json` supplies the actual EvalSpec. The
   oracle EvalSpec must exactly equal the resolved serialized value (whether
   supplied inline or loaded from tenant storage) and use
   non-empty NFC, control-free strings, closing canonical-digest-equivalent
   Unicode substitutions. The live tenant graph is not read for an M21 score.
6. Scorer and verifier blobs must match the SHA-256 digest of the normal
   non-symlink `cognigraph-server` file reached through `current_exe()` and
   pinned once when the server starts in local-CAS mode, plus the context's
   executable digest. This does not prove the process's mapped code: an atomic
   path replacement between image loading and startup hashing can diverge, and
   replacement after hashing does not update the pin. The staged blobs are
   never loaded, spawned, sandboxed, or executed, and no separate verifier
   program or verdict is produced. The existing oracle-separation gate
   continues to validate the frozen verifier metadata attestation.
7. A successful job result includes an `artifact_consumption` receipt finalized
   after distinct-fact scoring and the final active-authority check. It binds
   tenant/incarnation, job id, attempt, recovery count, immutable input and
   execution payload, exact EvalSpec, five consumed slots, and the canonical
   evaluation result without the receipt. Evidence schema version 4 binds all
   four receipt digests and candidate/baseline material digests; the promoter's
   `cognigraph.promotion-intent.v2` signature binds the resulting consumption
   authority digest.

The local reader checks and rechecks normal non-symlink parents and opened-file
identity and rejects observed missing, wrong-length, wrong-digest,
cross-tenant, symlink, non-file, replacement, retention-cap, and byte-budget
failures. These are path-based checks, not an `openat2`/dirfd-anchored defense
against a hostile concurrent filesystem custodian; keep the root under operator
control and mount it read-only to CogniGraph. Graph JSON is retained only up to
32 MiB, oracle JSON only up to 4 MiB, and the graph is capped at 100,000 sorted
unique evidence-bearing fact rows. Parsed graph memory can be a multiple of its
JSON size, and the cumulative byte budget does not limit memory, CPU, or time;
the separate caps are 10,000 aggregate entries and 4,096 unique digest+length
pairs. Scorer/verifier files and corpus
entries are streamed without retention unless the same digest+length pair must
later be retained as a graph/oracle entrypoint. The active five-attestation set
is checked both before and after consumption, including scoring, so a revocation
that lands during the operation fails that evaluation.

The worker races asynchronous consumption, parsing, scoring, final authority
validation, and receipt construction against a fixed 300-second deadline and
polls durable cancellation, tenant-suspension, and shutdown state every second.
A cancellation finishes the job as canceled, deadline expiry fails the attempt,
and suspension or shutdown parks it for recovery. These are cooperative
process-local guards, not per-file kill isolation, and cannot preempt a kernel
filesystem call that itself does not return.

The receipt is part of the durable terminal job result and is validated by
evidence, decision, recovery, reconciliation, and Native snapshot preflight.
It binds the exact durable execution identity, verified input material, and
canonical score result, but no separate verifier-program verdict. Its unkeyed
hash does not independently authenticate server authorship; later signed
promotion authority binds it into the governed chain. It is not an external
signature, remote attestation, trusted timestamp, or custody proof.
The CAS bytes themselves are not included in a Native snapshot and are not
copied into ArangoDB; back up and replicate them separately.

## Detailed procedures

- [M22 prepared-corpus](governance/prepared-corpus.md)
- [M23 raw-documents](governance/raw-documents.md)
- [M24 artifact-custody](governance/artifact-custody.md)
- [M25 repair-authority](governance/repair-authority.md)
- [M26 deployment](governance/deployment.md)
