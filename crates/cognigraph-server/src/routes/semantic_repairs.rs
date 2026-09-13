//! Tenant-scoped M25 signed Semantic Repair authority API.

use axum::extract::{DefaultBodyLimit, Extension, Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use cognigraph_auth::{Role, User};
use cognigraph_core::CogniGraphError;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::error::AppError;
use crate::governance::GovernanceActor;
use crate::jobs::JobManager;
use crate::promotions::PromotionTarget;
use crate::semantic_repairs::{
    CreateSemanticRepairRevisionRequest, MAX_SEMANTIC_REPAIR_REVISION_PAGE_SIZE,
    ReviewSemanticRepairRevisionRequest, SemanticRepairReviewRecord, SemanticRepairRevisionRecord,
};
use crate::state::AppState;
use crate::tenancy::current_tenant;

/// The embedded unchanged M22 candidate is capped at 8 MiB. Reserve 2 MiB
/// for the signed statement envelope and reject larger transport bodies
/// before JSON deserialization.
const MAX_SEMANTIC_REPAIR_REVISION_REQUEST_BYTES: usize = 10 * 1024 * 1024;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/revisions",
            get(list_revisions)
                .post(create_revision)
                .layer(DefaultBodyLimit::max(
                    MAX_SEMANTIC_REPAIR_REVISION_REQUEST_BYTES,
                )),
        )
        .route("/revisions/{id}", get(get_revision))
        .route("/revisions/{id}/review", post(review_revision))
        .route("/reviews", get(list_reviews))
        .route("/reviews/{id}", get(get_review))
        .route("/current/{space_type}/{channel}", get(current))
        .merge(crate::routes::materialized_repairs::router())
}

fn idempotency_key(headers: &HeaderMap) -> Result<&str, AppError> {
    headers
        .get("Idempotency-Key")
        .ok_or_else(|| {
            AppError(CogniGraphError::ValidationError(
                "missing Idempotency-Key header".into(),
            ))
        })?
        .to_str()
        .map_err(|_| {
            AppError(CogniGraphError::ValidationError(
                "Idempotency-Key must be printable ASCII".into(),
            ))
        })
}

fn require_review_revision_path(path_id: &str, signed_id: &str) -> Result<(), AppError> {
    if path_id != signed_id {
        return Err(AppError(CogniGraphError::ValidationError(
            "semantic repair revision path id does not match the signed review statement".into(),
        )));
    }
    Ok(())
}

async fn scope(state: &AppState) -> Result<(String, String), AppError> {
    let tenant = current_tenant();
    let incarnation = JobManager::tenant_incarnation(state, &tenant).await?;
    Ok((tenant, incarnation))
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct PageQuery {
    limit: Option<usize>,
    cursor: Option<String>,
}

fn bounded_page_limit(
    requested: Option<usize>,
    default: usize,
    maximum: usize,
    record_kind: &str,
) -> Result<usize, AppError> {
    let limit = requested.unwrap_or(default);
    if !(1..=maximum).contains(&limit) {
        return Err(AppError(CogniGraphError::ValidationError(format!(
            "semantic repair {record_kind} page limit must be between 1 and {maximum}"
        ))));
    }
    Ok(limit)
}

async fn create_revision(
    State(state): State<AppState>,
    user: Option<Extension<User>>,
    headers: HeaderMap,
    Json(request): Json<CreateSemanticRepairRevisionRequest>,
) -> Result<Response, AppError> {
    let actor =
        crate::routes::governance::require_live_role(&state, user, Role::PolicyAuthor).await?;
    let key = idempotency_key(&headers)?;
    let (tenant, incarnation) = scope(&state).await?;
    let mutation = state
        .promotions
        .create_semantic_repair_revision(
            &tenant,
            &incarnation,
            GovernanceActor::from_user(actor),
            key,
            request,
        )
        .await?;
    let id = mutation.record.semantic_repair_revision_id.clone();
    let status = if mutation.replayed {
        StatusCode::OK
    } else {
        StatusCode::CREATED
    };
    let mut response = (
        status,
        Json(json!({
            "semantic_repair_revision": mutation.record.public_value()?,
            "replayed": mutation.replayed,
        })),
    )
        .into_response();
    response.headers_mut().insert(
        header::LOCATION,
        format!("/api/semantic-repairs/revisions/{id}")
            .parse()
            .expect("semantic repair revision location is valid"),
    );
    Ok(response)
}

async fn list_revisions(
    State(state): State<AppState>,
    user: Option<Extension<User>>,
    Query(query): Query<PageQuery>,
) -> Result<Json<Value>, AppError> {
    crate::routes::governance::require_any_reader(&state, user).await?;
    let (tenant, incarnation) = scope(&state).await?;
    let limit = bounded_page_limit(
        query.limit,
        MAX_SEMANTIC_REPAIR_REVISION_PAGE_SIZE,
        MAX_SEMANTIC_REPAIR_REVISION_PAGE_SIZE,
        "revision",
    )?;
    let page = state
        .promotions
        .list_semantic_repair_revisions(&tenant, &incarnation, limit, query.cursor.as_deref())
        .await?;
    let records = page
        .records
        .iter()
        .map(semantic_repair_revision_summary)
        .collect::<Vec<_>>();
    Ok(Json(json!({
        "semantic_repair_revisions": records,
        "count": records.len(),
        "next_cursor": page.next_cursor,
    })))
}

fn semantic_repair_revision_summary(record: &SemanticRepairRevisionRecord) -> Value {
    json!({
        "schema_version": record.schema_version,
        "digest_algorithm": record.digest_algorithm,
        "tenant": record.tenant,
        "tenant_incarnation": record.tenant_incarnation,
        "semantic_repair_revision_id": record.semantic_repair_revision_id,
        "target": record.target,
        "base_promotion_head_decision_id": record.base_promotion_head_decision_id,
        "candidate_id": record.candidate.id,
        "candidate_revision": record.candidate.revision,
        "candidate_digest": record.candidate_digest,
        "author_registration_id": record.author_registration_id,
        "author_registration_digest": record.author_registration_digest,
        "author_principal_id": record.author_principal_id,
        "signed_at_ms": record.signed_at_ms,
        "created_by": record.created_by,
        "created_at_ms": record.created_at_ms,
        "semantic_repair_revision_digest": record.semantic_repair_revision_digest,
    })
}

async fn get_revision(
    State(state): State<AppState>,
    user: Option<Extension<User>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, AppError> {
    crate::routes::governance::require_any_reader(&state, user).await?;
    let (tenant, incarnation) = scope(&state).await?;
    Ok(Json(
        state
            .promotions
            .get_semantic_repair_revision(&tenant, &incarnation, &id)
            .await?
            .public_value()?,
    ))
}

async fn review_revision(
    State(state): State<AppState>,
    user: Option<Extension<User>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(request): Json<ReviewSemanticRepairRevisionRequest>,
) -> Result<Response, AppError> {
    let actor =
        crate::routes::governance::require_live_role(&state, user, Role::PolicyApprover).await?;
    let key = idempotency_key(&headers)?;
    require_review_revision_path(&id, &request.statement.payload.semantic_repair_revision_id)?;
    let (tenant, incarnation) = scope(&state).await?;
    let mutation = state
        .promotions
        .review_semantic_repair_revision(
            &tenant,
            &incarnation,
            GovernanceActor::from_user(actor),
            key,
            request,
        )
        .await?;
    let review_id = mutation.record.semantic_repair_review_id.clone();
    let status = if mutation.replayed {
        StatusCode::OK
    } else {
        StatusCode::CREATED
    };
    let mut response = (
        status,
        Json(json!({
            "semantic_repair_review": mutation.record.public_value()?,
            "replayed": mutation.replayed,
        })),
    )
        .into_response();
    response.headers_mut().insert(
        header::LOCATION,
        format!("/api/semantic-repairs/reviews/{review_id}")
            .parse()
            .expect("semantic repair review location is valid"),
    );
    Ok(response)
}

async fn list_reviews(
    State(state): State<AppState>,
    user: Option<Extension<User>>,
    Query(query): Query<PageQuery>,
) -> Result<Json<Value>, AppError> {
    crate::routes::governance::require_any_reader(&state, user).await?;
    let (tenant, incarnation) = scope(&state).await?;
    let limit = bounded_page_limit(query.limit, 25, 100, "review")?;
    let page = state
        .promotions
        .list_semantic_repair_reviews(&tenant, &incarnation, limit, query.cursor.as_deref())
        .await?;
    let records = page
        .records
        .iter()
        .map(semantic_repair_review_summary)
        .collect::<Vec<_>>();
    Ok(Json(json!({
        "semantic_repair_reviews": records,
        "count": records.len(),
        "next_cursor": page.next_cursor,
    })))
}

fn semantic_repair_review_summary(record: &SemanticRepairReviewRecord) -> Value {
    json!({
        "schema_version": record.schema_version,
        "digest_algorithm": record.digest_algorithm,
        "tenant": record.tenant,
        "tenant_incarnation": record.tenant_incarnation,
        "semantic_repair_review_id": record.semantic_repair_review_id,
        "semantic_repair_revision_id": record.semantic_repair_revision_id,
        "semantic_repair_revision_digest": record.semantic_repair_revision_digest,
        "target": record.target,
        "base_promotion_head_decision_id": record.base_promotion_head_decision_id,
        "candidate_digest": record.candidate_digest,
        "author_principal_id": record.author_principal_id,
        "approver_registration_id": record.approver_registration_id,
        "approver_registration_digest": record.approver_registration_digest,
        "approver_principal_id": record.approver_principal_id,
        "decision": record.decision,
        "reason": record.reason,
        "signed_at_ms": record.signed_at_ms,
        "reviewed_by": record.reviewed_by,
        "reviewed_at_ms": record.reviewed_at_ms,
        "semantic_repair_review_digest": record.semantic_repair_review_digest,
    })
}

async fn get_review(
    State(state): State<AppState>,
    user: Option<Extension<User>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, AppError> {
    crate::routes::governance::require_any_reader(&state, user).await?;
    let (tenant, incarnation) = scope(&state).await?;
    Ok(Json(
        state
            .promotions
            .get_semantic_repair_review(&tenant, &incarnation, &id)
            .await?
            .public_value()?,
    ))
}

async fn current(
    State(state): State<AppState>,
    user: Option<Extension<User>>,
    Path((space_type, channel)): Path<(String, String)>,
) -> Result<Json<Value>, AppError> {
    crate::routes::governance::require_any_reader(&state, user).await?;
    let (tenant, incarnation) = scope(&state).await?;
    let authority = state
        .promotions
        .resolve_current_semantic_repair_authority(
            &tenant,
            &incarnation,
            &PromotionTarget {
                space_type,
                channel,
            },
        )
        .await?;
    Ok(Json(json!({
        "authority": authority.public_value()?,
    })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::response::IntoResponse;
    use cognigraph_auth::AuthProvider;
    use cognigraph_native::NativeBackend;
    use std::sync::Arc;

    fn digest(byte: char) -> String {
        format!("sha256:{}", byte.to_string().repeat(64))
    }

    fn governance_signature() -> Value {
        json!({
            "schema_version": 1,
            "algorithm": "ed25519",
            "key_id": format!("ed25519:sha256:{}", "a".repeat(64)),
            "statement_digest": digest('b'),
            "signature": "public-signature",
        })
    }

    fn actor(role: &str) -> Value {
        json!({
            "user_key": format!("{role}-user"),
            "username": role,
            "role": role,
        })
    }

    fn revision_record(description: String) -> SemanticRepairRevisionRecord {
        serde_json::from_value(json!({
            "_key": "internal-key",
            "schema_version": 1,
            "digest_algorithm": "cognigraph-canonical-json-nfc-v1+sha256",
            "tenant": "default",
            "tenant_incarnation": "incarnation-1",
            "semantic_repair_revision_id": "1".repeat(64),
            "target": { "space_type": "clinical", "channel": "production" },
            "base_promotion_head_decision_id": "9".repeat(64),
            "candidate": {
                "schema_version": 1,
                "kind": "semantic-neuron-bundle",
                "id": "candidate-1",
                "revision": "r1",
                "base_space_type": {
                    "id": "clinical",
                    "name": "Clinical",
                    "version": 1,
                    "description": description,
                    "entities": [],
                    "relation_rules": [],
                },
                "accepted_neurons": [],
            },
            "candidate_digest": digest('c'),
            "author_registration_id": "2".repeat(64),
            "author_registration_digest": digest('d'),
            "author_principal_id": "author-principal",
            "signed_at_ms": 1,
            "author_signature": governance_signature(),
            "created_by": actor("policy-author"),
            "created_at_ms": 2,
            "idempotency_key_hash": digest('e'),
            "request_digest": digest('f'),
            "semantic_repair_revision_digest": digest('1'),
        }))
        .unwrap()
    }

    fn review_record() -> SemanticRepairReviewRecord {
        serde_json::from_value(json!({
            "_key": "internal-review-key",
            "schema_version": 1,
            "digest_algorithm": "cognigraph-canonical-json-nfc-v1+sha256",
            "tenant": "default",
            "tenant_incarnation": "incarnation-1",
            "semantic_repair_review_id": "3".repeat(64),
            "semantic_repair_revision_id": "1".repeat(64),
            "semantic_repair_revision_digest": digest('1'),
            "target": { "space_type": "clinical", "channel": "production" },
            "base_promotion_head_decision_id": "9".repeat(64),
            "candidate_digest": digest('c'),
            "author_principal_id": "author-principal",
            "approver_registration_id": "4".repeat(64),
            "approver_registration_digest": digest('5'),
            "approver_principal_id": "approver-principal",
            "decision": "approve",
            "reason": "independently approved",
            "signed_at_ms": 3,
            "approver_signature": governance_signature(),
            "reviewed_by": actor("policy-approver"),
            "reviewed_at_ms": 4,
            "idempotency_key_hash": digest('6'),
            "request_digest": digest('7'),
            "semantic_repair_review_digest": digest('8'),
        }))
        .unwrap()
    }

    async fn state_with_roles() -> (AppState, User, User, User) {
        let backend = Arc::new(NativeBackend::new());
        let auth = Arc::new(AuthProvider::new(backend.clone()).await.unwrap());
        let author = auth
            .create_user("author", "password", Role::PolicyAuthor)
            .await
            .unwrap();
        let approver = auth
            .create_user("approver", "password", Role::PolicyApprover)
            .await
            .unwrap();
        let editor = auth
            .create_user("editor", "password", Role::Editor)
            .await
            .unwrap();
        let mut state = AppState::new_shared(backend);
        state.auth = Some(auth);
        (state, author, approver, editor)
    }

    #[tokio::test]
    async fn mutation_roles_are_exact_even_without_route_middleware() {
        let (state, author, approver, editor) = state_with_roles().await;
        let author_for_wrong_role = author.clone();
        let approver_for_wrong_role = approver.clone();
        assert!(
            crate::routes::governance::require_live_role(
                &state,
                Some(Extension(author)),
                Role::PolicyAuthor,
            )
            .await
            .is_ok()
        );
        assert!(
            crate::routes::governance::require_live_role(
                &state,
                Some(Extension(approver)),
                Role::PolicyApprover,
            )
            .await
            .is_ok()
        );
        assert_eq!(
            crate::routes::governance::require_live_role(
                &state,
                Some(Extension(author_for_wrong_role)),
                Role::PolicyApprover,
            )
            .await
            .unwrap_err()
            .into_response()
            .status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            crate::routes::governance::require_live_role(
                &state,
                Some(Extension(approver_for_wrong_role)),
                Role::PolicyAuthor,
            )
            .await
            .unwrap_err()
            .into_response()
            .status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            crate::routes::governance::require_live_role(
                &state,
                Some(Extension(editor)),
                Role::PolicyAuthor,
            )
            .await
            .unwrap_err()
            .into_response()
            .status(),
            StatusCode::FORBIDDEN
        );
        state.jobs.shutdown().await;
    }

    #[test]
    fn idempotency_header_is_mandatory_and_ascii() {
        assert!(idempotency_key(&HeaderMap::new()).is_err());
        let mut headers = HeaderMap::new();
        headers.insert("Idempotency-Key", "revision-r1".parse().unwrap());
        assert_eq!(idempotency_key(&headers).unwrap(), "revision-r1");
    }

    #[test]
    fn review_path_must_match_the_signed_revision_identity() {
        require_review_revision_path("revision-1", "revision-1").unwrap();
        assert!(require_review_revision_path("revision-1", "revision-2").is_err());
    }

    #[test]
    fn list_summaries_omit_large_candidates_signatures_and_request_internals() {
        let revision = revision_record("x".repeat(128 * 1024));
        let summary = semantic_repair_revision_summary(&revision);
        assert!(summary.get("candidate").is_none());
        assert!(summary.get("author_signature").is_none());
        assert!(summary.get("request_digest").is_none());
        assert!(summary.get("idempotency_key_hash").is_none());
        assert_eq!(summary["candidate_id"], "candidate-1");
        assert!(serde_json::to_vec(&summary).unwrap().len() < 4 * 1024);

        let review = semantic_repair_review_summary(&review_record());
        assert!(review.get("approver_signature").is_none());
        assert!(review.get("request_digest").is_none());
        assert!(review.get("idempotency_key_hash").is_none());
        assert_eq!(review["decision"], "approve");
        assert!(serde_json::to_vec(&review).unwrap().len() < 4 * 1024);
    }

    #[test]
    fn list_limits_reject_repository_amplification_before_loading_records() {
        assert_eq!(MAX_SEMANTIC_REPAIR_REVISION_REQUEST_BYTES, 10 * 1024 * 1024);
        assert_eq!(
            bounded_page_limit(
                None,
                MAX_SEMANTIC_REPAIR_REVISION_PAGE_SIZE,
                MAX_SEMANTIC_REPAIR_REVISION_PAGE_SIZE,
                "revision",
            )
            .unwrap(),
            8
        );
        assert_eq!(
            bounded_page_limit(
                Some(8),
                MAX_SEMANTIC_REPAIR_REVISION_PAGE_SIZE,
                MAX_SEMANTIC_REPAIR_REVISION_PAGE_SIZE,
                "revision",
            )
            .unwrap(),
            8
        );
        assert!(
            bounded_page_limit(
                Some(9),
                MAX_SEMANTIC_REPAIR_REVISION_PAGE_SIZE,
                MAX_SEMANTIC_REPAIR_REVISION_PAGE_SIZE,
                "revision",
            )
            .is_err()
        );
        assert!(bounded_page_limit(Some(0), 25, 100, "review").is_err());
        assert!(bounded_page_limit(Some(101), 25, 100, "review").is_err());
    }
}
