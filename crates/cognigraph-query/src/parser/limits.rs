//! Query-text limits applied before the recursive-descent parser runs.
//!
//! The grammar recurses for brackets, prefix operators and the ternary
//! operator, and the AST builder, validator, planner, executor and `Drop` all
//! recurse over the expression tree. Unbounded nesting therefore overflowed
//! the stack and aborted the process (CG-105). This scan is linear, allocates
//! one small counter per open bracket, and never recurses.

use super::ParseError;

/// Longest accepted query text, in bytes.
pub const MAX_QUERY_BYTES: usize = 256 * 1024;

/// Deepest accepted structural nesting: open brackets, plus ternary operators
/// and prefix operators that are still pending at each open bracket. This is
/// the recursion depth of the grammar.
///
/// Sized from measurement, not taste. On a 2 MiB thread (the server's worker
/// stack) a release build parses and executes about 156 nested arrays and 183
/// nested parentheses before the stack runs out; a debug build manages 44.
/// 32 leaves roughly a fourfold margin in release and still fits in debug.
pub const MAX_EXPRESSION_NESTING: usize = 32;

/// Largest accepted expression-tree depth: structural nesting plus the binary
/// operators chained at each open bracket, which the builder folds into a
/// left-nested tree. This bounds every recursive pass over the tree.
///
/// A chained operator costs far less stack than a bracket (a release build
/// survives a chain of about 6,000 on 2 MiB), so this limit is wider. The
/// worst accepted query, full nesting with a full chain inside, needs about
/// 0.5 MiB in a release build. A debug build needs more than 2 MiB for it.
pub const MAX_EXPRESSION_DEPTH: usize = 256;

/// Clause keywords end one expression and start another, so chains counted at
/// the top level of a query do not carry across them.
const CLAUSE_KEYWORDS: [&str; 16] = [
    "FOR",
    "LET",
    "FILTER",
    "COLLECT",
    "AGGREGATE",
    "SORT",
    "LIMIT",
    "RETURN",
    "INSERT",
    "UPDATE",
    "REPLACE",
    "REMOVE",
    "UPSERT",
    "INTO",
    "WITH",
    "COLLATE",
];

#[derive(Default)]
struct Level {
    ternaries: usize,
    binary_operators: usize,
}

/// Reject query text that is too long or nests too deeply to parse safely.
pub(super) fn check_query_text(input: &str) -> Result<(), ParseError> {
    if input.len() > MAX_QUERY_BYTES {
        return Err(ParseError::new(format!(
            "query text is {} bytes; the limit is {MAX_QUERY_BYTES}",
            input.len()
        )));
    }
    let bytes = input.as_bytes();
    let mut levels = vec![Level::default()];
    let mut structural = 0usize; // ternaries pending at every open level
    let mut chained = 0usize; // binary operators pending at every open level
    let mut unary_run = 0usize;
    let mut after_dot = false;
    let mut index = 0usize;
    while index < bytes.len() {
        let byte = bytes[index];
        let mut is_unary = false;
        let mut next_after_dot = false;
        match byte {
            b' ' | b'\t' | b'\r' | b'\n' => {
                index += 1;
                continue;
            }
            b'/' if bytes.get(index + 1) == Some(&b'/') => {
                while index < bytes.len() && bytes[index] != b'\n' {
                    index += 1;
                }
                continue;
            }
            b'"' => {
                index += 1;
                while index < bytes.len() && bytes[index] != b'"' {
                    index += if bytes[index] == b'\\' { 2 } else { 1 };
                }
                index += 1;
            }
            b'(' | b'[' | b'{' => {
                levels.push(Level::default());
                index += 1;
            }
            b')' | b']' | b'}' => {
                if levels.len() > 1 {
                    let closed = levels.pop().expect("an open level");
                    structural -= closed.ternaries;
                    chained -= closed.binary_operators;
                }
                index += 1;
            }
            b',' => {
                reset(&mut levels, &mut structural, &mut chained);
                index += 1;
            }
            b'?' => {
                levels.last_mut().expect("the top level").ternaries += 1;
                structural += 1;
                index += 1;
            }
            b'-' => {
                // Either a prefix or a binary minus; count it as both, which
                // can only overestimate.
                is_unary = true;
                binary_operator(&mut levels, &mut chained);
                index += 1;
            }
            b'+' | b'*' | b'/' => {
                binary_operator(&mut levels, &mut chained);
                index += 1;
            }
            b'=' | b'!' | b'<' | b'>' => {
                binary_operator(&mut levels, &mut chained);
                index += if bytes.get(index + 1) == Some(&b'=') {
                    2
                } else {
                    1
                };
            }
            b'.' => {
                next_after_dot = true;
                index += 1;
            }
            byte if byte.is_ascii_alphabetic() || byte == b'_' => {
                let start = index;
                while index < bytes.len()
                    && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_')
                {
                    index += 1;
                }
                let word = &input[start..index];
                if after_dot {
                    // A field name such as `d.not` is never a keyword.
                } else if word.eq_ignore_ascii_case("NOT") {
                    is_unary = true;
                } else if ["AND", "OR", "IN"]
                    .iter()
                    .any(|keyword| word.eq_ignore_ascii_case(keyword))
                {
                    binary_operator(&mut levels, &mut chained);
                } else if levels.len() == 1
                    && CLAUSE_KEYWORDS
                        .iter()
                        .any(|keyword| word.eq_ignore_ascii_case(keyword))
                {
                    reset(&mut levels, &mut structural, &mut chained);
                }
            }
            _ => index += 1,
        }
        after_dot = next_after_dot;
        unary_run = if is_unary { unary_run + 1 } else { 0 };
        let nesting = levels.len() - 1 + structural + unary_run;
        if nesting > MAX_EXPRESSION_NESTING {
            return Err(ParseError::new(format!(
                "expression nesting exceeds the limit of {MAX_EXPRESSION_NESTING}"
            )));
        }
        if nesting + chained > MAX_EXPRESSION_DEPTH {
            return Err(ParseError::new(format!(
                "expression depth exceeds the limit of {MAX_EXPRESSION_DEPTH}; \
                 pass long value lists as a bind variable and test them with IN"
            )));
        }
    }
    Ok(())
}

fn binary_operator(levels: &mut [Level], chained: &mut usize) {
    levels.last_mut().expect("the top level").binary_operators += 1;
    *chained += 1;
}

/// A comma or a clause keyword ends the chains counted at the current level.
fn reset(levels: &mut [Level], structural: &mut usize, chained: &mut usize) {
    let level = levels.last_mut().expect("the top level");
    *structural -= level.ternaries;
    *chained -= level.binary_operators;
    *level = Level::default();
}
