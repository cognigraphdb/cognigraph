//! Negation cues, clause boundaries and case-folded offset mapping for affirmation checks.

pub(super) const NEGATION_CUES: &[&str] = &[
    "not",
    "no",
    "never",
    "none",
    "nor",
    "false",
    "untrue",
    "without",
    "denied",
    "denies",
    "deny",
    "retracted",
    "withdrawn",
    "cannot",
];

pub(super) const CLAUSE_BOUNDARIES: &[char] = &['.', ';', '!', '?', ',', ':'];

/// Casefolded fast path: `ground_chunk` folds the chunk text once and
/// reuses it across every rule trigger and veto phrase (measured 2.5x on
/// the grounding benchmark — see docs/benchmarks.md).
pub(super) fn affirms_phrase_cf(text_cf: &str, phrase_cf: &str) -> bool {
    affirming_offset_cf(text_cf, phrase_cf).is_some()
}

/// Byte offset (in the CASEFOLDED text) of the first occurrence of
/// `phrase_cf` that is not negated within its clause — the occurrence
/// that licenses grounding. `map_cf_offset` translates it back to the
/// original text for provenance.
pub(super) fn affirming_offset_cf(text_cf: &str, phrase_cf: &str) -> Option<usize> {
    affirming_offset_where(text_cf, phrase_cf, |_| true)
}

/// Like `affirming_offset_cf`, but the occurrence must also satisfy
/// `accept` (the sentence-scoped endpoint gate) — a later occurrence in a
/// satisfying sentence still grounds when an earlier one fails the gate,
/// mirroring how negation lookback is per-occurrence.
pub(super) fn affirming_offset_where(
    text_cf: &str,
    phrase_cf: &str,
    accept: impl Fn(usize) -> bool,
) -> Option<usize> {
    let negated_at = clause_negation_index(text_cf);
    affirming_offset_where_indexed(text_cf, phrase_cf, &negated_at, accept)
}

pub(super) fn affirming_offset_where_indexed(
    text_cf: &str,
    phrase_cf: &str,
    negated_at: &[bool],
    accept: impl Fn(usize) -> bool,
) -> Option<usize> {
    if phrase_cf.is_empty() {
        return None;
    }
    let mut start = 0;
    while let Some(offset) = text_cf[start..].find(phrase_cf) {
        let idx = start + offset;
        if !negated_at.get(idx).copied().unwrap_or(true) && accept(idx) {
            return Some(idx);
        }
        // Advance one character (respecting UTF-8 boundaries).
        start = idx + text_cf[idx..].chars().next().map_or(1, char::len_utf8);
        if start >= text_cf.len() {
            break;
        }
    }
    None
}

/// Map a byte offset in `text.to_lowercase()` back to a byte offset in
/// `text`. Casefolding can change byte lengths (ß → ss, İ → i̇), so the
/// two coordinate systems diverge on non-ASCII text; this walks both in
/// lockstep. An offset falling INSIDE one character's folded expansion
/// resolves to that character's start. Offsets past the end clamp to
/// `text.len()`.
pub(super) fn map_cf_offset(text: &str, cf_offset: usize) -> usize {
    let mut folded = 0usize;
    for (original, c) in text.char_indices() {
        if folded >= cf_offset {
            return original;
        }
        folded += c.to_lowercase().map(char::len_utf8).sum::<usize>();
        if folded > cf_offset {
            return original; // inside this char's folded expansion
        }
    }
    text.len()
}

pub(super) fn token_is_negation(token: &str) -> bool {
    !token.is_empty() && (NEGATION_CUES.contains(&token) || token.ends_with("n't"))
}

/// Whether each UTF-8 byte boundary has a negation cue earlier in its current
/// clause. This exactly preserves the prefix semantics of `negated_before`
/// while replacing repeated reverse clause scans with one linear chunk pass.
pub(super) fn clause_negation_index(text_cf: &str) -> Vec<bool> {
    let mut negated_at = vec![false; text_cf.len() + 1];
    let mut completed_negation = false;
    let mut token_start = None;
    for (index, character) in text_cf.char_indices() {
        let partial_negation = token_start
            .map(|start| token_is_negation(&text_cf[start..index]))
            .unwrap_or(false);
        negated_at[index] = completed_negation || partial_negation;

        if CLAUSE_BOUNDARIES.contains(&character) {
            completed_negation = false;
            token_start = None;
        } else if character.is_ascii_alphanumeric() || character == '\'' {
            token_start.get_or_insert(index);
        } else if let Some(start) = token_start.take()
            && token_is_negation(&text_cf[start..index])
        {
            completed_negation = true;
        }
    }
    let partial_negation = token_start
        .map(|start| token_is_negation(&text_cf[start..]))
        .unwrap_or(false);
    negated_at[text_cf.len()] = completed_negation || partial_negation;
    negated_at
}
