//! Catalog.

use super::*;

#[tokio::test]
async fn cursor_listing_is_stable_filter_bound_and_summary_only() {
    let state = seeded().await;
    state.jobs.pause_worker_claims();
    for index in 0..7 {
        state
            .jobs
            .submit(
                state.clone(),
                "default".into(),
                "default".into(),
                JobActor::request(None),
                &format!("cursor-r{index}"),
                JobKind::ConstructEvaluate,
                eval_input("cursor"),
            )
            .await
            .unwrap();
    }
    wait_unscheduled(&state).await;

    let mut cursor = None;
    let mut ids = Vec::new();
    loop {
        let page = state
            .jobs
            .list_cursor(
                "default",
                "default",
                None,
                Some(JobStatus::Queued),
                ArchiveFilter::Exclude,
                2,
                cursor.as_deref(),
            )
            .await
            .unwrap();
        for job in page.jobs {
            let public = job.public_value();
            assert!(public.get("input").is_none());
            assert!(public.get("events").is_none());
            ids.push(job.id);
        }
        let Some(next) = page.next_cursor else {
            break;
        };
        if cursor.is_none() {
            assert!(matches!(
                state
                    .jobs
                    .list_cursor(
                        "default",
                        "default",
                        None,
                        Some(JobStatus::Failed),
                        ArchiveFilter::Exclude,
                        2,
                        Some(&next),
                    )
                    .await,
                Err(CogniGraphError::ValidationError(_))
            ));
        }
        cursor = Some(next);
    }
    let unique = ids.iter().collect::<HashSet<_>>();
    assert_eq!(ids.len(), 7);
    assert_eq!(unique.len(), ids.len());
}
#[tokio::test]
async fn catalog_aliases_do_not_truncate_or_duplicate_and_reconcile_is_resumable() {
    let state = seeded().await;
    state.jobs.pause_worker_claims();
    let mut expected = HashSet::new();
    let mut canonical_document = None;
    for index in 0..3 {
        let submitted = state
            .jobs
            .submit(
                state.clone(),
                "default".into(),
                "default".into(),
                JobActor::request(None),
                &format!("catalog-alias-r{index}"),
                JobKind::ConstructEvaluate,
                eval_input("catalog-alias"),
            )
            .await
            .unwrap();
        expected.insert(submitted.job.id.clone());
        if canonical_document.is_none() {
            canonical_document = CURRENT_TENANT
                .scope(
                    "default".into(),
                    state
                        .jobs
                        .raw_backend
                        .get_document(JOB_CATALOG_COLLECTION, &catalog_key(&submitted.job)),
                )
                .await
                .unwrap();
        }
    }
    wait_unscheduled(&state).await;
    let scope = catalog_scope("default", "default");
    let alias_key = format!("{scope}!alias");
    let malformed_key = format!("{scope}!malformed");
    let canonical_alias_key = format!("{scope}-0000000000000000-alias");
    let mut alias = canonical_document.expect("canonical catalog row");
    alias["_key"] = json!(alias_key);
    CURRENT_TENANT
        .scope(
            "default".into(),
            state
                .jobs
                .raw_backend
                .create_document(JOB_CATALOG_COLLECTION, alias.clone()),
        )
        .await
        .unwrap();
    alias["_key"] = json!(canonical_alias_key);
    alias["archived"] = json!(true);
    CURRENT_TENANT
        .scope(
            "default".into(),
            state
                .jobs
                .raw_backend
                .create_document(JOB_CATALOG_COLLECTION, alias),
        )
        .await
        .unwrap();
    CURRENT_TENANT
        .scope(
            "default".into(),
            state.jobs.raw_backend.create_document(
                JOB_CATALOG_COLLECTION,
                json!({"_key": malformed_key, "schema_version": "bad"}),
            ),
        )
        .await
        .unwrap();
    CURRENT_TENANT
        .scope(
            "default".into(),
            state.jobs.raw_backend.create_document(
                JOB_CATALOG_COLLECTION,
                json!({"_key": scope.clone(), "schema_version": "bad"}),
            ),
        )
        .await
        .unwrap();

    let mut cursor = None;
    let mut listed = Vec::new();
    loop {
        let page = state
            .jobs
            .list_cursor(
                "default",
                "default",
                None,
                Some(JobStatus::Queued),
                ArchiveFilter::Exclude,
                1,
                cursor.as_deref(),
            )
            .await
            .unwrap();
        listed.extend(page.jobs.into_iter().map(|job| job.id));
        let Some(next) = page.next_cursor else {
            break;
        };
        assert!(next.starts_with("v2.list."));
        cursor = Some(next);
    }
    assert_eq!(listed.iter().cloned().collect::<HashSet<_>>(), expected);
    assert_eq!(listed.len(), expected.len());
    assert!(state.jobs.health().is_err());

    let mut reconcile_cursor = None;
    for _ in 0..32 {
        let result = state
            .jobs
            .reconcile("default", "default", true, 1, reconcile_cursor.as_deref())
            .await
            .unwrap();
        let Some(next) = result.next_cursor else {
            break;
        };
        assert!(next.starts_with("v2.reconcile."));
        reconcile_cursor = Some(next);
    }
    assert!(reconcile_cursor.is_some());
    assert!(
        state.jobs.health().is_err(),
        "dry-run must not clear health"
    );

    state
        .jobs
        .reconcile_fully("default", "default")
        .await
        .unwrap();
    for key in [
        format!("{scope}!alias"),
        format!("{scope}!malformed"),
        format!("{scope}-0000000000000000-alias"),
        scope,
    ] {
        assert!(
            CURRENT_TENANT
                .scope(
                    "default".into(),
                    state
                        .jobs
                        .raw_backend
                        .get_document(JOB_CATALOG_COLLECTION, &key),
                )
                .await
                .unwrap()
                .is_none()
        );
    }
    assert!(state.jobs.health().is_ok());

    let clean = seeded().await;
    assert!(matches!(
        clean
            .jobs
            .list_cursor(
                "default",
                "default",
                None,
                None,
                ArchiveFilter::Exclude,
                10,
                Some("invalid-cursor"),
            )
            .await,
        Err(CogniGraphError::ValidationError(_))
    ));
    assert!(clean.jobs.health().is_ok());
    state.jobs.shutdown().await;
    clean.jobs.shutdown().await;
}
