//! CG-86: the index routes declare, list and drop unique constraints on
//! document collections, and refuse everything the backend refuses before
//! any constraint is stored.

use axum::Json;
use axum::extract::{Path, State};
use cognigraph_core::{CogniGraphError, CollectionType};
use cognigraph_native::NativeBackend;
use serde_json::json;

use super::{IndexRequest, drop_index, ensure_index, list_indexes};
use crate::state::AppState;

fn request(fields: &[&str]) -> IndexRequest {
    serde_json::from_value(json!({ "fields": fields })).unwrap()
}

#[tokio::test]
async fn declare_list_enforce_and_drop() {
    let state = AppState::new(NativeBackend::new());
    state
        .backend
        .ensure_collection("users", CollectionType::Document)
        .await
        .unwrap();
    let Json(made) = ensure_index(
        State(state.clone()),
        Path("users".into()),
        Json(request(&["email"])),
    )
    .await
    .unwrap();
    assert_eq!(made["collection"], "users");
    assert_eq!(made["index"]["name"], "email_unique");
    assert_eq!(
        made["index"]["unique"], true,
        "the route defaults to unique"
    );

    // Idempotent, and listed once.
    let Json(_) = ensure_index(
        State(state.clone()),
        Path("users".into()),
        Json(request(&["email"])),
    )
    .await
    .unwrap();
    let Json(listed) = list_indexes(State(state.clone()), Path("users".into()))
        .await
        .unwrap();
    assert_eq!(listed["count"], 1, "{listed}");
    assert_eq!(listed["indexes"][0]["fields"], json!(["email"]));

    // Enforced on the write path.
    state
        .backend
        .create_document("users", json!({ "_key": "a", "email": "x" }))
        .await
        .unwrap();
    let err = state
        .backend
        .create_document("users", json!({ "_key": "b", "email": "x" }))
        .await
        .unwrap_err();
    assert!(
        matches!(err, CogniGraphError::UniqueViolation { .. }),
        "{err:?}"
    );

    let Json(dropped) = drop_index(
        State(state.clone()),
        Path(("users".into(), "email_unique".into())),
    )
    .await
    .unwrap();
    assert_eq!(dropped["dropped"], true);
    let Json(again) = drop_index(
        State(state.clone()),
        Path(("users".into(), "email_unique".into())),
    )
    .await
    .unwrap();
    assert_eq!(again["dropped"], false);
    state
        .backend
        .create_document("users", json!({ "_key": "b", "email": "x" }))
        .await
        .unwrap();
}

#[tokio::test]
async fn refusals_happen_before_any_constraint_is_stored() {
    let state = AppState::new(NativeBackend::new());
    state
        .backend
        .ensure_collection("links", CollectionType::Edge)
        .await
        .unwrap();
    // Edge collection: validation error from the backend.
    let err = ensure_index(
        State(state.clone()),
        Path("links".into()),
        Json(request(&["x"])),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err.0, CogniGraphError::ValidationError(_)),
        "{err:?}"
    );
    // System and managed names: refused by the guard as forbidden.
    for name in ["_users", "neurons", "side_views"] {
        let err = ensure_index(
            State(state.clone()),
            Path(name.into()),
            Json(request(&["x"])),
        )
        .await
        .unwrap_err();
        assert!(
            matches!(
                err.0,
                CogniGraphError::Forbidden(_) | CogniGraphError::ValidationError(_)
            ),
            "{name}: {err:?}"
        );
    }
    // Malformed definitions: 400 before the backend.
    for body in [
        json!({ "fields": [] }),
        json!({ "fields": ["a"], "index_type": "fulltext" }),
        json!({ "fields": ["a"], "name": "bad/name" }),
    ] {
        let req: IndexRequest = serde_json::from_value(body.clone()).unwrap();
        let err = ensure_index(State(state.clone()), Path("users".into()), Json(req))
            .await
            .unwrap_err();
        assert!(
            matches!(err.0, CogniGraphError::ValidationError(_)),
            "{body}: {err:?}"
        );
    }
    // Declaring over existing duplicates is a 409 and stores nothing.
    for key in ["a", "b"] {
        state
            .backend
            .create_document("users", json!({ "_key": key, "email": "same" }))
            .await
            .unwrap();
    }
    let err = ensure_index(
        State(state.clone()),
        Path("users".into()),
        Json(request(&["email"])),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err.0, CogniGraphError::UniqueViolation { .. }),
        "{err:?}"
    );
    let Json(listed) = list_indexes(State(state.clone()), Path("users".into()))
        .await
        .unwrap();
    assert_eq!(listed["count"], 0);
    // Listing an unknown collection is an empty catalog, not an error.
    let Json(none) = list_indexes(State(state), Path("nothing".into()))
        .await
        .unwrap();
    assert_eq!(none["count"], 0);
}
