# Public engineering record and private evidence — 2026-09-13

The owner selected a separate private evidence repository and clarified that
the shared Railway database is an on-demand QA environment. Applications have
their own CogniGraph instances. Follow the [hosted QA decision](../decisions/decision_hosted_qa_lifecycle.md).
Local migration and qualification are complete through CG-75–CG-77. The
[acceptance summary](../verification/public-boundary-2026-09-13.md) records the
checks and private archive identities. Code publication and historical cleanup
remain separate operations; current public history has not been rewritten.

## Distribution contract

Keep the Rust crates, public interfaces, architecture, decisions, changelogs,
generic operational guides, research conclusions and plans public. Enterprise
source and its existing license boundary remain in place. Retain negative
results, qualification limits and protocol descriptions without overstating
what a public reader can reproduce.

Store raw provider exchanges, labelled research runs, large audit captures,
real deployment inventories and internal remediation records privately.
Preserve original bytes and attribution. Public references should identify an
artifact by a stable package/path, SHA-256 and byte length, together with its
access classification. Such a digest establishes byte identity, not scientific
validity, timestamp proof or a cryptographic attestation.

Keep small public test fixtures and executable harnesses necessary for builds
and CI. Current evidence-directory placement is not itself a reason to make
an executable dependency private. Moving logs does not make prompts confidential
when the implementation still contains those prompts.

## Sequenced work

1. [CG-76](../issues/CG-76.md): stop shared hosted QA between test sessions,
   preserve its volume, remove its automatic deployment trigger and the unfinished
   permanent ingress setup. Keep actual operational inventories private and
   document deliberate startup, authenticated synthetic checks and shutdown.
2. [CG-75](../issues/CG-75.md): archive originals with verified hashes, extract
   public test dependencies, move captures and repair navigation through an
   evidence catalog. Preserve uncommitted release receipts as well as previously
   published captures.
3. [CG-77](../issues/CG-77.md): enforce the boundary in writers, agent workflows
   and CI, then qualify the public checkout without access to private evidence.

The v2.7.11 inventory contains 93,672,669 bytes across the three candidate
directories, before distinguishing retained verification inputs. This is
uncompressed file content, not Git transfer size or a promised clone saving.

## Historical publication and release identity

Deleting files from the current tree does not remove their historical blobs.
Any history operation must be prepared separately with a private inventory,
original backups, exact old/new refs, drift checks and explicit publication
scope. Existing public copies cannot be recalled. GitHub also limits support
removal to eligible sensitive data; ordinary business-value material must not
be represented as guaranteed server-side purging.
See [GitHub's documented limits](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/removing-sensitive-data-from-a-repository).

Preserve the original v2.7.11 image digests and their real source commit labels.
A later source-history mapping must not rewrite those historical release claims
or replace immutable registry tags. New outgoing code changes follow the normal
version, incoming-PR and complete local/remote verification workflow.

## Completion evidence

Acceptance requires verified private copies; checked public path/hash references;
passing public build, test, browser, Native and packaged-image checks; a proven
verified on-demand QA lifecycle; and an honest record of what remains
accessible through historical publication. No provider calls or holdout runs are
needed for this distribution work.
