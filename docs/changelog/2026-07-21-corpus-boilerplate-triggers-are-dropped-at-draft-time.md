# Corpus-boilerplate triggers are dropped at draft time

- Date: 2026-07-21
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:790-801` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **Corpus-boilerplate triggers are dropped at draft time.** The
  `MAXALT --CONTACT--> FDA` leak (29 facts licensed off the MedWatch footer, first
  evidenced from an unrelated atropine label) came from a corpus-wide phrase
  becoming a rule *trigger*. Per-document drafting is the first place the whole
  corpus is held, so it now drops any trigger that fires inside a document which
  never names the rule's source (and drops a rule that loses every trigger), both
  visible in `skips`. Grounding semantics and the gate-advisor contract are
  deliberately untouched — ground-then-advise exists so the system never silently
  picks a side (`decision_cross_chunk_grounding.md`). Global entity identity was
  investigated and deliberately left alone: a key collision is meant to be an error
  rather than a silent semantic merge.
