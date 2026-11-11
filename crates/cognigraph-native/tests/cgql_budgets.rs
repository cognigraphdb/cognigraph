//! CG-111: CGQL budgets cover the work a query does after its scans. Joins,
//! COLLECT and SORT draw on a row budget, built values on a memory budget,
//! and both stop a query before the oversized result exists.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use cognigraph_core::{CollectionType, GraphBackend};
use cognigraph_native::NativeBackend;
use cognigraph_query::{
    DEFAULT_MAX_INTERMEDIATE_ROWS, DEFAULT_MAX_MEMORY_BYTES, ExecutionBudget, ExecutionError,
    QueryMode, parse_and_execute_backend_with_options,
};
use serde_json::{Value, json};

async fn collection(backend: &NativeBackend, name: &str, docs: Vec<Value>) {
    backend
        .ensure_collection(name, CollectionType::Document)
        .await
        .unwrap();
    backend.bulk_insert(name, docs).unwrap();
}

fn numbered(count: usize) -> Vec<Value> {
    (0..count)
        .map(|n| json!({"_key": format!("k{n}"), "n": n, "g": n % 5}))
        .collect()
}

async fn run(
    backend: &NativeBackend,
    query: &str,
    budget: ExecutionBudget,
) -> Result<Vec<Value>, ExecutionError> {
    parse_and_execute_backend_with_options(
        query,
        backend,
        &HashMap::new(),
        QueryMode::ReadOnly,
        budget,
    )
    .await
}

fn rows(max: u64) -> ExecutionBudget {
    ExecutionBudget {
        max_intermediate_rows: Some(max),
        ..ExecutionBudget::unlimited()
    }
}

fn memory(max: u64) -> ExecutionBudget {
    ExecutionBudget {
        max_memory_bytes: Some(max),
        ..ExecutionBudget::unlimited()
    }
}

/// Accept `total` intermediate rows exactly and refuse one fewer.
async fn row_boundary(backend: &NativeBackend, query: &str, total: u64, results: usize) {
    let accepted = run(backend, query, rows(total)).await;
    assert_eq!(
        accepted.as_ref().map(Vec::len),
        Ok(results),
        "{query} at {total}"
    );
    assert_eq!(
        run(backend, query, rows(total - 1)).await,
        Err(ExecutionError::IntermediateRowBudgetExceeded(total - 1)),
        "{query} at {}",
        total - 1
    );
}

#[test]
fn the_default_budget_bounds_rows_and_memory_and_unlimited_is_explicit() {
    let default = ExecutionBudget::default();
    assert_eq!(
        default.max_intermediate_rows,
        Some(DEFAULT_MAX_INTERMEDIATE_ROWS)
    );
    assert_eq!(default.max_memory_bytes, Some(DEFAULT_MAX_MEMORY_BYTES));
    assert_eq!(default.max_source_rows, None);
    assert_eq!(default.time_budget_ms, None);
    let unlimited = ExecutionBudget::unlimited();
    assert_eq!(unlimited.max_intermediate_rows, None);
    assert_eq!(unlimited.max_memory_bytes, None);
}

#[tokio::test]
async fn a_nested_for_is_charged_for_every_row_it_produces() {
    let backend = NativeBackend::new();
    collection(&backend, "c", numbered(300)).await;

    // 300 rows from the first FOR, 90,000 from the second.
    row_boundary(&backend, "FOR a IN c FOR b IN c RETURN 1", 90_300, 90_000).await;

    // The ticket's query. The third FOR would make 27 million rows; it is
    // refused when it is reached, before one of them is built.
    let triple = "FOR a IN c FOR b IN c FOR d IN c RETURN 1";
    assert_eq!(
        run(&backend, triple, rows(1_000_000)).await,
        Err(ExecutionError::IntermediateRowBudgetExceeded(1_000_000))
    );
    // With nothing configured the default memory budget refuses it.
    assert_eq!(
        run(&backend, triple, ExecutionBudget::default()).await,
        Err(ExecutionError::MemoryBudgetExceeded(
            DEFAULT_MAX_MEMORY_BYTES
        ))
    );
    // The row budget is one counter for the whole query, subqueries
    // included: 2 rows pass the pushed-down filter, and the subquery, which
    // does not depend on them, runs once for 300.
    row_boundary(
        &backend,
        "FOR a IN c FILTER a.n < 2 LET inner = (FOR b IN c RETURN 1) RETURN LENGTH(inner)",
        302,
        2,
    )
    .await;
    // A subquery that depends on the outer row runs once per row.
    row_boundary(
        &backend,
        "FOR a IN c FILTER a.n < 2 \
         LET inner = (FOR b IN c FILTER b.n >= a.n RETURN 1) RETURN LENGTH(inner)",
        602,
        2,
    )
    .await;
}

#[tokio::test]
async fn a_join_on_a_non_string_key_is_charged_as_the_cross_product_it_is() {
    let backend = NativeBackend::new();
    let numeric = |field: &str| {
        (0..200)
            .map(|n| json!({"_key": format!("k{n}"), field: n}))
            .collect::<Vec<_>>()
    };
    let text = |field: &str| {
        (0..200)
            .map(|n| json!({"_key": format!("k{n}"), field: format!("id-{n}")}))
            .collect::<Vec<_>>()
    };
    collection(&backend, "orders", numeric("customer_id")).await;
    collection(&backend, "customers", numeric("id")).await;
    collection(&backend, "text_orders", text("customer_id")).await;
    collection(&backend, "text_customers", text("id")).await;

    // Numeric ids cannot use the join index: every order expands against
    // every customer, 200 + 200 x 200 rows, for 200 matches.
    row_boundary(
        &backend,
        "FOR o IN orders FOR c IN customers FILTER o.customer_id == c.id RETURN 1",
        40_200,
        200,
    )
    .await;
    // String ids are served by the index: 200 + 200 rows.
    row_boundary(
        &backend,
        "FOR o IN text_orders FOR c IN text_customers FILTER o.customer_id == c.id RETURN 1",
        400,
        200,
    )
    .await;
    // The cross product is a memory cost too, counted before it is built.
    assert_eq!(
        run(
            &backend,
            "FOR o IN orders FOR c IN customers FILTER o.customer_id == c.id RETURN 1",
            memory(64 * 1024),
        )
        .await,
        Err(ExecutionError::MemoryBudgetExceeded(64 * 1024))
    );
}

/// `LET v0 = seed LET v1 = f(v0 x 10) … RETURN LENGTH(v<levels>)`.
fn growth(seed: &str, levels: usize, wrap: impl Fn(&str) -> String) -> String {
    let mut query = format!("LET v0 = {seed} ");
    for level in 1..=levels {
        let previous = format!("v{}", level - 1);
        let arguments = [previous.as_str(); 10].join(", ");
        query.push_str(&format!("LET v{level} = {} ", wrap(&arguments)));
    }
    query.push_str(&format!("RETURN LENGTH(v{levels})"));
    query
}

#[tokio::test]
async fn values_that_multiply_stop_at_the_memory_budget() {
    let backend = NativeBackend::new();
    let megabyte = 1 << 20;
    let concat = |arguments: &str| format!("CONCAT({arguments})");
    let array = |arguments: &str| format!("[{arguments}]");

    // Three levels make a 10,000-byte string; well inside one megabyte.
    assert_eq!(
        run(
            &backend,
            &growth("\"0123456789\"", 3, concat),
            memory(megabyte)
        )
        .await,
        Ok(vec![json!(10_000)])
    );
    // The ticket's query: thirteen levels ask for 10^14 bytes.
    let bomb = growth("\"0123456789\"", 13, concat);
    assert!(bomb.len() < 1_000, "{}", bomb.len());
    assert_eq!(
        run(&backend, &bomb, memory(megabyte)).await,
        Err(ExecutionError::MemoryBudgetExceeded(megabyte))
    );
    // Arrays multiply the same way without calling a function.
    assert_eq!(
        run(&backend, &growth("[1, 2, 3]", 2, array), memory(megabyte)).await,
        Ok(vec![json!(10)])
    );
    assert_eq!(
        run(&backend, &growth("[1, 2, 3]", 12, array), memory(megabyte)).await,
        Err(ExecutionError::MemoryBudgetExceeded(megabyte))
    );
    // A replacement turns every match into a copy of a large text: 1,000
    // matches of a 10,000-byte replacement. It stops before that string
    // exists, and succeeds when the budget has room for it.
    let replace = format!(
        "{} LET out = REGEX_REPLACE(v3, \"0\", v3) RETURN LENGTH(out)",
        growth("\"0123456789\"", 3, concat)
            .rsplit_once("RETURN")
            .unwrap()
            .0
    );
    assert_eq!(
        run(&backend, &replace, memory(megabyte)).await,
        Err(ExecutionError::MemoryBudgetExceeded(megabyte))
    );
    assert_eq!(
        run(&backend, &replace, memory(64 * megabyte)).await,
        Ok(vec![json!(10_009_000)])
    );
    // A string literal repeated per row is counted as well.
    let literal = format!(
        "FOR i IN [1, 2, 3, 4, 5, 6, 7, 8] RETURN \"{}\"",
        "x".repeat(200_000)
    );
    assert_eq!(
        run(&backend, &literal, memory(megabyte)).await,
        Err(ExecutionError::MemoryBudgetExceeded(megabyte))
    );
    // Without a budget the small cases still run.
    assert_eq!(
        run(&backend, &literal, ExecutionBudget::unlimited())
            .await
            .map(|rows| rows.len()),
        Ok(8)
    );
}

#[tokio::test]
async fn collect_and_sort_read_their_rows_against_the_budget() {
    let backend = NativeBackend::new();
    collection(&backend, "c", numbered(500)).await;

    // 500 rows from FOR and 500 read by SORT.
    row_boundary(&backend, "FOR d IN c SORT d.n DESC RETURN d.n", 1_000, 500).await;
    // 500 from FOR, 500 read by COLLECT, and its 5 groups read by SORT.
    row_boundary(
        &backend,
        "FOR d IN c COLLECT g = d.g WITH COUNT INTO n SORT g RETURN [g, n]",
        1_005,
        5,
    )
    .await;
    // A query with neither pays for its FOR only, and a filter the scan
    // applies itself leaves just the rows that passed.
    row_boundary(&backend, "FOR d IN c RETURN d.n", 500, 500).await;
    row_boundary(&backend, "FOR d IN c FILTER d.n < 3 RETURN d.n", 3, 3).await;

    // What a group keeps is memory: 500 captured documents.
    let captured = "FOR d IN c COLLECT g = d.g INTO members = d RETURN LENGTH(members)";
    assert_eq!(
        run(&backend, captured, memory(16 * 1024)).await,
        Err(ExecutionError::MemoryBudgetExceeded(16 * 1024))
    );
    assert_eq!(
        run(&backend, captured, memory(16 << 20)).await,
        Ok(vec![json!(100); 5])
    );
}

#[tokio::test]
async fn a_subquery_returns_the_memory_of_the_rows_it_built() {
    let backend = NativeBackend::new();
    collection(&backend, "outer", numbered(60)).await;
    collection(&backend, "inner", numbered(200)).await;
    // A range condition cannot use the join index, so every run copies all
    // 200 inner documents. The copies are gone when the run returns: sixty
    // runs fit a budget that two runs' worth of rows would overflow.
    let query = "FOR o IN outer \
                 LET later = (FOR i IN inner FILTER i.n > o.n RETURN i.n) \
                 RETURN LENGTH(later)";
    let one_megabyte = run(&backend, query, memory(1 << 20)).await.unwrap();
    assert_eq!(one_megabyte.len(), 60);
    assert_eq!(one_megabyte[0], json!(199));
    // The budget still applies inside each run.
    assert_eq!(
        run(&backend, query, memory(32 * 1024)).await,
        Err(ExecutionError::MemoryBudgetExceeded(32 * 1024))
    );
}

#[tokio::test]
async fn a_time_budget_ends_a_join_while_it_is_being_built() {
    let backend = NativeBackend::new();
    collection(&backend, "c", numbered(2_000)).await;
    // Four million joined rows take seconds to build. The deadline is read
    // inside the join, so the query ends close to its 50 ms.
    let started = Instant::now();
    let result = run(
        &backend,
        "FOR a IN c FOR b IN c RETURN 1",
        ExecutionBudget {
            time_budget_ms: Some(50),
            ..ExecutionBudget::unlimited()
        },
    )
    .await;
    assert_eq!(result, Err(ExecutionError::TimeBudgetExceeded));
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "{:?}",
        started.elapsed()
    );
}

/// The smallest memory budget, to the kilobyte, under which `query` passes.
async fn smallest_passing_budget(backend: &NativeBackend, query: &str) -> u64 {
    let (mut refused, mut accepted) = (0u64, 1u64 << 32);
    assert!(run(backend, query, memory(accepted)).await.is_ok());
    while accepted - refused > 1_024 {
        let middle = refused + (accepted - refused) / 2;
        if run(backend, query, memory(middle)).await.is_ok() {
            accepted = middle;
        } else {
            refused = middle;
        }
    }
    accepted
}

#[tokio::test]
async fn a_round_repeated_for_document_lookups_is_not_charged_twice() {
    let backend = NativeBackend::new();
    let padding = "p".repeat(4_096);
    collection(
        &backend,
        "refs",
        (0..300)
            .map(|n| json!({"_key": format!("r{n}"), "target": format!("c/k{n}"), "pad": padding}))
            .collect(),
    )
    .await;
    collection(&backend, "c", numbered(300)).await;

    // DOCUMENT() is answered by fetching what a first round asked for and
    // running that part of the plan again. The first round's values are
    // dropped, so they must not stay on the budget.
    let plain = "FOR r IN refs LET big = CONCAT(r.pad, r.pad) RETURN LENGTH(big)";
    let looked_up = "FOR r IN refs \
                     LET d = DOCUMENT(r.target) \
                     LET big = CONCAT(r.pad, r.pad, d._key) \
                     RETURN LENGTH(big)";
    assert_eq!(
        run(&backend, looked_up, ExecutionBudget::unlimited())
            .await
            .unwrap()
            .len(),
        300
    );
    let once = smallest_passing_budget(&backend, plain).await;
    let repeated = smallest_passing_budget(&backend, looked_up).await;
    assert!(once > 2_000_000, "{once}");
    // The looked-up documents and their keys add a little; a second charge
    // for `big` would nearly double it.
    assert!(repeated < once + once / 3, "{repeated} against {once}");
}
