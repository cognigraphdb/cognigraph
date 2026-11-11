//! CG-111: the memory meter's arithmetic.

use serde_json::json;

use super::*;

#[test]
fn weights_follow_the_documented_formula() {
    assert_eq!(weight(&json!(null)), 32);
    assert_eq!(weight(&json!(true)), 32);
    assert_eq!(weight(&json!(1.5)), 32);
    // A string: 32, one allocation, its bytes.
    assert_eq!(weight(&json!("")), 48);
    assert_eq!(weight(&json!("abcd")), 52);
    // An array: 32, one allocation, its elements. Empty containers allocate
    // nothing.
    assert_eq!(weight(&json!([])), 32);
    assert_eq!(weight(&json!([1, "ab"])), 48 + 32 + 50);
    assert_eq!(weight(&json!([[[]]])), 48 + 48 + 32);
    // An object: 32, one node, and per field the key and what the value
    // holds beyond the 32 bytes the node already has for it.
    assert_eq!(weight(&json!({})), 32);
    assert_eq!(weight(&json!({"k": 1})), 32 + 640 + (16 + 1));
    assert_eq!(
        weight(&json!({"k": 1, "name": "x"})),
        32 + 640 + (16 + 1) + (16 + 4 + 17)
    );
    // A node is counted per six fields.
    let fields = |count: usize| {
        serde_json::Value::Object((0..count).map(|n| (format!("f{n}"), json!(n))).collect())
    };
    assert_eq!(weight(&fields(6)), 32 + 640 + 6 * 18);
    assert_eq!(weight(&fields(7)), 32 + 2 * 640 + 7 * 18);
    assert_eq!(weight(&fields(13)), 32 + 3 * 640 + 10 * 18 + 3 * 19);

    let mut env = Env::new();
    assert_eq!(env_weight(&env), 0);
    env.insert("n".into(), json!(7));
    assert_eq!(env_weight(&env), 160 + 16 + 1);
    env.insert("doc".into(), json!({"k": 1}));
    assert_eq!(env_weight(&env), 177 + 160 + 16 + 3 + 640 + 17);
}

#[test]
fn an_unlimited_meter_counts_nothing_and_never_fails() {
    let meter = MemoryMeter::new(None);
    assert!(!meter.enabled());
    meter.begin();
    meter.read(u64::MAX).unwrap();
    meter.retain(u64::MAX).unwrap();
    assert_eq!(meter.retain_value(&json!("text")).unwrap(), 0);
    assert_eq!(meter.remaining(), u64::MAX);
    assert_eq!(meter.mark(), 0);
}

#[test]
fn the_limit_is_inclusive_and_covers_retained_plus_transient() {
    let meter = MemoryMeter::new(Some(100));
    meter.retain(60).unwrap();
    meter.begin();
    meter.read(40).unwrap();
    assert_eq!(meter.remaining(), 0);
    assert_eq!(
        meter.read(1).unwrap_err(),
        ExecutionError::MemoryBudgetExceeded(100)
    );
    // A new expression starts its own count; what is retained stays.
    meter.begin();
    assert_eq!(meter.remaining(), 40);
    meter.read(40).unwrap();
    assert_eq!(
        meter.retain(41).unwrap_err(),
        ExecutionError::MemoryBudgetExceeded(100)
    );
}

#[test]
fn scalars_are_free_to_read_and_strings_are_not() {
    let meter = MemoryMeter::new(Some(60));
    meter.begin();
    for _ in 0..1_000 {
        meter.read_value(&json!(7)).unwrap();
        meter.read_value(&json!(null)).unwrap();
    }
    meter.read_value(&json!("12345678")).unwrap();
    assert!(meter.read_value(&json!("x")).is_err());
}

#[test]
fn release_and_marks_never_underflow_or_wrap() {
    let meter = MemoryMeter::new(Some(1_000));
    meter.retain(300).unwrap();
    let mark = meter.mark();
    meter.retain(500).unwrap();
    meter.release_to(mark);
    assert_eq!(meter.mark(), 300);
    meter.release(1_000_000);
    assert_eq!(meter.mark(), 0);
    // Saturating arithmetic: a huge charge fails, it does not wrap to a pass.
    assert!(meter.retain(u64::MAX).is_err());
    assert!(meter.retain(u64::MAX).is_err());
    assert_eq!(weight(&json!("a")).saturating_mul(u64::MAX), u64::MAX);
}
