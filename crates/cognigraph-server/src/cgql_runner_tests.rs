//! CG-197: a CGQL query runs off the runtime workers, a bounded number at a
//! time, and stops when its request is dropped.
//!
//! Each test body runs on its own thread; everything it spawns shares the
//! two runtime workers, as connections do in the server. The waits are
//! thread sleeps on purpose: a runtime timer cannot fire while every worker
//! is busy, which is the failure these tests are for.

use std::time::{Duration, Instant};

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use cognigraph_cache::{CacheConfig, InMemoryCache};
use cognigraph_native::NativeBackend;
use cognigraph_query::ExecutionBudget;
use serde_json::{Value, json};
use tower::ServiceExt;

use super::{CgqlSlots, concurrency};
use crate::state::AppState;

const WORKERS: usize = 2;
const ROUTES: [&str; 2] = ["/api/query", "/api/search/query"];

/// Runs until something stops it, and keeps almost nothing. The innermost
/// scan depends on the outermost row, so nothing is computed once and reused:
/// 3,000 rows make 27 billion comparisons in any build.
const LONG: &str = "FOR a IN c \
    LET x = (FOR b IN c \
        LET y = (FOR d IN c FILTER d.n == b.n + a.n RETURN 1) \
        RETURN LENGTH(y)) \
    RETURN LENGTH(x)";

async fn state(time_budget_ms: u64, slots: usize) -> AppState {
    let mut state =
        AppState::new(NativeBackend::new()).with_cache(InMemoryCache::new(CacheConfig::default()));
    state.cgql_budget = ExecutionBudget {
        time_budget_ms: Some(time_budget_ms),
        ..ExecutionBudget::unlimited()
    };
    state.cgql_slots = CgqlSlots::new(slots);
    for n in 0..3_000 {
        state
            .backend
            .create_document("c", json!({"_key": format!("k{n}"), "n": n}))
            .await
            .unwrap();
    }
    state
}

fn app(state: &AppState) -> Router {
    Router::new()
        .nest("/health", crate::routes::health::router())
        .nest("/api/query", crate::routes::query::router())
        .nest("/api/search", crate::routes::search::router())
        .with_state(state.clone())
}

async fn send(app: Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = app.oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn query(route: &str, text: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(route)
        .header("content-type", "application/json")
        .body(Body::from(json!({ "query": text }).to_string()))
        .unwrap()
}

fn health() -> Request<Body> {
    Request::builder()
        .uri("/health")
        .body(Body::empty())
        .unwrap()
}

/// Wait on the test thread, without the runtime, and say how long it took.
fn wait_until(what: &str, condition: impl Fn() -> bool) -> Duration {
    let started = Instant::now();
    while !condition() {
        assert!(started.elapsed() < Duration::from_secs(10), "{what}");
        std::thread::sleep(Duration::from_millis(5));
    }
    started.elapsed()
}

fn generation(state: &AppState) -> u64 {
    state.cache.as_ref().unwrap().result_generation()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn health_answers_while_every_runtime_worker_has_a_long_query() {
    let state = state(3_000, WORKERS).await;
    let app = app(&state);
    for route in ROUTES {
        let queries: Vec<_> = (0..WORKERS)
            .map(|_| tokio::spawn(send(app.clone(), query(route, LONG))))
            .collect();
        std::thread::sleep(Duration::from_millis(400));
        assert_eq!(state.cgql_slots.free(), 0, "{route}");

        let asked = Instant::now();
        let (status, _) = tokio::spawn(send(app.clone(), health())).await.unwrap();
        let waited = asked.elapsed();
        assert_eq!(status, StatusCode::OK, "{route}");
        assert!(waited < Duration::from_secs(1), "{route}: {waited:?}");

        // The queries were still running: each used its whole time budget.
        for query in queries {
            let (status, body) = query.await.unwrap();
            assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{route} {body}");
            assert_eq!(body["budget"], "time", "{route} {body}");
        }
        assert_eq!(state.cgql_slots.free(), WORKERS, "{route}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_query_beyond_the_limit_waits_and_then_has_its_whole_time_budget() {
    const BUDGET: Duration = Duration::from_millis(700);
    let state = state(BUDGET.as_millis() as u64, 1).await;
    let app = app(&state);
    for route in ROUTES {
        let first = tokio::spawn(send(app.clone(), query(route, LONG)));
        wait_until("the first query starts", || state.cgql_slots.free() == 0);
        let second = tokio::spawn(send(app.clone(), query(route, LONG)));
        let cheap = tokio::spawn(send(app.clone(), query(route, "RETURN 1")));

        let (status, body) = first.await.unwrap();
        let first_ended = Instant::now();
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{route} {body}");
        // Waiting for the slot did not use up the second query's budget.
        let (status, body) = second.await.unwrap();
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{route} {body}");
        assert_eq!(body["budget"], "time", "{route} {body}");
        assert_eq!(body["limit"], 700, "{route} {body}");
        let after_first = first_ended.elapsed();
        assert!(
            after_first >= BUDGET - Duration::from_millis(100),
            "{route}: {after_first:?}"
        );
        // A query that waited behind them is answered, not refused.
        let (status, body) = cheap.await.unwrap();
        assert_eq!(status, StatusCode::OK, "{route} {body}");
        assert_eq!(body["results"], json!([1.0]), "{route}");
        assert_eq!(state.cgql_slots.free(), 1, "{route}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dropping_the_request_stops_its_query_long_before_the_time_budget() {
    let state = state(120_000, 1).await;
    let app = app(&state);
    for route in ROUTES {
        let running = tokio::spawn(send(app.clone(), query(route, LONG)));
        wait_until("the query starts", || state.cgql_slots.free() == 0);
        // A request dropped while it waits never takes the slot.
        let queued = tokio::spawn(send(app.clone(), query(route, LONG)));
        std::thread::sleep(Duration::from_millis(100));
        queued.abort();
        assert!(queued.await.unwrap_err().is_cancelled(), "{route}");
        assert_eq!(state.cgql_slots.free(), 0, "{route}");

        running.abort();
        let stopped = wait_until("the dropped query stops", || state.cgql_slots.free() == 1);
        assert!(stopped < Duration::from_secs(5), "{route}: {stopped:?}");
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(state.cgql_slots.free(), 1, "{route}");

        let (status, body) = send(app.clone(), query(route, "RETURN 1")).await;
        assert_eq!(status, StatusCode::OK, "{route} {body}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_write_route_sweeps_cached_results_whatever_becomes_of_the_query() {
    let state = state(120_000, 2).await;
    let app = app(&state);
    let start = generation(&state);

    // The read-only route cannot write, so it leaves the cache alone.
    for text in ["FOR d IN c LIMIT 2 RETURN d.n", "RETURN 1 + \"a\""] {
        send(app.clone(), query("/api/search/query", text)).await;
    }
    assert_eq!(generation(&state), start);

    let (status, body) = send(
        app.clone(),
        query("/api/query", r#"INSERT { _key: "new", n: -1 } INTO c"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let written = generation(&state);
    assert!(written > start);

    // A write that fails may have written before it failed.
    let (status, _) = send(
        app.clone(),
        query("/api/query", r#"UPDATE "nobody" WITH { n: 1 } IN c"#),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let failed = generation(&state);
    assert!(failed > written);

    // A write whose request is dropped is swept when its query has stopped,
    // although nobody is waiting for the answer.
    let long_write = format!(
        "FOR a IN c LET x = ({}) UPDATE a._key WITH {{ seen: LENGTH(x) }} IN c",
        "FOR b IN c LET y = (FOR d IN c FILTER d.n == b.n + a.n RETURN 1) RETURN LENGTH(y)"
    );
    let dropped = tokio::spawn(send(app.clone(), query("/api/query", &long_write)));
    wait_until("the write starts", || state.cgql_slots.free() == 1);
    assert_eq!(generation(&state), failed);
    dropped.abort();
    let swept = wait_until("the dropped write is swept", || generation(&state) > failed);
    assert!(swept < Duration::from_secs(5), "{swept:?}");
    assert_eq!(state.cgql_slots.free(), 2);
    // It was stopped before its first document.
    let seen = send(
        app.clone(),
        query("/api/query", "FOR d IN c FILTER d.seen != null RETURN 1"),
    )
    .await;
    assert_eq!(seen.1["count"], 0, "{seen:?}");
}

#[test]
fn the_limit_is_a_positive_number_or_the_number_of_cpus() {
    let cpus = std::thread::available_parallelism().unwrap().get();
    assert!(cpus >= 1);
    assert_eq!(concurrency(None), cpus);
    assert_eq!(concurrency(Some("3")), 3);
    assert_eq!(concurrency(Some(" 3 ")), 3);
    // There is no unlimited: zero and garbage fall back to the default.
    for text in ["0", "-1", "many", "", "1.5", "18446744073709551616"] {
        assert_eq!(concurrency(Some(text)), cpus, "{text:?}");
    }
    assert_eq!(CgqlSlots::default().limit(), cpus);
    assert_eq!(CgqlSlots::new(0).limit(), 1);
    assert_eq!(CgqlSlots::new(0).free(), 1);
    assert_eq!(CgqlSlots::new(7).free(), 7);
}
