//! CG-197: the caller of a CGQL query can stop it. The query ends at its
//! next deadline check, whether or not it has a time budget.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use cognigraph_core::{CollectionType, GraphBackend};
use cognigraph_native::NativeBackend;
use cognigraph_query::{
    Cancellation, ExecutionBudget, ExecutionError, QueryMode,
    parse_and_execute_backend_cancellable, parse_and_execute_backend_with_options,
};
use serde_json::{Value, json};

/// Three nested scans of `c`, the innermost depending on the outermost row:
/// 27 billion comparisons over 3,000 rows, keeping almost nothing.
const LONG_READ: &str = "FOR a IN c \
    LET x = (FOR b IN c \
        LET y = (FOR d IN c FILTER d.n == b.n + a.n RETURN 1) \
        RETURN LENGTH(y)) \
    RETURN LENGTH(x)";

async fn backend(rows: usize) -> Arc<NativeBackend> {
    let backend = NativeBackend::new();
    backend
        .ensure_collection("c", CollectionType::Document)
        .await
        .unwrap();
    backend
        .bulk_insert(
            "c",
            (0..rows)
                .map(|n| json!({"_key": format!("k{n}"), "n": n}))
                .collect(),
        )
        .unwrap();
    Arc::new(backend)
}

async fn run(
    backend: &NativeBackend,
    query: &str,
    cancellation: &Cancellation,
) -> Result<Vec<Value>, ExecutionError> {
    parse_and_execute_backend_cancellable(
        query,
        backend,
        &HashMap::new(),
        QueryMode::ReadWrite,
        ExecutionBudget::unlimited(),
        cancellation,
    )
    .await
}

#[tokio::test]
async fn a_query_cancelled_before_it_starts_does_nothing() {
    let backend = backend(3).await;
    let cancellation = Cancellation::new();
    cancellation.cancel();
    for query in [
        "RETURN 1",
        "FOR d IN c RETURN d",
        "EXPLAIN FOR d IN c RETURN d",
        r#"INSERT { _key: "late", n: 9 } INTO c"#,
        r#"FOR d IN c UPDATE d._key WITH { touched: true } IN c"#,
        // Not even parsed: nobody is there to read the syntax error.
        "FOR FOR FOR",
    ] {
        assert_eq!(
            run(&backend, query, &cancellation).await,
            Err(ExecutionError::Cancelled),
            "{query}"
        );
    }
    let stored = backend.list_documents("c", None, None).await.unwrap();
    assert_eq!(stored.len(), 3);
    assert!(stored.iter().all(|doc| doc.get("touched").is_none()));
}

#[tokio::test]
async fn an_unraised_cancellation_changes_no_result() {
    let backend = backend(40).await;
    let cancellation = Cancellation::new();
    for query in [
        "FOR d IN c SORT d.n DESC LIMIT 3 RETURN d.n",
        "FOR a IN c FOR b IN c FILTER a.n == b.n COLLECT g = a.n % 4 WITH COUNT INTO n RETURN [g, n]",
        "FOR a IN c LET x = (FOR b IN c FILTER b.n < a.n RETURN 1) RETURN LENGTH(x)",
        "RETURN 1 + \"a\"",
        "FOR d IN nowhere RETURN d",
    ] {
        let plain = parse_and_execute_backend_with_options(
            query,
            backend.as_ref(),
            &HashMap::new(),
            QueryMode::ReadWrite,
            ExecutionBudget::unlimited(),
        )
        .await;
        assert_eq!(run(&backend, query, &cancellation).await, plain, "{query}");
    }
    assert!(!cancellation.is_cancelled());
}

/// Run `query` on a blocking thread, cancel it after it has started, and
/// return how long it took to stop.
async fn cancelled_in_flight(backend: Arc<NativeBackend>, query: &'static str) -> Duration {
    let cancellation = Cancellation::new();
    let remote = cancellation.clone();
    let runtime = tokio::runtime::Handle::current();
    let worker =
        tokio::task::spawn_blocking(move || runtime.block_on(run(&backend, query, &remote)));
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert!(!worker.is_finished(), "the query is still running: {query}");
    let raised = Instant::now();
    cancellation.cancel();
    let outcome = tokio::time::timeout(Duration::from_secs(10), worker)
        .await
        .expect("the query stopped")
        .unwrap();
    assert_eq!(outcome, Err(ExecutionError::Cancelled), "{query}");
    raised.elapsed()
}

#[tokio::test(flavor = "multi_thread")]
async fn cancelling_stops_a_running_query_that_has_no_time_budget() {
    let backend = backend(3_000).await;
    let stopped = cancelled_in_flight(backend.clone(), LONG_READ).await;
    assert!(stopped < Duration::from_secs(5), "{stopped:?}");

    // The rows of a mutation are computed first, so a query cancelled while
    // it computes them has written nothing.
    let stopped = cancelled_in_flight(
        backend.clone(),
        "FOR a IN c \
            LET x = (FOR b IN c \
                LET y = (FOR d IN c FILTER d.n == b.n + a.n RETURN 1) \
                RETURN LENGTH(y)) \
            UPDATE a._key WITH { touched: LENGTH(x) } IN c",
    )
    .await;
    assert!(stopped < Duration::from_secs(5), "{stopped:?}");
    let stored = backend.list_documents("c", None, None).await.unwrap();
    assert_eq!(stored.len(), 3_000);
    assert!(stored.iter().all(|doc| doc.get("touched").is_none()));
}
