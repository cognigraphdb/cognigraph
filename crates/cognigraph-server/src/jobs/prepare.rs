//! Prepare.

use super::*;

pub(super) async fn prepare_payload(
    state: &AppState,
    kind: JobKind,
    input: &Value,
    batch_size: usize,
) -> Result<JobPayload, CogniGraphError> {
    match kind {
        JobKind::ConstructIngest => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Input {
                space_type: String,
                chunks: Vec<Chunk>,
            }
            let input: Input = serde_json::from_value(input.clone()).map_err(|error| {
                CogniGraphError::ValidationError(format!("invalid construct.ingest input: {error}"))
            })?;
            if input.chunks.is_empty() {
                return Err(CogniGraphError::ValidationError("chunks is empty".into()));
            }
            if input.chunks.len() > MAX_INGEST_CHUNKS {
                return Err(CogniGraphError::ValidationError(format!(
                    "construct.ingest accepts at most {MAX_INGEST_CHUNKS} chunks"
                )));
            }
            let space =
                crate::routes::neurons::load_space(&*state.managed_backend, &input.space_type)
                    .await
                    .map_err(|error| error.0)?;
            let accepted = crate::routes::neurons::load_accepted(
                &*state.managed_backend,
                &input.space_type,
                "",
            )
            .await
            .map_err(|error| error.0)?;
            Ok(JobPayload::Ingest {
                space_type: space,
                accepted_neurons: accepted,
                chunks: input.chunks,
                batch_size: batch_size.max(1),
            })
        }
        JobKind::ConstructEvaluate => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Input {
                space_type: String,
                eval: Option<EvalSpec>,
                promotion_context: Option<crate::promotions::PromotionContext>,
            }
            let raw_eval = input.get("eval").filter(|value| !value.is_null()).cloned();
            let mut input: Input = serde_json::from_value(input.clone()).map_err(|error| {
                CogniGraphError::ValidationError(format!(
                    "invalid construct.evaluate input: {error}"
                ))
            })?;
            if input.promotion_context.as_ref().is_some_and(|context| {
                matches!(
                    context.schema_version,
                    crate::promotions::M21_PROMOTION_CONTEXT_SCHEMA_VERSION
                        | crate::promotions::M22_PROMOTION_CONTEXT_SCHEMA_VERSION
                        | crate::promotions::M23_PROMOTION_CONTEXT_SCHEMA_VERSION
                )
            }) && let Some(raw_eval) = raw_eval
            {
                input.eval = Some(
                    crate::artifact_consumption::parse_strict_eval_spec(raw_eval).map_err(
                        |error| {
                            CogniGraphError::ValidationError(format!(
                                "invalid verified-artifact construct.evaluate eval: {error}"
                            ))
                        },
                    )?,
                );
            }
            let eval = crate::routes::construct::resolve_spec(state, &input.space_type, input.eval)
                .await
                .map_err(|error| error.0)?;
            if eval.space_id != input.space_type {
                return Err(CogniGraphError::ValidationError(
                    "construct.evaluate space_type must match eval.space_id".into(),
                ));
            }
            if let Some(context) = &input.promotion_context {
                let (expected, forbidden) = validate_promotion_eval_spec(&eval)?;
                let eval_digest = crate::promotions::canonical_digest(&eval)?;
                context.validate(&input.space_type, &eval_digest, expected, forbidden)?;
                context.validate_runtime_backend(state.managed_backend.backend_name())?;
                if matches!(
                    context.schema_version,
                    crate::promotions::M21_PROMOTION_CONTEXT_SCHEMA_VERSION
                        | crate::promotions::M22_PROMOTION_CONTEXT_SCHEMA_VERSION
                        | crate::promotions::M23_PROMOTION_CONTEXT_SCHEMA_VERSION
                ) && (state.artifact_cas.is_none() || state.artifact_executable_digest.is_none())
                {
                    return Err(CogniGraphError::ConnectionError(
                        "verified artifact consumption is disabled; configure a local CAS".into(),
                    ));
                }
                let tenant = crate::tenancy::current_tenant();
                let incarnation = JobManager::tenant_incarnation(state, &tenant).await?;
                if let Some(binding) = &context.governance {
                    state
                        .promotions
                        .validate_policy_binding_active(
                            &tenant,
                            &incarnation,
                            &context.target,
                            &context.policy,
                            binding,
                        )
                        .await?;
                }
                if let Some(artifacts) = &context.artifact_attestations {
                    state
                        .promotions
                        .validate_artifact_binding_set_active(
                            &tenant,
                            &incarnation,
                            context,
                            artifacts,
                            crate::promotions::now_millis(),
                        )
                        .await?;
                }
            }
            Ok(JobPayload::Evaluate {
                eval,
                promotion_context: input.promotion_context.map(Box::new),
            })
        }
        JobKind::ConstructDraft => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Input {
                space_type: String,
                chunks: Vec<Chunk>,
                sample_cap: Option<usize>,
            }
            let input: Input = serde_json::from_value(input.clone()).map_err(|error| {
                CogniGraphError::ValidationError(format!("invalid construct.draft input: {error}"))
            })?;
            if input.chunks.is_empty() {
                return Err(CogniGraphError::ValidationError("chunks is empty".into()));
            }
            // D3, verbatim from the synchronous route: the drafter creates NEW
            // spaces only. Refused here, at submission, so an ineligible corpus
            // never becomes a queued job.
            if state
                .managed_backend
                .get_document(SPACE_TYPES, &input.space_type)
                .await?
                .is_some()
            {
                return Err(CogniGraphError::ValidationError(format!(
                    "space `{}` already exists in `{SPACE_TYPES}` — the drafter creates NEW \
                     spaces only (D3); extend accepted ontologies via versioned edits and neurons",
                    input.space_type
                )));
            }
            if state.completion.is_none() {
                return Err(CogniGraphError::ValidationError(
                    "No completion provider configured. Set OPENAI_API_KEY or GEMINI_API_KEY \
                     (model via COGNIGRAPH_COMPLETION_MODEL)."
                        .into(),
                ));
            }
            // The SAME grouping the synchronous route applies, from the one
            // implementation, so async and sync drafting see identical documents.
            let documents = crate::routes::construct::group_chunks_by_title(&input.chunks);
            if documents.len() > MAX_DRAFT_DOCUMENTS {
                return Err(CogniGraphError::ValidationError(format!(
                    "construct.draft accepts at most {MAX_DRAFT_DOCUMENTS} documents; this \
                     corpus groups into {} by title — split the run",
                    documents.len()
                )));
            }
            Ok(JobPayload::Draft {
                space_type: input.space_type,
                documents,
                sample_cap: input.sample_cap.unwrap_or(DEFAULT_DRAFT_SAMPLE_CAP),
                batch_size: batch_size.max(1),
            })
        }
        JobKind::SideviewsGenerate => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Input {
                collection: String,
                text_field: Option<String>,
                count: Option<usize>,
                regenerate: Option<bool>,
            }
            let input: Input = serde_json::from_value(input.clone()).map_err(|error| {
                CogniGraphError::ValidationError(format!(
                    "invalid sideviews.generate input: {error}"
                ))
            })?;
            // Side-views are generated FROM an ordinary source collection only.
            // The governed authority/derived collections, control storage, and
            // the side_views collection itself are all ineligible sources — the
            // last would recursively expand generated Q&A into more Q&A.
            if is_system_collection(&input.collection)
                || is_managed_collection(&input.collection)
                || is_generated_collection(&input.collection)
            {
                return Err(CogniGraphError::ValidationError(format!(
                    "collection `{}` cannot be a side-view source: system, governed, and \
                     machine-generated collections are ineligible",
                    input.collection
                )));
            }
            if state.sideviews_completion.is_none() || state.embedder.is_none() {
                return Err(CogniGraphError::ValidationError(
                    "side-view generation requires a configured side-view completion provider \
                     and embedding provider"
                        .into(),
                ));
            }
            crate::side_views::require_atomic_publication(state.managed_backend.as_ref())?;
            crate::side_views::require_source_identity(&cognigraph_core::DocumentId::new(
                &input.collection,
                "",
            ))?;
            let count = input.count.unwrap_or(12).clamp(1, 50);
            let text_field = input
                .text_field
                .filter(|field| !field.trim().is_empty())
                .unwrap_or_else(|| "text".into());
            // Freeze the source key set at submission (like Ingest freezes its
            // chunks) so the job is a durable, resumable per-document unit.
            let keys: Vec<String> = state
                .managed_backend
                .list_documents(&input.collection, None, None)
                .await?
                .into_iter()
                .filter_map(|doc| doc.get("_key").and_then(Value::as_str).map(str::to_string))
                .collect();
            if keys.len() > MAX_SIDEVIEW_DOCS {
                return Err(CogniGraphError::ValidationError(format!(
                    "side-view generation accepts at most {MAX_SIDEVIEW_DOCS} source documents; \
                     `{}` has {} — narrow the source collection or split the run",
                    input.collection,
                    keys.len()
                )));
            }
            for key in &keys {
                crate::side_views::require_source_identity(&cognigraph_core::DocumentId::new(
                    &input.collection,
                    key,
                ))?;
            }
            Ok(JobPayload::SideviewsGenerate {
                collection: input.collection,
                text_field,
                count,
                keys,
                batch_size: batch_size.max(1),
                regenerate: input.regenerate.unwrap_or(false),
            })
        }
    }
}
