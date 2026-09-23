//! Sentence boundary detection with non-terminal abbreviations.

/// Abbreviations whose trailing dot is NOT a sentence end.
///
/// Deliberately short and conservative, because the two mistakes are not
/// symmetric: failing to split costs restraint (a `require_in_sentence` gate
/// then judges a longer span and admits more groundings), while splitting too
/// eagerly only truncates a sentence. So this lists only tokens that are
/// essentially never sentence-final in label prose, and pointedly EXCLUDES ones
/// that often are — `etc.`, and the corporate suffixes `Inc.`/`Ltd.`/`Corp.`,
/// which routinely end a sentence in an FDA label's manufacturer line.
///
/// Every entry is drawn from a measured false flag on the 100-label pilot, not
/// from a general-purpose abbreviation list (decision_pilot_clinical_graph.md).
pub(super) const NON_TERMINAL_ABBREVIATIONS: [&str; 12] = [
    "st", // "St. John's Wort" — the case that motivated this
    "no", // "D C Yellow No. 10 Aluminum Lake"
    "dr", "mr", "mrs", "ms", "prof", // titles
    "vs", "cf", "al", // "et al."
    "fig", "approx",
];

/// The alphabetic token immediately preceding a dot, if any.
pub(super) fn word_before_dot(text: &str, dot: usize) -> &str {
    let head = &text[..dot];
    let start = head
        .char_indices()
        .rev()
        .find(|(_, c)| !c.is_alphabetic())
        .map_or(0, |(i, c)| i + c.len_utf8());
    &head[start..]
}

/// Sentence bounds (byte range in the original text) around an offset.
/// Sentence boundaries are deliberately COARSER than the negation clause
/// boundaries (no ','/':') — the gate asks "is this sentence about the
/// endpoint", not "is this clause negated". A '.' ends a sentence only
/// when followed by whitespace or end-of-text: a dot flanked by other
/// characters is part of a token — a decimal ("$3.09B", found by the
/// gate advisor) or a dotted name ("Chorus.ai", "OpenProtein.AI",
/// found by the chunk-sensitivity simulation: mid-name splits were the
/// ENTIRE measured cost of adversarial re-chunking, and they truncate
/// the sentence a gate judges).
///
/// A dot closing a known abbreviation is likewise not a boundary. Real FDA
/// labels name entities that CONTAIN one — `St. John's Wort`, `D C Yellow No. 10
/// Aluminum Lake` — and splitting there truncates the sentence mid-entity-name,
/// which made correct facts look target-absent to the relation-semantics
/// detector (6 false flags per 200 facts, all of them correct facts).
pub(super) fn is_sentence_boundary(text: &str, i: usize, c: char) -> bool {
    const BOUNDS: &[char] = &['.', '!', '?', '\n'];
    if !BOUNDS.contains(&c) {
        return false;
    }
    if c == '.' {
        let next = text[i + c.len_utf8()..].chars().next();
        if next.is_some_and(|n| !n.is_whitespace()) {
            return false;
        }
        let word = word_before_dot(text, i);
        if !word.is_empty()
            && NON_TERMINAL_ABBREVIATIONS
                .iter()
                .any(|abbrev| word.eq_ignore_ascii_case(abbrev))
        {
            return false;
        }
    }
    true
}

/// Sentence ranges in casefolded coordinates, computed once per chunk. Sentence
/// punctuation is stable under lowercasing, so these ranges match
/// `sentence_bounds` while avoiding a full before/after scan per occurrence.
pub(super) fn sentence_ranges_cf(text_cf: &str) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let mut start = 0;
    for (i, c) in text_cf.char_indices() {
        if is_sentence_boundary(text_cf, i, c) {
            let end = i + c.len_utf8();
            ranges.push((start, end));
            start = end;
        }
    }
    if start < text_cf.len() {
        ranges.push((start, text_cf.len()));
    }
    ranges
}
