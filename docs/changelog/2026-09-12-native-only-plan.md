# Approve the Native-only storage engineering batch

- Date: 2026-09-12
- Status: Unreleased
- Kind: Architecture and planning

## Change

The owner approved [Native-only storage](../decisions/decision_native_only.md),
replacing the indefinite ArangoDB maintenance clause in Native-first. The owner
clarified that CogniGraph has never been provided to anyone and live deployment
will start only after Native-only readiness. Breaking cleanup may remove legacy
configuration and active support narratives without a compatibility shim,
customer migration or mandatory major-version jump solely for Arango removal.

The revised [active sequence](../plans/native-only-2026-09-12.md) is CG-65 → CG-67
→ CG-68: preserve useful Native coverage, remove Arango, and verify fresh Native
readiness. CG-64/CG-66 remain Open with explicit deferred optional scheduling;
the [dump-import design](../plans/arangodump-import-design.md) moves out of the
runtime reference section. The registry has 62 Resolved, 1 Closed without change
and 5 Open P2 tickets: 3 active and 2 deferred. Original issue identities remain.

Update engineering navigation, current operational/migration context, decision
supersession and the root backend instruction. Current 2.7.1 still includes
Arango; the runtime cleanup and optional importer are not implemented by this
documentation change. Product/website repositories and A9 teaching material need
separate alignment; no edits to them are claimed here.

## Validation

Local documentation, decision-index and issue-identity checks passed for this
revision: 408 documents, 2,177 local links, 57 decision records and 68 issues.
The changelog index was regenerated and whitespace validation passed. These
checks do not qualify the proposed runtime behavior.
No new Rust implementation, live import, Arango fixture capture, release
qualification, version bump, commit or publication is performed by this record.
