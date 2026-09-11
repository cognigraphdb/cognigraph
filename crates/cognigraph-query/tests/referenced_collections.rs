//! `Query::referenced_collections` — the static collection census the
//! server's system-collection guard is built on. It must see every source
//! a query can read or write, and must NOT report bound variables as
//! collections (the planner's variable-first rule).

use cognigraph_query::parse_query;

fn collections(query: &str) -> Vec<String> {
    parse_query(query)
        .unwrap()
        .referenced_collections()
        .into_iter()
        .collect()
}

#[test]
fn collection_scan_is_reported() {
    assert_eq!(collections("FOR u IN _users RETURN u"), vec!["_users"]);
}

#[test]
fn vector_search_source_is_reported() {
    assert_eq!(
        collections("FOR d IN VECTOR_SEARCH(chunks, @v) RETURN d"),
        vec!["chunks"]
    );
}

#[test]
fn traversal_edge_collection_is_reported() {
    assert_eq!(
        collections("FOR v, e, p IN 1..2 OUTBOUND @start rels RETURN v"),
        vec!["rels"]
    );
}

#[test]
fn mutation_targets_are_reported() {
    assert_eq!(
        collections("INSERT { name: \"x\" } INTO _tokens"),
        vec!["_tokens"]
    );
    assert_eq!(
        collections("FOR u IN _users UPDATE u._key WITH { role: \"Admin\" } IN _users"),
        vec!["_users"]
    );
    assert_eq!(collections("REMOVE \"k\" IN _tenants"), vec!["_tenants"]);
    assert_eq!(
        collections("UPSERT { name: \"x\" } INSERT { name: \"x\" } UPDATE { seen: true } IN audit"),
        vec!["audit"]
    );
}

#[test]
fn subquery_sources_are_reported() {
    assert_eq!(
        collections("LET secrets = (FOR t IN _tokens RETURN t) RETURN secrets"),
        vec!["_tokens"]
    );
}

#[test]
fn bound_variables_are_not_collections() {
    // The inner FOR iterates the LET binding, not a collection.
    assert_eq!(
        collections("LET rows = [1, 2] FOR r IN rows RETURN r"),
        Vec::<String>::new()
    );
}

#[test]
fn outer_bindings_shadow_in_subqueries() {
    // `rows` is bound in the outer scope; the subquery's FOR sees it.
    assert_eq!(
        collections("LET rows = [1] LET inner = (FOR r IN rows RETURN r) RETURN inner"),
        Vec::<String>::new()
    );
}
