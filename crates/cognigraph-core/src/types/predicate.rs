//! Field predicates for filter pushdown: the subset of CGQL comparison
//! semantics a backend can apply during a scan, so non-matching documents
//! are never cloned. Semantics MUST mirror the CGQL evaluator exactly
//! (numbers compare by value, strings lexicographically, mixed types never
//! order, missing paths read as null) — the dual-engine corpus enforces
//! agreement end to end.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PredicateOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    /// Membership: the field equals (CGQL equality) any element of the
    /// predicate value; a non-array predicate value never matches.
    In,
}

/// One pushed-down comparison: `doc.<path> <op> <value>`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldPredicate {
    /// Attribute path below the document root; missing segments read null.
    pub path: Vec<String>,
    pub op: PredicateOp,
    pub value: Value,
}

impl FieldPredicate {
    pub fn matches(&self, doc: &Value) -> bool {
        let mut field: &Value = doc;
        for part in &self.path {
            field = field.get(part).unwrap_or(&Value::Null);
        }
        match self.op {
            PredicateOp::Eq => values_equal(field, &self.value),
            PredicateOp::Ne => !values_equal(field, &self.value),
            PredicateOp::Lt => compare(field, &self.value, |o| o == std::cmp::Ordering::Less),
            PredicateOp::Le => compare(field, &self.value, |o| o != std::cmp::Ordering::Greater),
            PredicateOp::Gt => compare(field, &self.value, |o| o == std::cmp::Ordering::Greater),
            PredicateOp::Ge => compare(field, &self.value, |o| o != std::cmp::Ordering::Less),
            PredicateOp::In => match &self.value {
                Value::Array(items) => items.iter().any(|item| values_equal(field, item)),
                _ => false,
            },
        }
    }
}

/// CGQL equality: numbers by numeric value (stored int 2 == literal 2.0),
/// arrays/objects element-wise, everything else strict.
pub fn values_equal(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Number(_), Value::Number(_)) => left.as_f64() == right.as_f64(),
        (Value::Array(l), Value::Array(r)) => {
            l.len() == r.len() && l.iter().zip(r).all(|(a, b)| values_equal(a, b))
        }
        (Value::Object(l), Value::Object(r)) => {
            l.len() == r.len()
                && l.iter()
                    .all(|(k, v)| r.get(k).is_some_and(|rv| values_equal(v, rv)))
        }
        _ => left == right,
    }
}

/// CGQL ordering: numbers numerically, strings lexicographically, any other
/// combination (including null on either side) never orders — false.
fn compare(left: &Value, right: &Value, pred: impl FnOnce(std::cmp::Ordering) -> bool) -> bool {
    match (left, right) {
        (Value::Number(_), Value::Number(_)) => match (left.as_f64(), right.as_f64()) {
            (Some(l), Some(r)) => l.partial_cmp(&r).is_some_and(pred),
            _ => false,
        },
        (Value::String(l), Value::String(r)) => pred(l.cmp(r)),
        _ => false,
    }
}

/// Clone only the requested top-level fields (plus `_key`/`_id`) into a
/// fresh object — the projection contract of `list_documents_projected`.
pub fn project_fields(doc: &Value, fields: &[String]) -> Value {
    let mut projected = Map::new();
    if let Value::Object(obj) = doc {
        for key in ["_key", "_id"] {
            if let Some(value) = obj.get(key) {
                projected.insert(key.to_string(), value.clone());
            }
        }
        for field in fields {
            if let Some(value) = obj.get(field) {
                projected.insert(field.clone(), value.clone());
            }
        }
    }
    Value::Object(projected)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn pred(path: &str, op: PredicateOp, value: Value) -> FieldPredicate {
        FieldPredicate {
            path: path.split('.').map(str::to_string).collect(),
            op,
            value,
        }
    }

    #[test]
    fn numeric_equality_by_value() {
        let doc = json!({"n": 2});
        assert!(pred("n", PredicateOp::Eq, json!(2.0)).matches(&doc));
        assert!(!pred("n", PredicateOp::Ne, json!(2.0)).matches(&doc));
    }

    #[test]
    fn missing_path_reads_null() {
        let doc = json!({"a": {"b": 1}});
        assert!(pred("a.missing", PredicateOp::Eq, Value::Null).matches(&doc));
        assert!(pred("nope.deep", PredicateOp::Eq, Value::Null).matches(&doc));
        // Null never orders.
        assert!(!pred("nope", PredicateOp::Lt, json!(5)).matches(&doc));
        assert!(!pred("nope", PredicateOp::Ge, json!(5)).matches(&doc));
    }

    #[test]
    fn ordering_matches_cgql() {
        let doc = json!({"n": 10, "s": "beta"});
        assert!(pred("n", PredicateOp::Gt, json!(9.5)).matches(&doc));
        assert!(pred("n", PredicateOp::Le, json!(10)).matches(&doc));
        assert!(pred("s", PredicateOp::Lt, json!("gamma")).matches(&doc));
        // Mixed types never order.
        assert!(!pred("s", PredicateOp::Gt, json!(1)).matches(&doc));
        assert!(!pred("n", PredicateOp::Lt, json!("z")).matches(&doc));
    }

    #[test]
    fn in_matches_by_value_equality() {
        let doc = json!({"n": 2, "cat": "research"});
        assert!(pred("n", PredicateOp::In, json!([1, 2.0, 3])).matches(&doc));
        assert!(pred("cat", PredicateOp::In, json!(["notes", "research"])).matches(&doc));
        assert!(!pred("cat", PredicateOp::In, json!(["notes"])).matches(&doc));
        // Non-array predicate value never matches (mirrors the evaluator).
        assert!(!pred("n", PredicateOp::In, json!(2)).matches(&doc));
        // Missing field reads null; null IN [null] matches.
        assert!(pred("missing", PredicateOp::In, json!([null])).matches(&doc));
    }

    #[test]
    fn deep_equality_for_composites() {
        let doc = json!({"tags": ["a", "b"], "meta": {"x": 1}});
        assert!(pred("tags", PredicateOp::Eq, json!(["a", "b"])).matches(&doc));
        assert!(pred("meta", PredicateOp::Eq, json!({"x": 1.0})).matches(&doc));
        assert!(pred("tags", PredicateOp::Ne, json!(["a"])).matches(&doc));
    }

    #[test]
    fn projection_keeps_keys_and_requested_fields() {
        let doc = json!({"_key": "k1", "_id": "c/k1", "a": 1, "b": 2});
        let projected = project_fields(&doc, &["a".to_string()]);
        assert_eq!(projected, json!({"_key": "k1", "_id": "c/k1", "a": 1}));
    }
}
