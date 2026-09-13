//! Stable rejection surface for tenant lifecycle in a Community binary.

use axum::Router;
use axum::routing::{get, post};

use crate::error::AppError;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(unavailable).post(unavailable))
        .route("/{name}", post(unavailable).delete(unavailable))
        .route("/{name}/admin", post(unavailable))
        .route("/{name}/quotas", post(unavailable))
}

async fn unavailable() -> AppError {
    crate::edition::required("multi-tenancy")
}
