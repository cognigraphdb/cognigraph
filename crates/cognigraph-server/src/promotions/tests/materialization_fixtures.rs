//! Materialization fixtures.

use super::*;

pub(super) async fn stored_construction_candidate(
    state: &AppState,
    cas_root: &Path,
    context: &PromotionContext,
) -> ConstructionCandidateArtifact {
    let graph_binding = &context.artifact_attestations.as_ref().unwrap().graph;
    let graph = state
        .promotions
        .get_artifact_attestation(TENANT, INCARNATION, &graph_binding.attestation_id)
        .await
        .unwrap();
    let entry = graph
        .manifest
        .entries
        .iter()
        .find(|entry| entry.logical_path == CANDIDATE_ENTRYPOINT)
        .unwrap();
    let bytes = fs::read(staged_blob_path(cas_root, &entry.blob_digest)).unwrap();
    assert_eq!(digest_bytes(&bytes), entry.blob_digest);
    let candidate: ConstructionCandidateArtifact = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        canonical_digest(&candidate).unwrap(),
        context.candidate.candidate_digest
    );
    candidate
}
pub(super) async fn rows_for_space(
    backend: &dyn GraphBackend,
    collection: &str,
    space_id: &str,
) -> Vec<Value> {
    let mut rows = backend
        .list_documents(collection, None, None)
        .await
        .unwrap()
        .into_iter()
        .filter(|row| row.get("space_id").and_then(Value::as_str) == Some(space_id))
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| left["_key"].as_str().cmp(&right["_key"].as_str()));
    rows
}
pub(super) async fn assert_materialized_space_rows<T: Serialize>(
    backend: &dyn GraphBackend,
    collection: &str,
    space_id: &str,
    expected: &[T],
) {
    let mut actual = rows_for_space(backend, collection, space_id).await;
    for row in &mut actual {
        if let Some(fields) = row.as_object_mut() {
            fields.remove("_id");
            fields.remove("_rev");
            fields.remove("created_at");
            fields.remove("updated_at");
            if matches!(collection, "mentions" | "facts") {
                fields.remove("confidence");
            }
        }
    }
    let mut expected = expected
        .iter()
        .map(|row| serde_json::to_value(row).unwrap())
        .collect::<Vec<_>>();
    expected.sort_by(|left, right| left["_key"].as_str().cmp(&right["_key"].as_str()));
    assert_eq!(
        actual, expected,
        "`{collection}` does not exactly match the selected M26 projection"
    );
}
#[derive(Debug, PartialEq)]
pub(super) struct M26BuildObservedState {
    pub(super) generations: Vec<Value>,
    pub(super) authority: BTreeMap<String, Vec<Value>>,
    pub(super) graph: BTreeMap<String, Vec<Value>>,
}
#[derive(Debug, PartialEq)]
pub(super) struct M26DeploymentObservedState {
    pub(super) decisions: Vec<Value>,
    pub(super) heads: Vec<Value>,
    pub(super) graph: BTreeMap<String, Vec<Value>>,
}
pub(super) async fn sorted_collection_rows(
    backend: &dyn GraphBackend,
    collection: &str,
) -> Vec<Value> {
    let mut rows = match backend.list_documents(collection, None, None).await {
        Ok(rows) => rows,
        Err(CogniGraphError::CollectionNotFound(_)) => Vec::new(),
        Err(error) => panic!("cannot inspect `{collection}` while testing M26: {error}"),
    };
    rows.sort_by(|left, right| left["_key"].as_str().cmp(&right["_key"].as_str()));
    rows
}
pub(super) async fn observe_m26_build_state(backend: &dyn GraphBackend) -> M26BuildObservedState {
    let generations = sorted_collection_rows(backend, SEMANTIC_REPAIR_GENERATIONS_COLLECTION).await;
    let mut authority = BTreeMap::new();
    for collection in [
        EVIDENCE_COLLECTION,
        DECISIONS_COLLECTION,
        HEADS_COLLECTION,
        SEMANTIC_REPAIR_REVISIONS_COLLECTION,
        SEMANTIC_REPAIR_REVIEWS_COLLECTION,
    ] {
        authority.insert(
            collection.into(),
            sorted_collection_rows(backend, collection).await,
        );
    }
    let mut graph = BTreeMap::new();
    for collection in ["entities", "chunks", "mentions", "facts"] {
        graph.insert(
            collection.into(),
            sorted_collection_rows(backend, collection).await,
        );
    }
    M26BuildObservedState {
        generations,
        authority,
        graph,
    }
}
pub(super) async fn observe_m26_deployment_state(
    backend: &dyn GraphBackend,
) -> M26DeploymentObservedState {
    let decisions =
        sorted_collection_rows(backend, SEMANTIC_REPAIR_DEPLOYMENT_DECISIONS_COLLECTION).await;
    let heads = sorted_collection_rows(backend, SEMANTIC_REPAIR_DEPLOYMENT_HEADS_COLLECTION).await;
    let mut graph = BTreeMap::new();
    for collection in ["entities", "chunks", "mentions", "facts"] {
        graph.insert(
            collection.into(),
            sorted_collection_rows(backend, collection).await,
        );
    }
    M26DeploymentObservedState {
        decisions,
        heads,
        graph,
    }
}
pub(super) async fn rejected_m26_deployment_preserves_state(
    manager: &PromotionManager,
    backend: &dyn GraphBackend,
    actor: GovernanceActor,
    idempotency_key: &str,
    generation_id: &str,
    authorization: SemanticRepairDeploymentIntentSubmission,
) -> CogniGraphError {
    let before = observe_m26_deployment_state(backend).await;
    let error = manager
        .deploy_semantic_repair_generation(
            TENANT,
            INCARNATION,
            actor,
            idempotency_key,
            generation_id,
            authorization,
        )
        .await
        .expect_err("invalid M26 deployment was admitted");
    let after = observe_m26_deployment_state(backend).await;
    assert_eq!(
        after, before,
        "failed M26 deployment `{idempotency_key}` mutated target rows, decisions, or heads"
    );
    error
}
pub(super) fn resign_semantic_repair_deployment_intent(
    principal: &TestGovernancePrincipal,
    authorization: &mut SemanticRepairDeploymentIntentSubmission,
) {
    authorization.promoter_signature = principal
        .signing_key
        .sign(&authorization.statement)
        .unwrap();
}
pub(super) fn unrelated_record_id(label: &str) -> String {
    digest(label).trim_start_matches("sha256:").into()
}
pub(super) async fn rejected_m26_build_preserves_state(
    manager: &PromotionManager,
    cas: &LocalArtifactCas,
    backend: &dyn GraphBackend,
    actor: GovernanceActor,
    idempotency_key: &str,
    request: BuildSemanticRepairGenerationRequest,
) -> CogniGraphError {
    let before = observe_m26_build_state(backend).await;
    let error = manager
        .build_semantic_repair_generation(cas, TENANT, INCARNATION, actor, idempotency_key, request)
        .await
        .expect_err("invalid M26 build source was admitted");
    let after = observe_m26_build_state(backend).await;
    assert_eq!(
        after, before,
        "failed M26 build `{idempotency_key}` mutated generation authority or graph rows"
    );
    error
}
