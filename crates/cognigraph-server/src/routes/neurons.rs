//! B5: validated neuron authoring and lifecycle. Neurons remain documents
//! in the `neurons` collection (readers like the graph-augmented route are
//! unchanged), but writes go through ontology validation — including the
//! accepted-hint/accepted-blocker conflict rule against the STORED set —
//! and lifecycle transitions are explicit endpoints, not raw status edits.
//!
//! The space type is read from the `space_types` collection (documents
//! keyed by space id, authored through the dedicated draft/accept flow or a
//! governed construction candidate rather than generic document mutation).

use axum::extract::{Extension, Path, Query, State};
use axum::routing::post;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{Value, json};

use cognigraph_auth::User;

use cognigraph_construct::{
    Chunk, GraduationReason, Neuron, NeuronSet, NeuronStatus, SpaceType, graduation_report,
    validate_neurons,
};
use cognigraph_core::{CogniGraphError, FieldPredicate, GraphBackend, PredicateOp};

use crate::error::AppError;
use crate::state::AppState;

const NEURONS: &str = "neurons";
const SPACE_TYPES: &str = "space_types";

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", post(create_neuron).get(list_neurons))
        .route("/graduation", axum::routing::get(graduation))
        .route(
            "/{key}/accept",
            post(|s, p, u, b| transition(s, p, u, b, NeuronStatus::Accepted)),
        )
        .route(
            "/{key}/reject",
            post(|s, p, u, b| transition(s, p, u, b, NeuronStatus::Rejected)),
        )
        .route(
            "/{key}/retire",
            post(|s, p, u, b| transition(s, p, u, b, NeuronStatus::Retired)),
        )
}

/// The authenticated actor, or "anonymous" when auth is disabled (the
/// middleware inserts `User` only when a token was validated). Recording
/// "anonymous" rather than omitting the field keeps the audit trail
/// honest about the mode it was written under.
pub(crate) fn actor(user: &Option<Extension<User>>) -> String {
    user.as_ref()
        .map(|Extension(user)| user.username.clone())
        .unwrap_or_else(|| "anonymous".into())
}

pub(crate) fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[derive(Deserialize)]
struct CreateNeuronRequest {
    space_type: String,
    #[serde(flatten)]
    neuron: Neuron,
}

/// Author a neuron: validated against its space type; always stored as
/// `proposed` regardless of the submitted status — acceptance is its own,
/// separately-validated transition.
async fn create_neuron(
    State(state): State<AppState>,
    user: Option<Extension<User>>,
    Json(req): Json<CreateNeuronRequest>,
) -> Result<Json<Value>, AppError> {
    let space = load_space(&*state.managed_backend, &req.space_type).await?;
    let mut neuron = req.neuron;
    neuron.status = NeuronStatus::Proposed;
    let set = NeuronSet {
        space_type: req.space_type.clone(),
        neurons: vec![neuron.clone()],
    };
    validate_neurons(&set, &space)
        .map_err(|e| AppError(CogniGraphError::ValidationError(e.to_string())))?;

    let proposed_by = actor(&user);
    let mut doc = serde_json::to_value(&neuron).map_err(CogniGraphError::from)?;
    if let Some(fields) = doc.as_object_mut() {
        fields.insert("_key".into(), json!(neuron.id));
        fields.insert("space_type".into(), json!(req.space_type));
        fields.insert("proposed_by".into(), json!(proposed_by));
        fields.insert("proposed_at".into(), json!(now_secs()));
    }
    let id = state.managed_backend.create_document(NEURONS, doc).await?;
    state.invalidate_search_results().await;
    Ok(Json(json!({
        "_key": id.key,
        "status": "proposed",
        "space_type": req.space_type,
        "proposed_by": proposed_by,
    })))
}

#[derive(Deserialize)]
struct ListQuery {
    space_type: Option<String>,
    status: Option<String>,
    /// Pagination for pilot-scale queues (thousands of proposals).
    limit: Option<usize>,
    offset: Option<usize>,
}

async fn list_neurons(
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Value>, AppError> {
    let mut predicates = Vec::new();
    if let Some(space_type) = &query.space_type {
        predicates.push(FieldPredicate {
            path: vec!["space_type".into()],
            op: PredicateOp::Eq,
            value: json!(space_type),
        });
    }
    if let Some(status) = &query.status {
        predicates.push(FieldPredicate {
            path: vec!["status".into()],
            op: PredicateOp::Eq,
            value: json!(status),
        });
    }
    let neurons = state
        .backend
        .list_documents_filtered(NEURONS, &predicates, None, query.limit, query.offset)
        .await
        .unwrap_or_default();
    Ok(Json(json!({
        "neurons": neurons,
        "count": neurons.len(),
        "limit": query.limit,
        "offset": query.offset,
    })))
}

#[derive(Deserialize, Default)]
struct TransitionRequest {
    /// Optional reviewer note, stored on the neuron with the attribution.
    note: Option<String>,
}

/// Lifecycle transition. Accepting re-validates the neuron in accepted
/// shape TOGETHER with the space's other accepted neurons, so the
/// hint/blocker same-triple conflict cannot be assembled one edit at a
/// time — the property raw document edits could not enforce. Every
/// transition stamps WHO reviewed and WHEN (QW1): the audit trail is the
/// product claim, not an optional extra.
async fn transition(
    State(state): State<AppState>,
    Path(key): Path<String>,
    user: Option<Extension<User>>,
    body: Option<Json<TransitionRequest>>,
    to: NeuronStatus,
) -> Result<Json<Value>, AppError> {
    let _guard = state.neuron_lifecycle.lock().await;
    let doc = state
        .backend
        .get_document(NEURONS, &key)
        .await?
        .ok_or_else(|| CogniGraphError::DocumentNotFound {
            collection: NEURONS.into(),
            key: key.clone(),
        })?;
    let space_id = doc
        .get("space_type")
        .and_then(Value::as_str)
        .ok_or_else(|| CogniGraphError::ValidationError("stored neuron lacks a space_type".into()))?
        .to_string();
    let neuron: Neuron = serde_json::from_value(doc).map_err(CogniGraphError::from)?;
    if to == NeuronStatus::Accepted {
        validate_acceptance(&*state.managed_backend, &space_id, &key, neuron).await?;
    }

    let status = serde_json::to_value(to).map_err(CogniGraphError::from)?;
    let reviewed_by = actor(&user);
    let mut merge = json!({
        "status": status,
        "reviewed_by": reviewed_by,
        "reviewed_at": now_secs(),
    });
    if let Some(note) = body.and_then(|Json(b)| b.note).filter(|n| !n.is_empty()) {
        merge["review_note"] = json!(note);
    }
    state
        .managed_backend
        .update_document(NEURONS, &key, merge)
        .await?;
    state.invalidate_search_results().await;
    Ok(Json(json!({
        "_key": key,
        "status": status,
        "space_type": space_id,
        "reviewed_by": reviewed_by,
    })))
}

#[derive(Deserialize)]
struct GraduationQuery {
    space_type: String,
}

/// B6: leave-one-out graduation candidates over the space's ACCEPTED
/// neurons, computed entirely from stored state (space type, neurons,
/// chunks). Flags only — retirement stays a human transition.
async fn graduation(
    State(state): State<AppState>,
    Query(query): Query<GraduationQuery>,
) -> Result<Json<Value>, AppError> {
    let space = load_space(&*state.managed_backend, &query.space_type).await?;
    let neuron_docs = state
        .backend
        .list_documents_filtered(
            NEURONS,
            &[FieldPredicate {
                path: vec!["space_type".into()],
                op: PredicateOp::Eq,
                value: json!(query.space_type),
            }],
            None,
            None,
            None,
        )
        .await
        .unwrap_or_default();
    // Attribution lives on the stored docs, not the Neuron struct — keep
    // it aside so graduation candidates carry their reviewer.
    let reviewers: std::collections::HashMap<String, Value> = neuron_docs
        .iter()
        .filter_map(|doc| {
            Some((
                doc.get("id")?.as_str()?.to_string(),
                json!({
                    "reviewed_by": doc.get("reviewed_by").cloned().unwrap_or(Value::Null),
                    "reviewed_at": doc.get("reviewed_at").cloned().unwrap_or(Value::Null),
                }),
            ))
        })
        .collect();
    let neurons: Vec<Neuron> = neuron_docs
        .into_iter()
        .filter_map(|doc| serde_json::from_value(doc).ok())
        .collect();
    let chunks: Vec<Chunk> = state
        .backend
        .list_documents_filtered(
            "chunks",
            &[FieldPredicate {
                path: vec!["space_id".into()],
                op: PredicateOp::Eq,
                value: json!(space.id),
            }],
            None,
            None,
            None,
        )
        .await
        .unwrap_or_default()
        .into_iter()
        .filter_map(|doc| {
            Some(Chunk {
                id: doc.get("_key")?.as_str()?.to_string(),
                title: doc
                    .get("title")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                text: doc.get("text")?.as_str()?.to_string(),
            })
        })
        .collect();

    let candidates: Vec<Value> = graduation_report(&space, &neurons, &chunks)
        .into_iter()
        .map(|c| {
            json!({
                "neuron_id": c.neuron_id,
                "fact": format!("{} --{}--> {}", c.fact.source, c.fact.relation, c.fact.target),
                "reason": match c.reason {
                    GraduationReason::CoveredByBase => "covered_by_base",
                    GraduationReason::CoveredByOthers => "covered_by_others",
                    GraduationReason::InertBlocker => "inert_blocker",
                },
                "review": reviewers.get(&c.neuron_id).cloned().unwrap_or(Value::Null),
            })
        })
        .collect();
    Ok(Json(json!({
        "space_type": query.space_type,
        "candidates": candidates,
        "count": candidates.len(),
        "note": "flags only — retire via POST /api/neurons/{key}/retire",
    })))
}

pub(crate) async fn load_space(
    backend: &dyn GraphBackend,
    space_id: &str,
) -> Result<SpaceType, AppError> {
    let doc = backend
        .get_document(SPACE_TYPES, space_id)
        .await?
        .ok_or_else(|| {
            AppError(CogniGraphError::ValidationError(format!(
                "space type `{space_id}` not found in `{SPACE_TYPES}` (author and accept a dedicated space-type draft, or use governed candidate construction)"
            )))
        })?;
    serde_json::from_value(doc).map_err(|e| {
        AppError(CogniGraphError::ValidationError(format!(
            "bad space type: {e}"
        )))
    })
}

pub(crate) async fn load_accepted(
    backend: &dyn GraphBackend,
    space_id: &str,
    exclude_key: &str,
) -> Result<Vec<Neuron>, AppError> {
    let predicates = [
        FieldPredicate {
            path: vec!["space_type".into()],
            op: PredicateOp::Eq,
            value: json!(space_id),
        },
        FieldPredicate {
            path: vec!["status".into()],
            op: PredicateOp::Eq,
            value: json!("accepted"),
        },
    ];
    let documents = match backend
        .list_documents_filtered(NEURONS, &predicates, None, None, None)
        .await
    {
        Ok(documents) => documents,
        Err(CogniGraphError::CollectionNotFound(name)) if name == NEURONS => Vec::new(),
        Err(error) => return Err(error.into()),
    };
    documents
        .into_iter()
        .filter(|doc| doc.get("_key").and_then(Value::as_str) != Some(exclude_key))
        .map(|doc| serde_json::from_value(doc).map_err(|error| AppError(error.into())))
        .collect()
}

/// Call while holding the tenant's neuron lifecycle lock, through the write.
/// Backend failures and malformed accepted rows must not hide conflicts.
pub(crate) async fn validate_acceptance(
    backend: &dyn GraphBackend,
    space_id: &str,
    key: &str,
    mut neuron: Neuron,
) -> Result<(), AppError> {
    let space = load_space(backend, space_id).await?;
    neuron.status = NeuronStatus::Accepted;
    let mut accepted = load_accepted(backend, space_id, key).await?;
    accepted.push(neuron);
    validate_neurons(
        &NeuronSet {
            space_type: space_id.to_string(),
            neurons: accepted,
        },
        &space,
    )
    .map_err(|error| AppError(CogniGraphError::ValidationError(error.to_string())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cognigraph_native::NativeBackend;

    async fn seeded_state() -> AppState {
        let backend = NativeBackend::new();
        backend
            .create_document(
                SPACE_TYPES,
                json!({
                    "_key": "pharma",
                    "id": "pharma",
                    "entities": [
                        {"name": "Meridian", "type": "org", "aliases": []},
                        {"name": "Compound X", "type": "compound", "aliases": []}
                    ],
                    "relation_rules": [
                        {"source": "Meridian", "relation": "SUPPLIES", "target": "Compound X",
                         "when_any": ["meridian supplies compound x"]}
                    ]
                }),
            )
            .await
            .unwrap();
        AppState::new(backend)
    }

    fn hint(id: &str) -> Value {
        json!({
            "space_type": "pharma",
            "id": id,
            "type": "relation_hint",
            "status": "accepted",  // submitted status is ignored
            "confidence": 0.9,
            "evidence": ["contract text"],
            "source": "Meridian",
            "relation": "SUPPLIES",
            "target": "Compound X",
            "triggers": ["meridian supplies compound x"]
        })
    }

    #[tokio::test]
    async fn create_validates_and_forces_proposed() {
        let state = seeded_state().await;
        let response = create_neuron(
            State(state.clone()),
            None,
            Json(serde_json::from_value(hint("supply-hint")).unwrap()),
        )
        .await
        .unwrap_or_else(|_| panic!("create failed"));
        assert_eq!(response.0["status"], "proposed");
        let stored = state
            .backend
            .get_document(NEURONS, "supply-hint")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stored["status"], "proposed", "submitted `accepted` ignored");

        // Ontology violations are rejected at write time.
        let mut bad = hint("ghost-hint");
        bad["source"] = json!("Ghost Corp");
        let result = create_neuron(
            State(state),
            None,
            Json(serde_json::from_value(bad).unwrap()),
        )
        .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn transitions_record_reviewer_attribution() {
        use cognigraph_auth::Role;
        let state = seeded_state().await;
        let response = create_neuron(
            State(state.clone()),
            None,
            Json(serde_json::from_value(hint("supply-hint")).unwrap()),
        )
        .await
        .unwrap_or_else(|_| panic!("create failed"));
        // Auth disabled: the trail records "anonymous", not nothing.
        assert_eq!(response.0["proposed_by"], "anonymous");

        let reviewer = User {
            key: "u1".into(),
            username: "vera".into(),
            role: Role::Editor,
            tenant: "default".into(),
        };
        let response = transition(
            State(state.clone()),
            Path("supply-hint".to_string()),
            Some(Extension(reviewer)),
            Some(Json(TransitionRequest {
                note: Some("verified against p.3".into()),
            })),
            NeuronStatus::Accepted,
        )
        .await
        .unwrap_or_else(|_| panic!("accept failed"));
        assert_eq!(response.0["reviewed_by"], "vera");

        let stored = state
            .backend
            .get_document(NEURONS, "supply-hint")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stored["proposed_by"], "anonymous");
        assert_eq!(stored["reviewed_by"], "vera");
        assert_eq!(stored["review_note"], "verified against p.3");
        assert!(stored["reviewed_at"].as_u64().unwrap() > 0);
        assert!(stored["proposed_at"].as_u64().unwrap() > 0);

        // A later transition re-stamps the reviewer but keeps provenance.
        let _ = transition(
            State(state.clone()),
            Path("supply-hint".to_string()),
            None,
            None,
            NeuronStatus::Retired,
        )
        .await
        .unwrap_or_else(|_| panic!("retire failed"));
        let stored = state
            .backend
            .get_document(NEURONS, "supply-hint")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stored["reviewed_by"], "anonymous");
        assert_eq!(stored["proposed_by"], "anonymous");
        assert_eq!(stored["review_note"], "verified against p.3");
    }

    #[tokio::test]
    async fn graduation_flags_base_covered_hint() {
        let state = seeded_state().await;
        // The chunk grounds the fact via the BASE rule, so an accepted hint
        // targeting the same triple is redundant.
        state
            .managed_backend
            .create_document(
                "chunks",
                json!({
                    "_key": "pharma-c1",
                    "space_id": "pharma",
                    "text": "Meridian supplies Compound X to regional pharmacies."
                }),
            )
            .await
            .unwrap();
        let _ = create_neuron(
            State(state.clone()),
            None,
            Json(serde_json::from_value(hint("supply-hint")).unwrap()),
        )
        .await
        .unwrap_or_else(|_| panic!("create failed"));
        let _ = transition(
            State(state.clone()),
            Path("supply-hint".to_string()),
            None,
            None,
            NeuronStatus::Accepted,
        )
        .await
        .unwrap_or_else(|_| panic!("accept failed"));

        let report = graduation(
            State(state),
            Query(GraduationQuery {
                space_type: "pharma".into(),
            }),
        )
        .await
        .unwrap_or_else(|_| panic!("graduation failed"));
        assert_eq!(report.0["count"], 1);
        assert_eq!(report.0["candidates"][0]["neuron_id"], "supply-hint");
        assert_eq!(report.0["candidates"][0]["reason"], "covered_by_base");
    }

    #[tokio::test]
    async fn accept_enforces_hint_blocker_conflict_across_stored_set() {
        let state = seeded_state().await;
        let _ = create_neuron(
            State(state.clone()),
            None,
            Json(serde_json::from_value(hint("supply-hint")).unwrap()),
        )
        .await
        .unwrap_or_else(|_| panic!("create hint failed"));
        let _ = transition(
            State(state.clone()),
            Path("supply-hint".to_string()),
            None,
            None,
            NeuronStatus::Accepted,
        )
        .await
        .unwrap_or_else(|_| panic!("accept hint failed"));

        // A blocker on the same triple can be PROPOSED...
        let blocker = json!({
            "space_type": "pharma",
            "id": "supply-blocker",
            "type": "relation_blocker",
            "confidence": 0.9,
            "evidence": ["allegation text"],
            "source": "Meridian",
            "relation": "SUPPLIES",
            "target": "Compound X",
            "when_any": ["under investigation"]
        });
        let _ = create_neuron(
            State(state.clone()),
            None,
            Json(serde_json::from_value(blocker).unwrap()),
        )
        .await
        .unwrap_or_else(|_| panic!("propose blocker failed"));

        // ...but ACCEPTING it against the stored accepted hint must fail —
        // the conflict cannot be assembled one edit at a time.
        let result = transition(
            State(state.clone()),
            Path("supply-blocker".to_string()),
            None,
            None,
            NeuronStatus::Accepted,
        )
        .await;
        assert!(result.is_err(), "hint/blocker conflict must be enforced");

        // Retiring the hint clears the way.
        let _ = transition(
            State(state.clone()),
            Path("supply-hint".to_string()),
            None,
            None,
            NeuronStatus::Retired,
        )
        .await
        .unwrap_or_else(|_| panic!("retire failed"));
        let _ = transition(
            State(state),
            Path("supply-blocker".to_string()),
            None,
            None,
            NeuronStatus::Accepted,
        )
        .await
        .unwrap_or_else(|_| panic!("accept after retire failed"));
    }
}
