//! CG-113: snapshot import cannot introduce unsigned M18 promotion authority
//! unless the operator has switched legacy restore on.

use super::*;

#[test]
fn unsigned_promotion_records_are_refused_on_import_before_any_write() {
    lifecycle(flow);
}

fn refused(result: Result<(), CogniGraphError>, record: &str, context: &str) {
    match result {
        Err(CogniGraphError::Forbidden(message)) => {
            assert!(
                message.contains("unsigned legacy promotion")
                    && message.contains(record)
                    && message.contains("COGNIGRAPH_ALLOW_LEGACY_PROMOTION_IMPORT"),
                "{context}: {message}"
            )
        }
        other => panic!("{context}: expected a refusal, got {other:?}"),
    }
}

async fn stored(raw: &Arc<dyn GraphBackend>, collection: &str) -> usize {
    raw.list_documents(collection, None, None)
        .await
        .map(|rows| rows.len())
        .unwrap_or(0)
}

/// A store with one unsigned M18 chain: four evaluation jobs, one v1
/// evidence bundle and one v1 `promote` decision made by an Admin.
async fn legacy_store() -> (Arc<dyn GraphBackend>, AppState, PromotionTarget) {
    let raw: Arc<dyn GraphBackend> = Arc::new(NativeBackend::new());
    raw.ensure_collection("facts", CollectionType::Edge)
        .await
        .unwrap();
    raw.create_edge(
        "facts",
        json!({
            "_key": "cg113-expected-fact",
            "_from": "entities/meridian",
            "_to": "entities/compound-x",
            "relation_type": "SUPPLIES",
            "space_id": SPACE,
            "evidence_chunk_id": "cg113-chunk-1",
        }),
    )
    .await
    .unwrap();
    let state = AppState::new_shared(raw.clone());
    let baseline = promotion_context("cg113-baseline", &eval_spec());
    let candidate = promotion_context("cg113-candidate", &eval_spec());
    let baseline_original = submit_evaluation(&state, "cg113-baseline-a", baseline.clone()).await;
    let baseline_replay = submit_evaluation(&state, "cg113-baseline-b", baseline).await;
    let candidate_original =
        submit_evaluation(&state, "cg113-candidate-a", candidate.clone()).await;
    let candidate_replay = submit_evaluation(&state, "cg113-candidate-b", candidate).await;
    let evidence = state
        .promotions
        .register_evidence(
            TENANT,
            INCARNATION,
            admin(),
            "cg113-evidence",
            RegisterEvidenceRequest {
                candidate_original_job_id: candidate_original,
                candidate_replay_job_id: candidate_replay,
                baseline_original_job_id: baseline_original,
                baseline_replay_job_id: baseline_replay,
                expected_head_decision_id: None,
            },
        )
        .await
        .unwrap()
        .record;
    assert!(evidence.governance.is_none());
    let decision = state
        .promotions
        .promote(
            TENANT,
            INCARNATION,
            &evidence.id,
            admin(),
            "cg113-promote",
            PromoteRequest {
                expected_head_decision_id: None,
                reason: "unsigned M18 selection".into(),
            },
        )
        .await
        .unwrap()
        .record;
    assert!(decision.governance.is_none());
    state.jobs.shutdown().await;
    let target = PromotionTarget {
        space_type: SPACE.into(),
        channel: "stable".into(),
    };
    (raw, state, target)
}

async fn flow() {
    let (source_raw, source, target) = legacy_store().await;
    let mut snapshot = source_raw.export_snapshot().await.unwrap();
    snapshot["collections"]["cg113_marker"] = json!({
        "type": "document",
        "documents": {"marker": {"_key": "marker", "value": 1}}
    });

    // The head of an unsigned chain says so; nothing signed selected it.
    let head = source
        .promotions
        .current(TENANT, INCARNATION, &target)
        .await
        .unwrap()
        .unwrap();
    assert!(head.unsigned_legacy);
    assert_eq!(head.public_value().unwrap()["unsigned_legacy"], json!(true));

    // A fresh store refuses the chain, and nothing of the snapshot lands.
    let fresh_raw: Arc<dyn GraphBackend> = Arc::new(NativeBackend::new());
    let fresh = AppState::new_shared(fresh_raw.clone());
    refused(
        fresh
            .promotions
            .import_snapshot(TENANT, INCARNATION, &snapshot)
            .await,
        "evidence",
        "fresh store",
    );
    for collection in [
        EVIDENCE_COLLECTION,
        DECISIONS_COLLECTION,
        HEADS_COLLECTION,
        JOBS_COLLECTION,
        "facts",
        "cg113_marker",
    ] {
        assert_eq!(stored(&fresh_raw, collection).await, 0, "{collection}");
    }
    assert!(
        fresh
            .promotions
            .current(TENANT, INCARNATION, &target)
            .await
            .unwrap()
            .is_none()
    );

    // An unsigned decision alone is refused too: here the evidence is
    // dropped from the snapshot, so the decision is the first unsigned
    // record the preflight meets.
    let mut decision_only = snapshot.clone();
    decision_only["collections"]
        .as_object_mut()
        .unwrap()
        .remove(EVIDENCE_COLLECTION);
    refused(
        fresh
            .promotions
            .import_snapshot(TENANT, INCARNATION, &decision_only)
            .await,
        "decision",
        "decision without its evidence",
    );
    assert_eq!(stored(&fresh_raw, DECISIONS_COLLECTION).await, 0);

    // The store that already holds the records takes its own snapshot back:
    // nothing new is introduced.
    source
        .promotions
        .import_snapshot(TENANT, INCARNATION, &snapshot)
        .await
        .unwrap();

    // A snapshot without promotion records is unaffected by the rule.
    let mut plain = snapshot.clone();
    for collection in [EVIDENCE_COLLECTION, DECISIONS_COLLECTION, HEADS_COLLECTION] {
        plain["collections"]
            .as_object_mut()
            .unwrap()
            .remove(collection);
    }
    let plain_raw: Arc<dyn GraphBackend> = Arc::new(NativeBackend::new());
    let plain_state = AppState::new_shared(plain_raw.clone());
    plain_state
        .promotions
        .import_snapshot(TENANT, INCARNATION, &plain)
        .await
        .unwrap();
    assert_eq!(stored(&plain_raw, "cg113_marker").await, 1);

    // With legacy restore switched on by the operator the chain imports,
    // and the rebuilt head is still marked unsigned.
    let restored_raw: Arc<dyn GraphBackend> = Arc::new(NativeBackend::new());
    let restored = AppState::new_shared(restored_raw.clone());
    restored.promotions.configure_legacy_promotion_import(true);
    restored
        .promotions
        .import_snapshot(TENANT, INCARNATION, &snapshot)
        .await
        .unwrap();
    let restored_head = restored
        .promotions
        .current(TENANT, INCARNATION, &target)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(restored_head, head);
    assert!(restored_head.unsigned_legacy);

    // Switching it off again does not disturb what is stored, and still
    // refuses a different unsigned chain.
    restored.promotions.configure_legacy_promotion_import(false);
    restored
        .promotions
        .recover_tenant(TENANT, INCARNATION)
        .await
        .unwrap();
    restored
        .promotions
        .import_snapshot(TENANT, INCARNATION, &snapshot)
        .await
        .unwrap();

    // A head stored before the marker existed is rewritten by reconcile.
    let head_key = head.key.clone();
    let mut unmarked = head.clone();
    unmarked.unsigned_legacy = false;
    unmarked.projection_digest = record_digest(&unmarked, "projection_digest").unwrap();
    source_raw
        .replace_document(
            HEADS_COLLECTION,
            &head_key,
            serde_json::to_value(&unmarked).unwrap(),
        )
        .await
        .unwrap();
    let repaired = source
        .promotions
        .reconcile(TENANT, INCARNATION, &target, false)
        .await
        .unwrap();
    assert!(repaired.head_changed);
    assert_eq!(repaired.head.as_ref(), Some(&head));
}
