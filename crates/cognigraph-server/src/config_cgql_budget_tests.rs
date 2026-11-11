//! CG-111: the CGQL budgets the server applies have finite defaults.

use super::{cgql_limit, cgql_time_budget_ms};

#[test]
fn the_time_budget_follows_the_request_timeout_unless_set() {
    // Nine tenths, so the query's refusal is answered before the timeout.
    assert_eq!(cgql_time_budget_ms(None, 30), 27_000);
    assert_eq!(cgql_time_budget_ms(None, 1), 900);
    assert_eq!(cgql_time_budget_ms(None, 0), 0);
    assert_eq!(cgql_time_budget_ms(Some("250"), 30), 250);
    assert_eq!(cgql_time_budget_ms(Some(" 250 "), 30), 250);
    // An explicit zero is the only way to switch the budget off.
    assert_eq!(cgql_time_budget_ms(Some("0"), 30), 0);
    // Garbage and negative numbers fall back to the safe value.
    assert_eq!(cgql_time_budget_ms(Some("soon"), 30), 27_000);
    assert_eq!(cgql_time_budget_ms(Some("-1"), 30), 27_000);
    assert_eq!(cgql_time_budget_ms(None, u64::MAX), u64::MAX);
}

#[test]
fn row_and_memory_limits_default_to_finite_values() {
    let rows = cognigraph_query::DEFAULT_MAX_INTERMEDIATE_ROWS;
    let bytes = cognigraph_query::DEFAULT_MAX_MEMORY_BYTES;
    assert!(rows > 0 && bytes > 0);
    assert_eq!(cgql_limit(None, rows), rows);
    assert_eq!(cgql_limit(Some("5000"), rows), 5_000);
    assert_eq!(cgql_limit(Some("0"), bytes), 0);
    assert_eq!(cgql_limit(Some("lots"), bytes), bytes);
    assert_eq!(cgql_limit(Some("18446744073709551616"), bytes), bytes);
}

#[test]
fn zero_means_unlimited_in_the_budget_handed_to_the_executor() {
    let mut config = super::Config::from_env();
    config.cgql_max_source_rows = 0;
    config.cgql_time_budget_ms = 0;
    config.cgql_max_intermediate_rows = 0;
    config.cgql_max_memory_bytes = 0;
    assert_eq!(
        config.cgql_budget(),
        cognigraph_query::ExecutionBudget::unlimited()
    );
    config.cgql_max_source_rows = 7;
    config.cgql_time_budget_ms = 8;
    config.cgql_max_intermediate_rows = 9;
    config.cgql_max_memory_bytes = 10;
    assert_eq!(
        config.cgql_budget(),
        cognigraph_query::ExecutionBudget {
            max_source_rows: Some(7),
            time_budget_ms: Some(8),
            max_intermediate_rows: Some(9),
            max_memory_bytes: Some(10),
        }
    );
}
