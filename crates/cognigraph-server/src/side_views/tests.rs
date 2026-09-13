use super::*;
use crate::state::AppState;
use cognigraph_native::NativeBackend;
use std::sync::atomic::Ordering;

mod fault_backend;
mod isolation;
use fault_backend::FaultBackend;

async fn seed(state: &AppState, collection: &str, key: &str, sides: usize) {
    state
        .backend
        .create_document(collection, json!({"_key":key,"text":"Original source"}))
        .await
        .unwrap();
    for n in 0..sides {
        state.managed_backend.create_document(SIDE_VIEWS_COLLECTION,
            json!({"_key":format!("{collection}-{key}-{n}"),"document_id":format!("{collection}/{key}"),"question":"Old question"})
        ).await.unwrap();
    }
}

async fn prepare(state: &AppState, regenerate: bool) -> Source {
    state
        .side_views
        .prepare(
            state.managed_backend.as_ref(),
            "default",
            "default",
            DocumentId::new("notes", "a"),
            regenerate,
        )
        .await
        .unwrap()
        .unwrap()
}

fn surfaces(question: &str) -> Vec<Value> {
    vec![json!({"question":question,"answer":"Synthetic answer","embedding":[1.0,0.0]})]
}

#[tokio::test]
async fn exact_unicode_source_handles_are_supported_and_slash_collections_rejected() {
    let state = AppState::new(NativeBackend::new());
    for (collection, key) in [
        ("notes", "cafe\u{301}"),
        ("cafe\u{301}", "a"),
        ("nested/source", "a"),
    ] {
        seed(&state, collection, key, 0).await;
        let result = state
            .side_views
            .prepare(
                state.managed_backend.as_ref(),
                "default",
                "default",
                DocumentId::new(collection, key),
                false,
            )
            .await;
        if collection.contains('/') {
            assert!(matches!(result, Err(CogniGraphError::ValidationError(_))));
        } else {
            assert!(result.unwrap().is_some());
        }
    }
    assert!(require_source_identity(&DocumentId::new("café", "café")).is_ok());
    assert!(rows(&state).await.is_empty());
}

async fn rows(state: &AppState) -> Vec<Value> {
    if !has_side_views(state.managed_backend.as_ref())
        .await
        .unwrap()
    {
        return vec![];
    }
    state
        .backend
        .list_documents(SIDE_VIEWS_COLLECTION, None, None)
        .await
        .unwrap()
}

#[tokio::test]
async fn all_deletes_revoke_generation_even_after_recreation() {
    for method in ["direct", "batch", "query", "drop"] {
        let state = AppState::new(NativeBackend::new());
        seed(&state, "notes", "a", 2).await;
        seed(&state, "notes_other", "a", 1).await;
        let source = prepare(&state, true).await;
        match method {
            "direct" => {
                assert!(state.backend.delete_document("notes", "a").await.unwrap());
            }
            "batch" => {
                let result = state
                    .backend
                    .execute_batch(vec![
                        BatchOp::Delete {
                            collection: "notes".into(),
                            key: "a".into(),
                        },
                        BatchOp::Insert {
                            collection: "notes".into(),
                            doc: json!({"_key":"a","text":"Recreated"}),
                        },
                    ])
                    .await
                    .unwrap();
                assert_eq!(result.len(), 2);
                assert!(result[0].is_null());
                assert_eq!(result[1]["_key"], "a");
            }
            "query" => {
                cognigraph_query::parse_and_execute_backend_with_options(
                    "FOR d IN notes REMOVE d._key IN notes",
                    state.backend.as_ref(),
                    &HashMap::new(),
                    cognigraph_query::QueryMode::ReadWrite,
                    cognigraph_query::ExecutionBudget::default(),
                )
                .await
                .unwrap();
            }
            "drop" => {
                state.backend.drop_collection("notes").await.unwrap();
            }
            _ => unreachable!(),
        }
        if method != "batch" {
            seed(&state, "notes", "a", 0).await;
        }
        assert_eq!(rows(&state).await.len(), 1, "{method}");
        assert!(
            matches!(
                state
                    .side_views
                    .publish(
                        state.managed_backend.as_ref(),
                        source,
                        surfaces("Stale"),
                        true
                    )
                    .await,
                Err(CogniGraphError::DocumentConflict(_))
            ),
            "{method}"
        );
        let fresh = prepare(&state, false).await;
        assert_eq!(
            state
                .side_views
                .publish(
                    state.managed_backend.as_ref(),
                    fresh,
                    surfaces("Fresh"),
                    false
                )
                .await
                .unwrap(),
            1
        );
        let remaining = rows(&state).await;
        assert_eq!(remaining.len(), 2);
        assert!(remaining.iter().any(|row| row["question"] == "Fresh"));
        assert!(!remaining.iter().any(|row| row["question"] == "Stale"));
    }
}

#[tokio::test]
async fn overlapping_generation_is_idempotent_and_regeneration_reads_current_rows() {
    let state = AppState::new(NativeBackend::new());
    seed(&state, "notes", "a", 0).await;
    let one = prepare(&state, false).await;
    let two = prepare(&state, false).await;
    assert_eq!(
        state
            .side_views
            .publish(
                state.managed_backend.as_ref(),
                one,
                surfaces("First"),
                false
            )
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        state
            .side_views
            .publish(
                state.managed_backend.as_ref(),
                two,
                surfaces("Duplicate"),
                false
            )
            .await
            .unwrap(),
        0
    );
    let one = prepare(&state, true).await;
    let two = prepare(&state, true).await;
    state
        .side_views
        .publish(
            state.managed_backend.as_ref(),
            one,
            surfaces("Replacement one"),
            true,
        )
        .await
        .unwrap();
    state
        .side_views
        .publish(
            state.managed_backend.as_ref(),
            two,
            surfaces("Replacement two"),
            true,
        )
        .await
        .unwrap();
    let result = rows(&state).await;
    assert_eq!(result.len(), 1);
    assert_eq!(result[0]["question"], "Replacement two");
}

#[tokio::test]
async fn failed_user_batch_preserves_parent_and_side_views() {
    let state = AppState::new(NativeBackend::new());
    seed(&state, "notes", "a", 2).await;
    let before = state.managed_backend.export_snapshot().await.unwrap();
    let result = state
        .backend
        .execute_batch(vec![
            BatchOp::Delete {
                collection: "notes".into(),
                key: "a".into(),
            },
            BatchOp::Update {
                collection: "notes".into(),
                key: "missing".into(),
                merge: json!({"v":1}),
            },
        ])
        .await;
    assert!(matches!(
        result,
        Err(CogniGraphError::DocumentNotFound { .. })
    ));
    assert_eq!(
        before,
        state.managed_backend.export_snapshot().await.unwrap()
    );
}

#[tokio::test]
async fn cleanup_errors_preserve_sources_and_are_retryable() {
    for failure in ["read", "malformed", "batch", "drop_cleanup", "drop"] {
        let raw = Arc::new(FaultBackend::new());
        let state = AppState::new_shared(raw.clone());
        seed(&state, "notes", "a", 2).await;
        let before = raw.export_snapshot().await.unwrap();
        match failure {
            "read" => raw.fail_read.store(true, Ordering::SeqCst),
            "malformed" => raw.malformed.store(true, Ordering::SeqCst),
            "batch" | "drop_cleanup" => raw.fail_next_batch.store(true, Ordering::SeqCst),
            "drop" => raw.fail_drop.store(true, Ordering::SeqCst),
            _ => unreachable!(),
        }
        let result = if failure.starts_with("drop") {
            state.backend.drop_collection("notes").await
        } else {
            state
                .backend
                .delete_document("notes", "a")
                .await
                .map(|_| ())
        };
        assert!(
            matches!(result, Err(CogniGraphError::BackendError(_))),
            "{failure}"
        );
        assert!(
            state
                .backend
                .get_document("notes", "a")
                .await
                .unwrap()
                .is_some()
        );
        if failure == "drop" {
            assert!(rows(&state).await.is_empty());
        } else {
            assert_eq!(before, raw.export_snapshot().await.unwrap(), "{failure}");
        }
        if failure.starts_with("drop") {
            state.backend.drop_collection("notes").await.unwrap();
        } else {
            assert!(state.backend.delete_document("notes", "a").await.unwrap());
        }
        assert!(rows(&state).await.is_empty());
        assert!(
            state
                .backend
                .get_document("notes", "a")
                .await
                .unwrap()
                .is_none()
        );
    }
}

#[tokio::test]
async fn generation_failure_preserves_previous_rows_until_retry() {
    let raw = Arc::new(FaultBackend::new());
    let state = AppState::new_shared(raw.clone());
    seed(&state, "notes", "a", 2).await;
    let source = prepare(&state, true).await;
    let before = raw.export_snapshot().await.unwrap();
    raw.fail_next_batch.store(true, Ordering::SeqCst);
    assert!(
        state
            .side_views
            .publish(raw.as_ref(), source, surfaces("New"), true)
            .await
            .is_err()
    );
    assert_eq!(before, raw.export_snapshot().await.unwrap());
    let source = prepare(&state, true).await;
    assert_eq!(
        state
            .side_views
            .publish(raw.as_ref(), source, surfaces("New"), true)
            .await
            .unwrap(),
        1
    );
    assert_eq!(rows(&state).await.len(), 1);
}

#[tokio::test]
async fn deletion_waits_for_publication_commit_then_removes_its_rows() {
    let raw = Arc::new(FaultBackend::new());
    let state = AppState::new_shared(raw.clone());
    seed(&state, "notes", "a", 0).await;
    let source = prepare(&state, false).await;
    raw.pause_batch.store(true, Ordering::SeqCst);
    let publishing = state.clone();
    let publish = tokio::spawn(async move {
        publishing
            .side_views
            .publish(
                publishing.managed_backend.as_ref(),
                source,
                surfaces("New"),
                false,
            )
            .await
    });
    tokio::time::timeout(std::time::Duration::from_secs(2), raw.entered.notified())
        .await
        .unwrap();
    let deleting = state.clone();
    let mut delete =
        tokio::spawn(async move { deleting.backend.delete_document("notes", "a").await });
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(30), &mut delete)
            .await
            .is_err()
    );
    raw.release.notify_one();
    assert_eq!(publish.await.unwrap().unwrap(), 1);
    assert!(delete.await.unwrap().unwrap());
    assert!(rows(&state).await.is_empty());
}

#[tokio::test]
async fn non_atomic_cleanup_stops_before_parent_delete_and_retries_partial_progress() {
    let raw = Arc::new(FaultBackend::new());
    raw.atomic.store(false, Ordering::SeqCst);
    let state = AppState::new_shared(raw.clone());
    seed(&state, "notes", "a", 2).await;
    assert!(matches!(
        require_atomic_publication(raw.as_ref()),
        Err(CogniGraphError::ValidationError(_))
    ));
    raw.fail_side_delete_at.store(2, Ordering::SeqCst);
    assert!(state.backend.delete_document("notes", "a").await.is_err());
    assert!(
        state
            .backend
            .get_document("notes", "a")
            .await
            .unwrap()
            .is_some()
    );
    assert_eq!(rows(&state).await.len(), 1);
    assert!(state.backend.delete_document("notes", "a").await.unwrap());
    assert!(rows(&state).await.is_empty());
}

#[tokio::test]
async fn absent_parent_delete_and_collection_drop_repair_legacy_orphans() {
    let state = AppState::new(NativeBackend::new());
    state
        .managed_backend
        .create_document(SIDE_VIEWS_COLLECTION, json!({"document_id":"notes/a"}))
        .await
        .unwrap();
    assert_eq!(
        state
            .side_views
            .delete_document(state.managed_backend.as_ref(), "notes", "a")
            .await
            .unwrap(),
        (false, 1)
    );
    state
        .managed_backend
        .create_document(SIDE_VIEWS_COLLECTION, json!({"document_id":"notes/a"}))
        .await
        .unwrap();
    state.backend.drop_collection("notes").await.unwrap();
    assert!(rows(&state).await.is_empty());
}
