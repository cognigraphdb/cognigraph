//! Collection-name rules: a name must be a CGQL identifier and must not be a
//! CGQL reserved word or a collection the server owns. The lists mirror
//! `cognigraph-query/src/validation.rs` and
//! `cognigraph-server/src/system_collections.rs`; `names_tests.rs` parses both
//! sources and fails when they drift.

use super::report::Problem;

pub(crate) const RESERVED_WORDS: [&str; 28] = [
    "aggregate",
    "analyze",
    "and",
    "any",
    "asc",
    "collate",
    "collect",
    "count",
    "desc",
    "distinct",
    "explain",
    "false",
    "filter",
    "for",
    "in",
    "inbound",
    "into",
    "let",
    "limit",
    "not",
    "null",
    "or",
    "outbound",
    "return",
    "sort",
    "true",
    "vector_search",
    "with",
];

pub(crate) const SERVER_OWNED: [&str; 11] = [
    "chunks",
    "construction_refusals",
    "entities",
    "eval_specs",
    "fact_semantics",
    "facts",
    "mentions",
    "neurons",
    "review_policies",
    "side_views",
    "space_types",
];

fn identifier(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// `None` when the name can be imported unchanged.
pub(crate) fn check(name: &str) -> Option<Problem> {
    if !identifier(name) {
        return Some(Problem::new("incompatible_collection_name").collection(name));
    }
    let lower = name.to_ascii_lowercase();
    if RESERVED_WORDS.contains(&lower.as_str()) || SERVER_OWNED.contains(&name) {
        return Some(Problem::new("reserved_collection_name").collection(name));
    }
    None
}

#[cfg(test)]
#[path = "names_tests.rs"]
mod tests;
