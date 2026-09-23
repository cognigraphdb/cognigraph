use super::*;
use std::collections::BTreeSet;

fn quoted(text: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let mut rest = text;
    while let Some(start) = rest.find('"') {
        let tail = &rest[start + 1..];
        let Some(end) = tail.find('"') else { break };
        found.insert(tail[..end].to_string());
        rest = &tail[end + 1..];
    }
    found
}

#[test]
fn reserved_words_match_the_cgql_validator() {
    let source = include_str!("../../../cognigraph-query/src/validation.rs");
    let body = source
        .split("fn is_reserved_word")
        .nth(1)
        .and_then(|rest| rest.split("\n}").next())
        .expect("is_reserved_word body");
    let expected: BTreeSet<String> = quoted(body)
        .iter()
        .map(|w| w.to_ascii_lowercase())
        .collect();
    let ours: BTreeSet<String> = RESERVED_WORDS.iter().map(|w| w.to_string()).collect();
    assert_eq!(ours, expected);
}

#[test]
fn server_owned_names_match_the_managed_and_generated_lists() {
    let source = include_str!("../../../cognigraph-server/src/system_collections.rs");
    let mut expected = BTreeSet::new();
    for list in ["const MANAGED_COLLECTIONS", "const GENERATED_COLLECTIONS"] {
        let block = source
            .split(list)
            .nth(1)
            .unwrap()
            .split("];")
            .next()
            .unwrap();
        expected.extend(quoted(block));
        for constant in ["SIDE_VIEWS_COLLECTION", "REFUSALS_COLLECTION"] {
            if block.contains(constant) {
                let definition = source
                    .split(&format!("const {constant}: &str = "))
                    .nth(1)
                    .unwrap();
                expected.extend(quoted(definition.split(';').next().unwrap()));
            }
        }
    }
    let ours: BTreeSet<String> = SERVER_OWNED.iter().map(|w| w.to_string()).collect();
    assert_eq!(ours, expected);
}

#[test]
fn identifiers_reserved_words_and_owned_names() {
    for (name, code) in [
        ("order-lines", "incompatible_collection_name"),
        ("9lives", "incompatible_collection_name"),
        ("naïve", "incompatible_collection_name"),
        ("_graphs", "incompatible_collection_name"),
        ("", "incompatible_collection_name"),
        ("COUNT", "reserved_collection_name"),
        ("Return", "reserved_collection_name"),
        ("facts", "reserved_collection_name"),
    ] {
        assert_eq!(check(name).map(|p| p.code), Some(code), "{name}");
    }
    // Owned names are case-sensitive, like the server's lists.
    for name in ["Customers", "orders_2026", "a", "Facts"] {
        assert!(check(name).is_none(), "{name}");
    }
}
