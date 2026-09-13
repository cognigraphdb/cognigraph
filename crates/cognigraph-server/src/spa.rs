//! Serving the built console UI (`COGNIGRAPH_UI_DIST`) with an SPA
//! fallback: real files win; every other non-API path serves index.html so
//! the console's copyable path URLs (`/users/alice`, `/review`) deep-link
//! straight into the app. The API namespace never falls through to the
//! UI — unknown `/api` paths get an honest JSON 404 from [`api_not_found`]
//! instead of a 200 text/html.

use axum::Json;
use axum::http::StatusCode;
use serde_json::json;
use tower_http::services::{ServeDir, ServeFile};

/// Fallback for the nested `/api` router. Without it, an unknown API path
/// would fall through to the SPA fallback and answer 200 with index.html —
/// the worst kind of API error to debug.
pub async fn api_not_found() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::NOT_FOUND,
        Json(json!({ "error": "Unknown API route" })),
    )
}

/// Static service over the built UI: exact files first, index.html for
/// everything else (the console's client-side routes).
pub fn ui_service(dist: &str) -> ServeDir<ServeFile> {
    let index = std::path::Path::new(dist).join("index.html");
    ServeDir::new(dist).fallback(ServeFile::new(index))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::Router;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    /// A dist directory shaped like `bun run build` output.
    fn dist_fixture() -> tempfile::TempDir {
        let dir = tempfile::Builder::new()
            .prefix("cognigraph-spa-test-")
            .tempdir()
            .expect("fixture dir");
        std::fs::write(dir.path().join("index.html"), "<html>console shell</html>").expect("index");
        std::fs::write(dir.path().join("index-abc123.css"), "body{color:red}").expect("asset");
        dir
    }

    /// Mirrors the shape main.rs assembles: exact routes, the /api nest
    /// with its JSON 404 fallback, and the SPA fallback underneath.
    fn app(dist: &std::path::Path) -> Router {
        let api = Router::new()
            .route("/ping", axum::routing::get(|| async { "pong" }))
            .fallback(api_not_found);
        Router::new()
            .route("/health", axum::routing::get(|| async { "ok" }))
            .nest("/api", api)
            .fallback_service(ui_service(dist.to_str().expect("utf8 path")))
    }

    async fn get(path: &str) -> (StatusCode, String, String) {
        // Keep the fixture alive until the streamed file body has been consumed.
        // TempDir also removes it if request handling or an assertion panics.
        let dist = dist_fixture();
        let response = app(dist.path())
            .oneshot(Request::get(path).body(Body::empty()).expect("request"))
            .await
            .expect("response");
        let status = response.status();
        let content_type = response
            .headers()
            .get(axum::http::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_string();
        let body = axum::body::to_bytes(response.into_body(), 1 << 20)
            .await
            .expect("body");
        dist.close().expect("fixture cleanup");
        (
            status,
            String::from_utf8_lossy(&body).into_owned(),
            content_type,
        )
    }

    #[tokio::test]
    async fn client_routes_serve_the_index_shell() {
        for path in ["/", "/users/alice", "/review", "/collections"] {
            let (status, body, content_type) = get(path).await;
            assert_eq!(status, StatusCode::OK, "{path}");
            assert!(body.contains("console shell"), "{path} body: {body}");
            assert!(
                content_type.starts_with("text/html"),
                "{path}: {content_type}"
            );
        }
    }

    #[tokio::test]
    async fn real_assets_win_over_the_fallback() {
        let (status, body, content_type) = get("/index-abc123.css").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, "body{color:red}");
        assert!(content_type.starts_with("text/css"), "{content_type}");
    }

    #[tokio::test]
    async fn unknown_api_paths_stay_json_404() {
        let (status, body, content_type) = get("/api/definitely/not/a/route").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(body.contains("Unknown API route"), "{body}");
        assert!(
            content_type.starts_with("application/json"),
            "{content_type}"
        );
    }

    #[tokio::test]
    async fn exact_routes_still_win() {
        let (status, body, _) = get("/api/ping").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, "pong");
        let (status, body, _) = get("/health").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, "ok");
    }
}
