# Sentence splitting no longer breaks on abbreviation periods

- Date: 2026-07-21
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:669-684` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **Sentence splitting no longer breaks on abbreviation periods.**
  `is_sentence_boundary` ended a sentence at any period followed by whitespace, so
  `St. John's Wort` and `D C Yellow No. 10 Aluminum Lake` were cut in half and
  their (correct) facts looked target-absent to the relation-semantics detector.
  A dot closing a known abbreviation is now non-terminal. **The list is
  deliberately narrow because the two mistakes are not symmetric:** failing to
  split merges sentences, and a `require_in_sentence` gate then judges a longer
  span and admits more groundings — a restraint regression — while splitting too
  eagerly merely truncates. So it excludes `etc.` and `Inc.`/`Ltd.`/`Corp.`,
  which routinely end an FDA label's manufacturer line, and every entry traces to
  a measured false flag. Measured: clean lane **91.8% → 92.2%**, separation
  **+15.2% → +17.2%**, errors caught unchanged at 70% (no true error escaped),
  review volume down 756 → 742. Both safety checks held — the full suite
  (gate, restraint, reference kits) stayed green and the pilot's grounded-fact
  count was identical at 1575, confirming no grounding moved.
