//! Archival.

use super::*;

#[tokio::test]
async fn archival_preserves_detail_and_replay_while_reconciliation_repairs_catalog() {
    let state = seeded().await;
    let submission = state
        .jobs
        .submit(
            state.clone(),
            "default".into(),
            "default".into(),
            JobActor::request(None),
            "archive-r1",
            JobKind::ConstructEvaluate,
            eval_input("archive"),
        )
        .await
        .unwrap();
    let finished = wait_terminal(&state, &submission.job.id).await;
    let dry_run = state
        .jobs
        .archive_terminal(
            "default",
            "default",
            JobActor::request(None),
            Some(now_millis() + 1),
            Some(10),
            None,
            true,
            Some("retention preview".into()),
        )
        .await
        .unwrap();
    assert_eq!(dry_run.eligible, 1);
    assert_eq!(dry_run.archived, 0);
    let applied = state
        .jobs
        .archive_terminal(
            "default",
            "default",
            JobActor::request(None),
            Some(now_millis() + 1),
            Some(10),
            None,
            false,
            Some("retention policy".into()),
        )
        .await
        .unwrap();
    assert_eq!(applied.archived, 1);
    assert!(
        state
            .jobs
            .get_raw("default", &finished.id)
            .await
            .unwrap()
            .is_none()
    );
    let archived = state
        .jobs
        .get("default", "default", &finished.id)
        .await
        .unwrap();
    assert!(archived.archived_at_ms.is_some());
    assert!(
        archived
            .events
            .iter()
            .any(|event| event.event == "archived")
    );
    let mut competing_archive = archived.clone();
    competing_archive.updated_at_ms = competing_archive.updated_at_ms.saturating_add(1);
    let preserved = state
        .jobs
        .copy_to_archive("default", &competing_archive)
        .await
        .unwrap();
    assert_eq!(preserved.updated_at_ms, archived.updated_at_ms);
    assert_eq!(preserved.events.len(), archived.events.len());
    assert_eq!(
        preserved.events.last().map(|event| event.event.as_str()),
        archived.events.last().map(|event| event.event.as_str())
    );

    // A copy-first archival failure can temporarily leave a hot duplicate.
    // Public lifecycle reads must still prefer the immutable archive, and
    // reconciliation removes the duplicate.
    state.jobs.create_raw("default", &finished).await.unwrap();
    let visible = state
        .jobs
        .get("default", "default", &finished.id)
        .await
        .unwrap();
    assert!(visible.archived_at_ms.is_some());
    state
        .jobs
        .reconcile_fully("default", "default")
        .await
        .unwrap();
    assert!(
        state
            .jobs
            .get_raw("default", &finished.id)
            .await
            .unwrap()
            .is_none()
    );
    let replay = state
        .jobs
        .submit(
            state.clone(),
            "default".into(),
            "default".into(),
            JobActor::request(None),
            "archive-r1",
            JobKind::ConstructEvaluate,
            eval_input("archive"),
        )
        .await
        .unwrap();
    assert!(replay.replayed);
    assert!(matches!(
        state
            .jobs
            .retry(
                state.clone(),
                "default",
                "default",
                &finished.id,
                JobActor::request(None),
                "archive-retry",
                RetryMode::Restart,
                None,
            )
            .await,
        Err(CogniGraphError::DocumentConflict(_))
    ));
    let only_archived = state
        .jobs
        .list_cursor(
            "default",
            "default",
            None,
            None,
            ArchiveFilter::Only,
            10,
            None,
        )
        .await
        .unwrap();
    assert_eq!(only_archived.jobs.len(), 1);

    let key = catalog_key(&archived);
    CURRENT_TENANT
        .scope(
            "default".into(),
            state
                .jobs
                .raw_backend
                .delete_document(JOB_CATALOG_COLLECTION, &key),
        )
        .await
        .unwrap();
    state
        .jobs
        .reconcile_fully("default", "default")
        .await
        .unwrap();
    let repaired = state
        .jobs
        .list_cursor(
            "default",
            "default",
            None,
            None,
            ArchiveFilter::Only,
            10,
            None,
        )
        .await
        .unwrap();
    assert_eq!(repaired.jobs.len(), 1);
    assert!(state.jobs.health().is_ok());
    state.jobs.shutdown().await;
}
#[tokio::test]
async fn reconciliation_refuses_to_delete_a_divergent_hot_archive_duplicate() {
    let state = seeded().await;
    let submission = state
        .jobs
        .submit(
            state.clone(),
            "default".into(),
            "default".into(),
            JobActor::request(None),
            "archive-divergence-r1",
            JobKind::ConstructEvaluate,
            eval_input("archive-divergence"),
        )
        .await
        .unwrap();
    let finished = wait_terminal(&state, &submission.job.id).await;
    state
        .jobs
        .archive_terminal(
            "default",
            "default",
            JobActor::request(None),
            Some(now_millis() + 1),
            Some(10),
            None,
            false,
            None,
        )
        .await
        .unwrap();

    let mut divergent = finished.clone();
    divergent.result = Some(json!({"unexpected": "different terminal outcome"}));
    state.jobs.create_raw("default", &divergent).await.unwrap();
    assert!(matches!(
        state.jobs.reconcile_fully("default", "default").await,
        Err(CogniGraphError::BackendError(_))
    ));
    assert!(
        state
            .jobs
            .get_raw("default", &finished.id)
            .await
            .unwrap()
            .is_some(),
        "unsafe duplicate must remain for operator investigation"
    );
    assert!(
        state
            .jobs
            .get("default", "default", &finished.id)
            .await
            .unwrap()
            .archived_at_ms
            .is_some(),
        "public lifecycle lookup must fail closed to the archive"
    );
    state.jobs.shutdown().await;
}
