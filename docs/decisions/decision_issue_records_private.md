# Decision: the public issue directory holds the ledger only

Status: accepted and implemented 2026-11-11 for [CG-193](../issues/CG-193.md).

## Context

`docs/issues/` held two kinds of file: the `CG-N.md` tickets with their
registry, and 38 dated records written while working on them (review
reports, remediation batches, verification write-ups). The dated records are
long working narratives. They were useful to the people doing the work and
are not something a reader of a public database repository needs. The owner
asked for the public directory to carry the ledger and nothing else.

## Decision

Owner decision, 2026-11-11.

1. **Ledger only.** The tracked contents of `docs/issues/` are the `CG-N.md`
   tickets and `README.md`, the registry. `python3 scripts/issue.py check`
   fails on anything else.
2. **Dated records are private.** They live in `docs/private/issues/`, which
   Git ignores. Their bytes are unchanged by the move, so relative links
   inside them no longer resolve from the new location.
3. **Existing links stay.** Tickets, changelog records, decisions and archived
   plans link to the dated records about 250 times, and several of those
   documents are sealed history. The links are left as written. The
   documentation checks treat a link to `docs/issues/<name>-YYYY-MM-DD.md` as
   a private record: verified against the private folder when it is present,
   skipped and counted when it is not. Any other missing target still fails.
4. **New work.** Resolution and verification evidence that a ticket needs is
   written in the ticket. A longer dated record goes to the private folder
   or, when it should be preserved, to the private evidence repository under
   the [evidence policy](../operations/evidence-policy.md).
5. **Rejected: rewriting the links as plain text.** It would change about 137
   documents, including sealed ones, to remove names that are already in
   public history.
6. **Rejected: leaving the files in place but untracked.** The directory would
   still mix ledger and working records locally, which is what the owner
   wanted to end.

## Consequences

- In the public repository those links lead nowhere. The registry says so
  once, under its conventions.
- An ignored folder is not versioned and is not an archive. A record that
  must survive belongs in the private evidence repository.
- The 37 records published before this decision remain in the public Git
  history. Removing them from history is a separate, owner-authorized
  procedure in the [push guide](../operations/push.md#owner-authorized-data-removal).
