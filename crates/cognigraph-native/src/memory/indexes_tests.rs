//! Value encoding for unique indexes: the key an index stores for a
//! document must be canonical, sparse-aware and path-aware.

use super::{index_name, value_key};
use cognigraph_core::{IndexDef, IndexType};
use serde_json::json;

fn def(fields: &[&str], sparse: bool, name: Option<&str>) -> IndexDef {
    IndexDef {
        index_type: IndexType::Persistent,
        fields: fields.iter().map(|f| f.to_string()).collect(),
        unique: true,
        sparse,
        name: name.map(str::to_string),
    }
}

#[test]
fn names_derive_from_fields_unless_given() {
    assert_eq!(index_name(&def(&["email"], false, None)), "email_unique");
    assert_eq!(index_name(&def(&["a", "b.c"], false, None)), "a_b.c_unique");
    assert_eq!(index_name(&def(&["email"], false, Some("mail"))), "mail");
}

#[test]
fn missing_fields_index_as_null_unless_sparse() {
    let d = def(&["email"], false, None);
    assert_eq!(
        value_key(&d, &json!({})),
        Some(value_key(&d, &json!({"email": null})).unwrap())
    );
    assert_eq!(value_key(&def(&["email"], true, None), &json!({})), None);
    assert_eq!(
        value_key(&def(&["email"], true, None), &json!({"email": null})),
        None
    );
    // A compound sparse index is exempt only when every field is absent or null.
    let pair = def(&["a", "b"], true, None);
    assert_eq!(value_key(&pair, &json!({"a": 1})), None);
    assert!(value_key(&pair, &json!({"a": 1, "b": 2})).is_some());
}

#[test]
fn keys_are_canonical_and_type_aware() {
    let d = def(&["v"], false, None);
    let k = |v: serde_json::Value| value_key(&d, &json!({"v": v})).unwrap();
    assert_eq!(k(json!(1)), k(json!(1)));
    assert_ne!(k(json!(1)), k(json!("1")), "number and string differ");
    assert_ne!(k(json!(1)), k(json!(1.0)), "integer and float differ");
    assert_eq!(
        k(json!({"b": 1, "a": 2})),
        k(json!({"a": 2, "b": 1})),
        "object key order"
    );
    assert_ne!(k(json!([1, 2])), k(json!([2, 1])), "array order matters");
    assert_ne!(k(json!("x")), k(json!("X")), "strings are exact");
    assert_ne!(
        k(json!("a\u{0}b")),
        k(json!("a")),
        "NUL inside a value is just a byte"
    );
}

#[test]
fn nested_paths_resolve_and_compound_keys_do_not_collide() {
    let nested = def(&["oauth.provider", "oauth.id"], true, None);
    let doc = json!({"oauth": {"provider": "gh", "id": 1}});
    assert!(value_key(&nested, &doc).is_some());
    let flat = def(&["a", "b"], false, None);
    assert_ne!(
        value_key(&flat, &json!({"a": "x,y", "b": "z"})),
        value_key(&flat, &json!({"a": "x", "b": "y,z"})),
        "field boundaries are encoded, not joined by a separator"
    );
    // Dotted path through a non-object is absent.
    assert_eq!(
        value_key(&def(&["a.b"], true, None), &json!({"a": 5})),
        None
    );
}
