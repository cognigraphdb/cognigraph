# Decision: Bind directed extraction to complete endpoint tokens and request IDs

- Date: 2026-09-09
- Status: Active
- Scope: CG-39 and CG-40; directed ingestion only
- Policy revision: `directed-policy-v2`

Directed construction now checks both endpoint mentions at complete token
boundaries and gives the completion provider the exact set of permitted chunk
IDs and relation names. The local gates retain final authority. This changes
which proposals can survive, so new fact attribution and newly created directed
spaces carry policy v2. Existing v1 facts and spaces are not migrated; re-ingest
through the normal reconciliation path to evaluate an existing chunk under v2.
A space's `drafted_by` records its creation policy; each fact's `reviewed_by`
records the policy that actually evaluated that occurrence.

## Endpoint contract

Chunk text, endpoint names and quotes retain explicit NFC normalization. The
endpoint matcher retains the existing ASCII case and whitespace-run tolerance,
but rejects a match when either adjacent character belongs to the Unicode word
set. That set includes alphabetic characters, marks, decimal digits, connector
punctuation and join controls, as defined by
[UTS #18 through regex-syntax](https://docs.rs/regex-syntax/0.8.10/regex_syntax/fn.is_word_character.html).
The dependency explicitly enables the required `unicode-perl` feature.

This rejects `Ann` inside `Joanne` and `South Africa` inside `South African`, on
either endpoint. An invalid first substring does not hide a later complete
mention. Apostrophes, hyphens and other non-word punctuation separate tokens;
quoted names, internal punctuation and multi-word names remain usable.

Quoted-evidence lookup remains separate and unchanged. It may locate a quote
starting inside a word; its byte offsets still index the canonical stored chunk
text. Endpoint checks apply within the existing evidence sentence window.

These boundaries do not prove semantic truth, resolve coreference, or establish
that a token sequence is an entire multi-word name. Unsegmented scripts need
separators around a proposed name under this conservative policy. Separately,
the existing construction writer derives ASCII entity keys and rejects names
with no ASCII letters or digits. This change does not alter that identity
contract or promise unrestricted multilingual construction.

## Completion contract

The response schema is built from the accepted taxonomy and canonical chunk
slice immediately before completion. Its `relation` and `chunk_id` fields use
deterministically sorted, deduplicated string enums. Values preserve exact
request bytes, including opaque punctuation, whitespace and distinct Unicode
normalization forms in IDs. This does not normalize identifiers or relation
names. The prompt and Luna-low model configuration remain unchanged.

The schema retains closed objects and all required fields, allowing the OpenAI
provider to keep `strict: true`; Gemini receives the same contract through
`responseJsonSchema`. If a provider ignores enums, exact local taxonomy and
chunk lookup still reject unknown values. No prefix stripping, trimming, fuzzy
matching or guessed ID substitution is introduced.

Malformed response envelopes still fail before fact reconciliation. An explicit
valid `{"facts": []}` still clears the submitted chunks' projection. A parseable
response whose proposals all fail the gates also produces an empty projection,
consistent with the pre-existing directed contract.

## Verification and development measurement

The [original trial](../research/experiments/luna-documents-2026-09-09/README.md)
and minimal failure reproduction were committed as `a0e4b24` before fixes.
The [candidate package](../research/experiments/luna-directed-v2-2026-09-09/README.md)
freezes new source/binary hashes and evaluates only the same 40 development
documents. Fixed-output replay isolates endpoint-gate effects; a fresh single
Luna-low pass measures the new request schema. Historical evidence and the
80-document holdout remain unchanged. Full-document truth and negative-case
qualification still require independent human review.

Formatting, `cargo clippy --all-targets -- -D warnings`, `cargo test --all` and
the release server build passed. The suite reported 967 passing tests; eight
Arango integration entries returned early without an exported `ARANGO_PASSWORD`,
so this run does not establish live Arango coverage. The
[release HTTP matrix](../evidence/engineering-historical-checks.md#artifact-97932bee025d16118de5)
passed 34 cases across OpenAI and Gemini with synthetic loopback completions.
It verifies actual provider schemas, rejection of ignored enums, unchanged facts
after malformed envelopes, explicit-empty clearing and stored quote byte spans.

The initial format check required Rustfmt layout changes. Two initial new tests
also incorrectly assumed the existing writer could store pure CJK entity names;
they exposed its documented ASCII key restriction. The final tests check pure
Unicode boundaries independently and exercise supported accented names through
storage. No writer identity behavior was changed to make these tests pass.

Fixed-output replay retained all 16 published-reference matches and removed the
single previously stored `South Africa` occurrence derived from `South African`.
The [fresh development pass](../research/experiments/luna-directed-v2-2026-09-09/results.md)
completed 40 real Luna-low calls without retries or provider failures. It had
zero invalid chunk citations, stored 35 occurrences and matched 20 published
references, versus 32/16 in the original capture. Estimated token cost was
$0.0194147. All 40 replay and 40 fresh request schemas/prompts were verified,
as were 31 replay and 35 fresh evidence spans. Five integrity probes passed on
copies. This single development pass is not independent truth qualification;
the next step remains reference/evidence-policy review with the holdout unrun.
The [validation record](../evidence/engineering-historical-checks.md#artifact-417ef1d20b7bd9a5a302)
summarizes command results, preserved failures, artifact hashes and worktree
preservation. The fixes and new candidate remain local and uncommitted after the
`a0e4b24` evidence checkpoint; no remote push was made.
