//! Production hardening: request metrics and per-IP rate limiting.
//!
//! Both are deliberately dependency-free: metrics render the Prometheus
//! text exposition format by hand, and rate limiting is a fixed-window
//! per-IP counter.

use std::collections::{HashMap, VecDeque};
use std::net::{IpAddr, SocketAddr};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use axum::extract::{ConnectInfo, Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

/// A recorded non-2xx response — the raw material for the console's
/// server-logs panel and, later, an outbound webhook sink.
#[derive(Debug, Clone, Serialize)]
pub struct ErrorEvent {
    /// Monotonic per-process sequence — a stable unique id (timestamps
    /// collide within a second) for ordering, client keys, and dedup.
    pub seq: u64,
    /// Unix seconds when the response was recorded.
    pub at: u64,
    pub method: String,
    pub path: String,
    pub status: u16,
    pub latency_ms: u64,
    /// The response's `error` field when it was our JSON error body, else
    /// a short snippet; empty when the body had none.
    pub message: String,
    /// The request's tenant, when it passed the auth/tenant gate. `None`
    /// for pre-auth failures (401) and untenanted routes — those carry no
    /// tenant-specific data and are safe to show any admin.
    pub tenant: Option<String>,
}

/// Bounded in-memory ring of recent non-2xx events. Intentionally
/// process-local and lossy: it is an at-a-glance operational aid, not an
/// audit log. (A durable/streamed sink — e.g. forwarding each `record`
/// to a Datadog Vector or other HTTP consumer — is the natural next step
/// and would hang off `record`.)
pub struct RecentErrors {
    events: Mutex<VecDeque<ErrorEvent>>,
    capacity: usize,
    seq: AtomicU64,
}

impl RecentErrors {
    pub fn new(capacity: usize) -> Self {
        Self {
            events: Mutex::new(VecDeque::with_capacity(capacity)),
            capacity,
            seq: AtomicU64::new(0),
        }
    }

    /// Stamp the event with the next sequence number and store it. The
    /// caller leaves `seq` at its default (0); this owns it.
    pub fn record(&self, mut event: ErrorEvent) {
        event.seq = self.seq.fetch_add(1, Ordering::Relaxed);
        let mut events = self.events.lock().expect("recent-errors lock poisoned");
        if events.len() == self.capacity {
            events.pop_front();
        }
        events.push_back(event);
    }

    /// Newest first, optionally limited to `tenant` (plus untenanted
    /// events, which are never tenant-specific). `None` tenant = no
    /// filter (single-tenant / auth-disabled deployments).
    pub fn snapshot(&self, tenant: Option<&str>) -> Vec<ErrorEvent> {
        let events = self.events.lock().expect("recent-errors lock poisoned");
        events
            .iter()
            .rev()
            .filter(|event| match (tenant, &event.tenant) {
                (Some(caller), Some(event_tenant)) => caller == event_tenant,
                (Some(_), None) => true,
                (None, _) => true,
            })
            .cloned()
            .collect()
    }
}

/// Combined state for the metrics middleware: the Prometheus counters and
/// the recent-errors ring share one layer so every response is seen once.
#[derive(Clone)]
pub struct Observability {
    pub metrics: std::sync::Arc<Metrics>,
    pub errors: std::sync::Arc<RecentErrors>,
}

#[derive(Default)]
pub struct Metrics {
    pub requests_total: AtomicU64,
    pub responses_2xx: AtomicU64,
    pub responses_4xx: AtomicU64,
    pub responses_5xx: AtomicU64,
    pub duration_micros_sum: AtomicU64,
    pub started_unix: AtomicU64,
}

impl Metrics {
    pub fn new() -> Self {
        let metrics = Self::default();
        metrics.started_unix.store(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or_default(),
            Ordering::Relaxed,
        );
        metrics
    }

    pub fn render(&self) -> String {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or_default();
        let total = self.requests_total.load(Ordering::Relaxed);
        format!(
            "# TYPE cognigraph_requests_total counter\n\
             cognigraph_requests_total {total}\n\
             # TYPE cognigraph_responses_total counter\n\
             cognigraph_responses_total{{class=\"2xx\"}} {}\n\
             cognigraph_responses_total{{class=\"4xx\"}} {}\n\
             cognigraph_responses_total{{class=\"5xx\"}} {}\n\
             # TYPE cognigraph_request_duration_seconds summary\n\
             cognigraph_request_duration_seconds_sum {}\n\
             cognigraph_request_duration_seconds_count {total}\n\
             # TYPE cognigraph_uptime_seconds gauge\n\
             cognigraph_uptime_seconds {}\n",
            self.responses_2xx.load(Ordering::Relaxed),
            self.responses_4xx.load(Ordering::Relaxed),
            self.responses_5xx.load(Ordering::Relaxed),
            self.duration_micros_sum.load(Ordering::Relaxed) as f64 / 1_000_000.0,
            now.saturating_sub(self.started_unix.load(Ordering::Relaxed)),
        )
    }
}

pub async fn record_metrics(
    State(obs): State<Observability>,
    req: Request,
    next: Next,
) -> Response {
    let Observability { metrics, errors } = obs;
    let start = Instant::now();
    metrics.requests_total.fetch_add(1, Ordering::Relaxed);
    // Captured before `req` is consumed; a non-2xx event reports these.
    let method = req.method().to_string();
    let path = req.uri().path().to_string();

    let response = next.run(req).await;
    let latency = start.elapsed();
    metrics
        .duration_micros_sum
        .fetch_add(latency.as_micros() as u64, Ordering::Relaxed);
    let status = response.status().as_u16();
    let counter = match status {
        200..=399 => &metrics.responses_2xx,
        400..=499 => &metrics.responses_4xx,
        _ => &metrics.responses_5xx,
    };
    counter.fetch_add(1, Ordering::Relaxed);

    // Only non-2xx/3xx responses are recorded and logged — the body is
    // buffered ONLY on this rare path, so the hot path never pays for it.
    if status >= 400 {
        let tenant = response
            .extensions()
            .get::<TenantTag>()
            .map(|tag| tag.0.clone());
        let latency_ms = latency.as_millis() as u64;

        // Consume the body to read our JSON `error` message, then rebuild
        // the response so the client still gets it intact.
        let (parts, body) = response.into_parts();
        let bytes = axum::body::to_bytes(body, 64 * 1024)
            .await
            .unwrap_or_default();
        let message = error_message(&bytes);

        if status >= 500 {
            tracing::warn!(%method, %path, status, latency_ms, tenant = ?tenant, error = %message, "request failed");
        } else {
            tracing::info!(%method, %path, status, latency_ms, tenant = ?tenant, error = %message, "request rejected");
        }
        errors.record(ErrorEvent {
            seq: 0, // stamped by RecentErrors::record
            at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or_default(),
            method,
            path,
            status,
            latency_ms,
            message,
            tenant,
        });
        return Response::from_parts(parts, axum::body::Body::from(bytes));
    }
    response
}

/// The tenant a request resolved to, stamped onto the response by the auth
/// middleware so the outer metrics layer can attribute an error without
/// re-entering the tenant scope.
#[derive(Clone)]
pub struct TenantTag(pub String);

/// Pull a human message out of a non-2xx body: our error responses are
/// `{"error": "..."}`; anything else yields a short trimmed snippet.
fn error_message(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return String::new();
    }
    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(bytes)
        && let Some(error) = value.get("error").and_then(|v| v.as_str())
    {
        return error.to_string();
    }
    let text = String::from_utf8_lossy(bytes);
    text.trim().chars().take(200).collect()
}

/// Fixed-window per-IP rate limiter. Window state is pruned lazily.
pub struct RateLimiter {
    max_per_window: u32,
    window_secs: u64,
    windows: Mutex<HashMap<IpAddr, (u64, u32)>>,
}

impl RateLimiter {
    pub fn new(max_per_window: u32, window_secs: u64) -> Self {
        Self {
            max_per_window,
            window_secs,
            windows: Mutex::new(HashMap::new()),
        }
    }

    pub fn allow(&self, ip: IpAddr, now_unix: u64) -> bool {
        let window = now_unix / self.window_secs;
        let mut windows = self.windows.lock().expect("rate limiter lock poisoned");
        // Lazy prune: drop stale windows once the map grows.
        if windows.len() > 10_000 {
            windows.retain(|_, (w, _)| *w == window);
        }
        let entry = windows.entry(ip).or_insert((window, 0));
        if entry.0 != window {
            *entry = (window, 0);
        }
        entry.1 += 1;
        entry.1 <= self.max_per_window
    }
}

pub async fn rate_limit(
    State(limiter): State<std::sync::Arc<RateLimiter>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    req: Request,
    next: Next,
) -> Response {
    // Health and metrics probes are exempt.
    let path = req.uri().path();
    if path.starts_with("/health") || path == "/metrics" {
        return next.run(req).await;
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();
    if !limiter.allow(addr.ip(), now) {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            axum::Json(serde_json::json!({ "error": "rate limit exceeded" })),
        )
            .into_response();
    }
    next.run(req).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_limiter_fixed_window() {
        let limiter = RateLimiter::new(2, 60);
        let ip: IpAddr = "10.0.0.1".parse().unwrap();
        assert!(limiter.allow(ip, 1000));
        assert!(limiter.allow(ip, 1010));
        assert!(!limiter.allow(ip, 1015), "third request in window blocked");
        // Other IPs are independent.
        assert!(limiter.allow("10.0.0.2".parse().unwrap(), 1015));
        // Next window resets.
        assert!(limiter.allow(ip, 1080));
    }

    #[test]
    fn metrics_render_prometheus_text() {
        let metrics = Metrics::new();
        metrics.requests_total.fetch_add(3, Ordering::Relaxed);
        metrics.responses_2xx.fetch_add(2, Ordering::Relaxed);
        metrics.responses_5xx.fetch_add(1, Ordering::Relaxed);
        let text = metrics.render();
        assert!(text.contains("cognigraph_requests_total 3"));
        assert!(text.contains("cognigraph_responses_total{class=\"2xx\"} 2"));
        assert!(text.contains("cognigraph_responses_total{class=\"5xx\"} 1"));
        assert!(text.contains("cognigraph_uptime_seconds"));
    }
}
