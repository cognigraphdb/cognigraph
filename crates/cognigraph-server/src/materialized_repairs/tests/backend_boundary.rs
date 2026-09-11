//! Backend boundary.

use super::*;

#[tokio::test]
async fn arango_build_fails_at_atomic_capability_before_cas_or_backend_access() {
    let reserved =
        std::net::TcpListener::bind("127.0.0.1:0").expect("reserve an unreachable Arango endpoint");
    let endpoint = format!(
        "http://{}",
        reserved.local_addr().expect("read reserved endpoint")
    );
    drop(reserved);

    let backend: Arc<dyn cognigraph_core::GraphBackend> = Arc::new(ArangoBackend::connect(
        endpoint,
        "unreachable-m26",
        "nobody",
        "not-a-secret",
    ));
    assert_eq!(backend.backend_name(), "arango");
    assert!(!backend.supports_atomic_batches());
    let jobs = Arc::new(JobManager::new(backend.clone(), 1));
    let manager = PromotionManager::new(backend, jobs);
    let cas_root = TestCasRoot::new();
    let cas = LocalArtifactCas::open(&cas_root.0, 1).expect("open empty read-only CAS");

    let result = tokio::time::timeout(
        Duration::from_secs(1),
        Box::pin(manager.build_semantic_repair_generation(
            &cas,
            "tenant",
            "incarnation",
            GovernanceActor {
                user_key: "promoter-user".into(),
                username: "promoter@example.test".into(),
                role: Role::Promoter,
            },
            "m26-arango-capability",
            BuildSemanticRepairGenerationRequest {
                target: PromotionTarget {
                    space_type: "pharma".into(),
                    channel: "stable".into(),
                },
                expected_promotion_head_decision_id: "a".repeat(64),
            },
        )),
    )
    .await
    .expect("M26 capability rejection must not wait for Arango")
    .expect_err("non-atomic Arango must reject M26 generation builds");
    assert!(matches!(
        result,
        CogniGraphError::ConnectionError(message)
            if message == "M26 verified Semantic Repair materialization requires atomic batch support; backend `arango` is unsupported"
    ));
    assert_eq!(
        fs::read_dir(cas_root.0.join("tenants"))
            .expect("inspect CAS after rejected build")
            .count(),
        0,
        "capability rejection must not stage or mutate CAS content"
    );
}
#[test]
fn non_atomic_repository_probe_requires_all_protected_collections_to_be_absent() {
    assert_eq!(M26_PROTECTED_COLLECTIONS.len(), 3);
    for collection in M26_PROTECTED_COLLECTIONS {
        validate_non_atomic_repository_probe(
            "arango",
            collection,
            Err(CogniGraphError::CollectionNotFound("missing".into())),
        )
        .unwrap();

        for present in [Vec::new(), vec![json!({"_key": "authority"})]] {
            assert!(matches!(
                validate_non_atomic_repository_probe("arango", collection, Ok(present)),
                Err(CogniGraphError::ConnectionError(message))
                    if message.contains(collection) && message.contains("arango")
            ));
        }
    }

    for collection in M26_PROTECTED_COLLECTIONS {
        assert!(matches!(
            validate_non_atomic_repository_probe(
                "arango",
                collection,
                Err(CogniGraphError::BackendError("probe failed".into())),
            ),
            Err(CogniGraphError::BackendError(message)) if message == "probe failed"
        ));
    }
}
#[test]
fn deployment_nullable_fields_are_required_and_unknown_fields_fail() {
    let valid = deployment_payload_value();
    let parsed: SemanticRepairDeploymentIntentPayload =
        serde_json::from_value(valid.clone()).unwrap();
    assert!(parsed.expected_deployment_head_decision_id.is_none());
    assert!(parsed.rollback_target_generation_id.is_none());

    for field in [
        "expected_deployment_head_decision_id",
        "rollback_target_generation_id",
    ] {
        let mut omitted = valid.clone();
        omitted.as_object_mut().unwrap().remove(field);
        assert!(
            serde_json::from_value::<SemanticRepairDeploymentIntentPayload>(omitted).is_err(),
            "omitting required nullable `{field}` must fail"
        );
    }

    let mut populated = valid.clone();
    populated["expected_deployment_head_decision_id"] = json!("e".repeat(64));
    populated["rollback_target_generation_id"] = json!("f".repeat(64));
    let populated: SemanticRepairDeploymentIntentPayload =
        serde_json::from_value(populated).unwrap();
    assert!(populated.expected_deployment_head_decision_id.is_some());
    assert!(populated.rollback_target_generation_id.is_some());

    let mut unknown = valid;
    unknown["private_key"] = json!("must-never-be-accepted");
    assert!(serde_json::from_value::<SemanticRepairDeploymentIntentPayload>(unknown).is_err());
}
