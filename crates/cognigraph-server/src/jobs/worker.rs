//! Worker.

use super::*;

impl JobManager {
    pub(super) async fn run_job(
        self: &Arc<Self>,
        runtime: &JobRuntime,
        tenant: &str,
        id: &str,
    ) -> Result<DispatchOutcome, CogniGraphError> {
        let mut job = {
            let _guard = self.transition_lock.lock().await;
            let Some(mut job) = self.get_raw(tenant, id).await? else {
                return Ok(DispatchOutcome::Complete);
            };
            if job.status.terminal() {
                return Ok(DispatchOutcome::Complete);
            }
            if job.status == JobStatus::CancelRequested {
                self.finish_canceled(tenant, &mut job, "canceled before the next operation")
                    .await?;
                return Ok(DispatchOutcome::Complete);
            }
            if self.shutting_down.load(Ordering::Acquire) || self.tenant_paused(tenant) {
                if job.status == JobStatus::Running {
                    self.requeue_interrupted(tenant, &mut job).await?;
                }
                return Ok(DispatchOutcome::Parked);
            }
            if !runtime_identity_active(runtime, &job).await? {
                if job.status == JobStatus::Running {
                    self.requeue_interrupted(tenant, &mut job).await?;
                }
                return Ok(DispatchOutcome::Parked);
            }
            if job.status == JobStatus::Queued {
                let from = job.status;
                job.status = JobStatus::Running;
                job.attempt = job.attempt.saturating_add(1);
                job.started_at_ms = Some(now_millis());
                job.finished_at_ms = None;
                job.updated_at_ms = now_millis();
                job.progress.phase = "running".into();
                job.push_event(
                    "started",
                    JobActor::worker(),
                    Some(from),
                    Some(JobStatus::Running),
                    None,
                );
                self.save_raw(tenant, &job).await?;
                self.metrics.started.fetch_add(1, Ordering::Relaxed);
            } else if job.status != JobStatus::Running {
                return Ok(DispatchOutcome::Complete);
            }
            job
        };

        #[cfg(test)]
        if self.panic_next_worker.swap(false, Ordering::AcqRel) {
            panic!("injected durable job worker panic after claim");
        }

        match job.payload.clone() {
            JobPayload::Ingest {
                space_type,
                accepted_neurons,
                chunks,
                batch_size,
            } => {
                let config = effective_config(&space_type, &accepted_neurons);
                let vetoes = effective_vetoes(&accepted_neurons);
                if job.progress.completed < chunks.len() {
                    if self.pause_or_cancel(runtime, &mut job).await? {
                        return Ok(DispatchOutcome::Parked);
                    }
                    let start = job.progress.completed;
                    let end = start.saturating_add(batch_size.max(1)).min(chunks.len());
                    let scoped = TenantScoped::new(tenant.into(), runtime.backend.clone());
                    let promotions = runtime.promotions.upgrade().ok_or_else(|| {
                        CogniGraphError::ConnectionError(
                            "promotion authority is unavailable during durable ingestion".into(),
                        )
                    })?;
                    let grounded = promotions
                        .ingest_unmaterialized_chunks(
                            &scoped,
                            (tenant, &job.tenant_incarnation),
                            &space_type.id,
                            &config,
                            &chunks[start..end],
                            &vetoes,
                        )
                        .await?;
                    CURRENT_TENANT
                        .scope(tenant.into(), runtime.invalidate_search_results())
                        .await;

                    let _guard = self.transition_lock.lock().await;
                    job = self.get_raw(tenant, id).await?.ok_or_else(|| {
                        CogniGraphError::DocumentNotFound {
                            collection: "jobs".into(),
                            key: id.into(),
                        }
                    })?;
                    job.progress.completed = end;
                    job.progress.facts_grounded =
                        job.progress.facts_grounded.saturating_add(grounded);
                    job.updated_at_ms = now_millis();
                    self.metrics.checkpoints.fetch_add(1, Ordering::Relaxed);
                    if job.status == JobStatus::CancelRequested && end < chunks.len() {
                        self.finish_canceled(tenant, &mut job, "canceled after durable checkpoint")
                            .await?;
                        return Ok(DispatchOutcome::Complete);
                    }
                    if self.shutting_down.load(Ordering::Acquire) || self.tenant_paused(tenant) {
                        self.requeue_interrupted(tenant, &mut job).await?;
                        return Ok(DispatchOutcome::Parked);
                    }
                    self.save_raw(tenant, &job).await?;
                    if end < chunks.len() {
                        return Ok(DispatchOutcome::Continue);
                    }
                }
                let _guard = self.transition_lock.lock().await;
                job = self.get_raw(tenant, id).await?.ok_or_else(|| {
                    CogniGraphError::DocumentNotFound {
                        collection: "jobs".into(),
                        key: id.into(),
                    }
                })?;
                let result = json!({
                    "space_type": space_type.id,
                    "chunks": chunks.len(),
                    "facts_grounded": job.progress.facts_grounded,
                    "accepted_neurons": accepted_neurons.len(),
                });
                self.finish_succeeded(tenant, &mut job, result).await?;
                Ok(DispatchOutcome::Complete)
            }
            JobPayload::Evaluate {
                eval,
                promotion_context,
            } => {
                if self.pause_or_cancel(runtime, &mut job).await? {
                    return Ok(DispatchOutcome::Parked);
                }
                let (mut result, consumption) = if let Some(context) =
                    promotion_context.as_deref().filter(|context| {
                        matches!(
                            context.schema_version,
                            crate::promotions::M21_PROMOTION_CONTEXT_SCHEMA_VERSION
                                | crate::promotions::M22_PROMOTION_CONTEXT_SCHEMA_VERSION
                                | crate::promotions::M23_PROMOTION_CONTEXT_SCHEMA_VERSION
                        )
                    }) {
                    let expected_job_schema = if context.schema_version
                        == crate::promotions::M23_PROMOTION_CONTEXT_SCHEMA_VERSION
                    {
                        M23_JOB_SCHEMA_VERSION
                    } else if context.schema_version
                        == crate::promotions::M22_PROMOTION_CONTEXT_SCHEMA_VERSION
                    {
                        M22_JOB_SCHEMA_VERSION
                    } else {
                        M21_JOB_SCHEMA_VERSION
                    };
                    if job.schema_version != expected_job_schema {
                        return Err(CogniGraphError::DocumentConflict(
                        "verified-artifact evaluation context requires its matching durable job generation".into(),
                    ));
                    }
                    let cas = runtime.artifact_cas.as_deref().ok_or_else(|| {
                        CogniGraphError::ConnectionError(
                            "verified artifact consumption is disabled".into(),
                        )
                    })?;
                    let pinned_executable_digest = runtime
                        .artifact_executable_digest
                        .as_deref()
                        .ok_or_else(|| {
                            CogniGraphError::ConnectionError(
                                "verified artifact executable identity is unavailable".into(),
                            )
                        })?;
                    let promotions = runtime.promotions.upgrade().ok_or_else(|| {
                        CogniGraphError::ConnectionError(
                            "promotion authority is unavailable during artifact consumption".into(),
                        )
                    })?;
                    let input_digest = if job.input_digest.starts_with("sha256:") {
                        job.input_digest.clone()
                    } else {
                        format!("sha256:{}", job.input_digest)
                    };
                    let execution_payload_digest =
                        crate::promotions::canonical_digest(&job.payload)?;
                    let eval_spec_digest = crate::promotions::canonical_digest(&eval)?;
                    let job_binding = crate::artifact_consumption::ConsumptionJobBinding {
                        tenant,
                        tenant_incarnation: &job.tenant_incarnation,
                        job_id: &job.id,
                        attempt: job.attempt,
                        recoveries: job.recoveries,
                        input_digest: &input_digest,
                        execution_payload_digest: &execution_payload_digest,
                        eval_spec_digest: &eval_spec_digest,
                    };
                    #[cfg(test)]
                    let consumption_delay_ms =
                        self.artifact_consumption_delay_ms.load(Ordering::Acquire);
                    #[cfg(test)]
                    self.artifact_consumption_started
                        .store(true, Ordering::Release);
                    let mut consumption = Box::pin(async move {
                        #[cfg(test)]
                        if consumption_delay_ms > 0 {
                            tokio::time::sleep(std::time::Duration::from_millis(
                                consumption_delay_ms,
                            ))
                            .await;
                        }
                        promotions
                            .consume_verified_evaluation_inputs(
                                cas,
                                pinned_executable_digest,
                                &job_binding,
                                &eval,
                                context,
                            )
                            .await
                    });
                    let deadline = tokio::time::sleep(std::time::Duration::from_secs(
                        M21_CONSUMPTION_TIMEOUT_SECS,
                    ));
                    tokio::pin!(deadline);
                    let mut cancellation_poll = tokio::time::interval(
                        std::time::Duration::from_millis(M21_CANCELLATION_POLL_MS),
                    );
                    cancellation_poll
                        .set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                    let consumed = loop {
                        tokio::select! {
                            biased;
                            _ = &mut deadline => {
                                return Err(CogniGraphError::CapacityExceeded(format!(
                                    "verified artifact consumption exceeded its fixed {M21_CONSUMPTION_TIMEOUT_SECS} second time budget"
                                )));
                            }
                            current = async {
                                cancellation_poll.tick().await;
                                self.get_raw(tenant, id).await
                            } => {
                                let cancellation_requested = current?
                                    .is_some_and(|current| current.status == JobStatus::CancelRequested);
                                if cancellation_requested
                                    || self.shutting_down.load(Ordering::Acquire)
                                    || self.tenant_paused(tenant)
                                {
                                    drop(consumption);
                                    if self.pause_or_cancel(runtime, &mut job).await? {
                                        return Ok(DispatchOutcome::Parked);
                                    }
                                    return Err(CogniGraphError::DocumentConflict(
                                        "artifact consumption interruption changed before it could be checkpointed".into(),
                                    ));
                                }
                            }
                            result = &mut consumption => break result?,
                        }
                    };
                    (consumed.result, Some(consumed.receipt))
                } else {
                    if job.schema_version != JOB_SCHEMA_VERSION {
                        return Err(CogniGraphError::DocumentConflict(
                            "legacy evaluation context requires job schema version 1".into(),
                        ));
                    }
                    let scoped = TenantScoped::new(tenant.into(), runtime.backend.clone());
                    let outcome = evaluate(&scoped, &eval.space_id, &eval).await?;
                    (
                        json!({
                            "space_type": eval.space_id,
                            "recall": { "found": outcome.expected_found, "total": outcome.expected_total },
                            "restraint": { "violations": outcome.forbidden_triggered, "total": outcome.forbidden_total },
                            "recall_ok": outcome.recall_ok(),
                            "restraint_ok": outcome.restraint_ok(),
                            "missing": fact_lines(&outcome.missing),
                            "violations": fact_lines(&outcome.violations),
                        }),
                        None,
                    )
                };
                if let Some(receipt) = consumption {
                    result
                        .as_object_mut()
                        .expect("evaluation result is an object")
                        .insert(
                            "artifact_consumption".into(),
                            serde_json::to_value(receipt)?,
                        );
                }
                let _guard = self.transition_lock.lock().await;
                job = self.get_raw(tenant, id).await?.ok_or_else(|| {
                    CogniGraphError::DocumentNotFound {
                        collection: "jobs".into(),
                        key: id.into(),
                    }
                })?;
                if job.status == JobStatus::CancelRequested {
                    self.finish_canceled(
                        tenant,
                        &mut job,
                        "canceled after evaluation before the result checkpoint",
                    )
                    .await?;
                    return Ok(DispatchOutcome::Complete);
                }
                self.finish_succeeded(tenant, &mut job, result).await?;
                Ok(DispatchOutcome::Complete)
            }
            JobPayload::SideviewsGenerate {
                collection,
                text_field,
                count,
                keys,
                batch_size,
                regenerate,
            } => {
                // Providers were required at submission (prepare_payload); a
                // configuration change between submit and run is a fail-closed
                // error, never a silent skip that would write nothing.
                let Some(sideviews_completion) = runtime.sideviews_completion.clone() else {
                    return Err(CogniGraphError::ConnectionError(
                        "side-view completion provider is unavailable during generation".into(),
                    ));
                };
                let Some(embedder) = runtime.embedder.clone() else {
                    return Err(CogniGraphError::ConnectionError(
                        "embedding provider is unavailable during side-view generation".into(),
                    ));
                };
                if job.progress.completed < keys.len() {
                    if self.pause_or_cancel(runtime, &mut job).await? {
                        return Ok(DispatchOutcome::Parked);
                    }
                    let start = job.progress.completed;
                    let end = start.saturating_add(batch_size.max(1)).min(keys.len());
                    // EVERY side-view read and write goes through the trusted
                    // managed backend re-entered into this tenant's scope — never
                    // the guarded public handle, and never the default tenant.
                    let scoped = TenantScoped::new(tenant.into(), runtime.backend.clone());
                    let mut written = 0usize;
                    for key in &keys[start..end] {
                        let Some(source) = runtime
                            .side_views
                            .prepare(
                                &scoped,
                                tenant,
                                &job.tenant_incarnation,
                                cognigraph_core::DocumentId::new(&collection, key),
                                regenerate,
                            )
                            .await?
                        else {
                            continue;
                        };
                        let document = &source.document;
                        let Some(text) = document
                            .get(text_field.as_str())
                            .and_then(Value::as_str)
                            .map(str::trim)
                            .filter(|text| !text.is_empty())
                        else {
                            continue;
                        };
                        let parent = format!("{collection}/{key}");
                        let pairs = generate_sideviews(&*sideviews_completion, text, count)
                            .await
                            .map_err(|error| {
                                CogniGraphError::BackendError(format!(
                                    "side-view generation for `{parent}`: {error}"
                                ))
                            })?;
                        let mut documents = Vec::new();
                        if !pairs.is_empty() {
                            let questions: Vec<&str> =
                                pairs.iter().map(|pair| pair.question.as_str()).collect();
                            let embeddings = embedder.embed(&questions).await.map_err(|error| {
                                CogniGraphError::BackendError(format!(
                                    "embedding side-views for `{parent}`: {error}"
                                ))
                            })?;
                            if embeddings.len() != pairs.len() {
                                return Err(CogniGraphError::BackendError(format!(
                                    "embedding provider returned {} vectors for {} side-view \
                                 questions on `{parent}`",
                                    embeddings.len(),
                                    pairs.len()
                                )));
                            }
                            for (QaPair { question, answer }, embedding) in
                                pairs.into_iter().zip(embeddings)
                            {
                                documents.push(json!({
                                    "kind": "side_view",
                                    "question": question,
                                    "answer": answer,
                                    "embedding": embedding,
                                }));
                            }
                        }
                        let inserts = runtime
                            .side_views
                            .publish(&scoped, source, documents, regenerate)
                            .await?;
                        written = written.saturating_add(inserts);
                        CURRENT_TENANT
                            .scope(tenant.into(), runtime.invalidate_search_results())
                            .await;
                    }
                    let _guard = self.transition_lock.lock().await;
                    job = self.get_raw(tenant, id).await?.ok_or_else(|| {
                        CogniGraphError::DocumentNotFound {
                            collection: "jobs".into(),
                            key: id.into(),
                        }
                    })?;
                    job.progress.completed = end;
                    job.progress.side_views_written =
                        job.progress.side_views_written.saturating_add(written);
                    job.updated_at_ms = now_millis();
                    self.metrics.checkpoints.fetch_add(1, Ordering::Relaxed);
                    if job.status == JobStatus::CancelRequested && end < keys.len() {
                        self.finish_canceled(tenant, &mut job, "canceled after durable checkpoint")
                            .await?;
                        return Ok(DispatchOutcome::Complete);
                    }
                    if self.shutting_down.load(Ordering::Acquire) || self.tenant_paused(tenant) {
                        self.requeue_interrupted(tenant, &mut job).await?;
                        return Ok(DispatchOutcome::Parked);
                    }
                    self.save_raw(tenant, &job).await?;
                    if end < keys.len() {
                        return Ok(DispatchOutcome::Continue);
                    }
                }
                let _guard = self.transition_lock.lock().await;
                job = self.get_raw(tenant, id).await?.ok_or_else(|| {
                    CogniGraphError::DocumentNotFound {
                        collection: "jobs".into(),
                        key: id.into(),
                    }
                })?;
                let result = json!({
                    "collection": collection,
                    "documents": keys.len(),
                    "side_views_written": job.progress.side_views_written,
                });
                self.finish_succeeded(tenant, &mut job, result).await?;
                Ok(DispatchOutcome::Complete)
            }
            JobPayload::Draft {
                space_type,
                documents,
                sample_cap,
                batch_size,
            } => {
                // The provider was required at submission (prepare_payload); a
                // configuration change between submit and run is a fail-closed
                // error, never a silently empty ontology.
                let Some(completion) = runtime.completion.clone() else {
                    return Err(CogniGraphError::ConnectionError(
                        "completion provider is unavailable during ontology drafting".into(),
                    ));
                };
                // EVERY draft read and write goes through the trusted managed
                // backend re-entered into this tenant's scope — never the
                // guarded public handle, and never the default tenant.
                let scoped = TenantScoped::new(tenant.into(), runtime.backend.clone());
                if job.progress.completed < documents.len() {
                    if self.pause_or_cancel(runtime, &mut job).await? {
                        return Ok(DispatchOutcome::Parked);
                    }
                    let start = job.progress.completed;
                    let end = start.saturating_add(batch_size.max(1)).min(documents.len());
                    // Materialize the drafts collection on first use; the call
                    // is idempotent, so every resumed pass is a cheap no-op.
                    scoped
                        .ensure_collection(SPACE_TYPE_DRAFTS, CollectionType::Document)
                        .await?;

                    // The ACCUMULATOR IS THE STORED DRAFT. A pass merges its
                    // documents into `space_type_drafts/{space_type}` and writes
                    // it back before checkpointing progress, so a restart
                    // continues from the completions already paid for instead of
                    // re-spending them. Only a draft this job itself
                    // checkpointed is resumed: a first pass always starts clean
                    // and replaces any prior unaccepted draft, exactly as
                    // re-drafting does synchronously.
                    let stored = scoped.get_document(SPACE_TYPE_DRAFTS, &space_type).await?;
                    let resumable =
                        (start > 0)
                            .then_some(stored.as_ref())
                            .flatten()
                            .filter(|doc| {
                                doc.get("drafting_job").and_then(Value::as_str)
                                    == Some(job.id.as_str())
                            });
                    let (mut space, mut skips, mut sampled_chunks) = match resumable {
                        Some(doc) => {
                            let space: SpaceType =
                                serde_json::from_value((*doc).clone()).map_err(|error| {
                                    CogniGraphError::DocumentConflict(format!(
                                        "draft accumulator `{SPACE_TYPE_DRAFTS}/{space_type}` no \
                                     longer parses as a space type (edited?): {error}"
                                    ))
                                })?;
                            let skips = doc
                                .get("skips")
                                .and_then(Value::as_array)
                                .map(|rows| {
                                    rows.iter()
                                        .filter_map(Value::as_str)
                                        .map(str::to_string)
                                        .collect()
                                })
                                .unwrap_or_default();
                            let sampled =
                                doc.get("sampled_chunks")
                                    .and_then(Value::as_u64)
                                    .unwrap_or_default() as usize;
                            (space, skips, sampled)
                        }
                        None => (
                            cognigraph_construct::empty_per_document_draft(&space_type),
                            Vec::new(),
                            0usize,
                        ),
                    };

                    // One `draft_space_type` call per document against that
                    // document's OWN chunks — the per-document drafting that
                    // recovers entities a single broad sample starves — merged
                    // into the accumulator.
                    for (index, document) in documents.iter().enumerate().take(end).skip(start) {
                        if document.is_empty() {
                            continue;
                        }
                        let report = cognigraph_construct::draft_space_type(
                            &*completion,
                            &space_type,
                            document,
                            sample_cap,
                        )
                        .await
                        .map_err(|error| {
                            CogniGraphError::BackendError(format!(
                                "drafting document {index}: {error}"
                            ))
                        })?;
                        sampled_chunks = sampled_chunks.saturating_add(report.sampled_chunks);
                        cognigraph_construct::merge_drafted(&mut space, report.space, &mut skips);
                        for skip in report.skips {
                            skips.push(format!("[doc {index}] {skip}"));
                        }
                    }

                    // The cross-document specificity check and the gate advisor
                    // both need the WHOLE corpus in hand, so they run once, on
                    // the last pass — never per batch.
                    let final_pass = end >= documents.len();
                    let advisor = final_pass.then(|| {
                        cognigraph_construct::finalize_draft(&mut space, &documents, &mut skips);
                        let corpus: Vec<Chunk> = documents.iter().flatten().cloned().collect();
                        cognigraph_construct::advise_gates(&space, &[], &corpus)
                    });

                    let drafted_by = format!(
                        "draft:{}@{}",
                        completion.model_name(),
                        cognigraph_construct::DRAFT_REV
                    );
                    let mut doc = serde_json::to_value(&space)?;
                    let fields = doc
                        .as_object_mut()
                        .expect("a space type serializes to an object");
                    fields.insert("_key".into(), json!(space_type));
                    // Inert either way (no grounding path reads this
                    // collection), but only a FINISHED draft is acceptable:
                    // `draft_accept` requires `status == "draft"`, so a partial
                    // accumulator can never be turned into vocabulary.
                    fields.insert(
                        "status".into(),
                        json!(if final_pass { "draft" } else { "drafting" }),
                    );
                    fields.insert("drafted_by".into(), json!(drafted_by));
                    fields.insert("requested_by".into(), json!(job.actor.username));
                    fields.insert(
                        "drafted_at".into(),
                        json!(crate::routes::neurons::now_secs()),
                    );
                    fields.insert("skips".into(), json!(skips));
                    fields.insert("sampled_chunks".into(), json!(sampled_chunks));
                    fields.insert("drafting_job".into(), json!(job.id));
                    fields.insert("documents_drafted".into(), json!(end));
                    fields.insert("documents_total".into(), json!(documents.len()));
                    if let Some(advisor) = &advisor {
                        fields.insert("advisor".into(), serde_json::to_value(&advisor.rules)?);
                    }
                    // Written BEFORE the progress checkpoint: an interruption in
                    // between re-drafts this batch, and merging is idempotent
                    // (entities keyed by name, rules by source/relation/target,
                    // triggers deduped), so the accumulator cannot double-count.
                    if stored.is_some() {
                        scoped
                            .replace_document(SPACE_TYPE_DRAFTS, &space_type, doc)
                            .await?;
                    } else {
                        scoped.create_document(SPACE_TYPE_DRAFTS, doc).await?;
                    }
                    CURRENT_TENANT
                        .scope(tenant.into(), runtime.invalidate_search_results())
                        .await;

                    let _guard = self.transition_lock.lock().await;
                    job = self.get_raw(tenant, id).await?.ok_or_else(|| {
                        CogniGraphError::DocumentNotFound {
                            collection: "jobs".into(),
                            key: id.into(),
                        }
                    })?;
                    job.progress.completed = end;
                    job.updated_at_ms = now_millis();
                    self.metrics.checkpoints.fetch_add(1, Ordering::Relaxed);
                    if job.status == JobStatus::CancelRequested && end < documents.len() {
                        self.finish_canceled(tenant, &mut job, "canceled after durable checkpoint")
                            .await?;
                        return Ok(DispatchOutcome::Complete);
                    }
                    if self.shutting_down.load(Ordering::Acquire) || self.tenant_paused(tenant) {
                        self.requeue_interrupted(tenant, &mut job).await?;
                        return Ok(DispatchOutcome::Parked);
                    }
                    self.save_raw(tenant, &job).await?;
                    if end < documents.len() {
                        return Ok(DispatchOutcome::Continue);
                    }
                }
                // Report from the durable artifact, not from a local variable:
                // a job resumed past its final pass still reports truthfully.
                let written = scoped.get_document(SPACE_TYPE_DRAFTS, &space_type).await?;
                let count = |field: &str| {
                    written
                        .as_ref()
                        .and_then(|doc| doc.get(field))
                        .and_then(Value::as_array)
                        .map(Vec::len)
                        .unwrap_or_default()
                };
                let result = json!({
                    "space_type": space_type,
                    "documents": documents.len(),
                    "entities": count("entities"),
                    "relation_rules": count("relation_rules"),
                });
                let _guard = self.transition_lock.lock().await;
                job = self.get_raw(tenant, id).await?.ok_or_else(|| {
                    CogniGraphError::DocumentNotFound {
                        collection: "jobs".into(),
                        key: id.into(),
                    }
                })?;
                self.finish_succeeded(tenant, &mut job, result).await?;
                Ok(DispatchOutcome::Complete)
            }
        }
    }
}
