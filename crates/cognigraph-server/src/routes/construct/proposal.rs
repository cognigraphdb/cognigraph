//! Proposal.

use super::*;

#[derive(Deserialize)]
pub(super) struct ProposeRequest {
    pub(super) space_type: String,
    /// Gaps to repair, as `A --REL--> B` lines. When absent, the gaps are
    /// measured live: evaluate with the stored eval spec and take the
    /// missing facts.
    pub(super) gaps: Option<Vec<String>>,
    /// Inline eval spec for the gap measurement (as on /evaluate).
    pub(super) eval: Option<EvalSpec>,
    /// Side views as the gap source (CG-88); excludes `gaps` and `eval`.
    pub(super) side_views: Option<SideViewSelection>,
}
pub(super) async fn propose(
    State(state): State<AppState>,
    user: Option<Extension<User>>,
    Json(req): Json<ProposeRequest>,
) -> Result<Json<Value>, AppError> {
    if let Some(selection) = &req.side_views {
        if req.gaps.is_some() || req.eval.is_some() {
            return Err(AppError(CogniGraphError::ValidationError(
                "side_views cannot be combined with gaps or eval".into(),
            )));
        }
        selection.validate()?;
        if selection.dry_run {
            let space = load_space(&*state.managed_backend, &req.space_type).await?;
            let detected =
                side_view_gaps(&*state.managed_backend, &space, &req.space_type, selection).await?;
            return Ok(Json(json!({
                "space_type": req.space_type,
                "source": "side_views",
                "dry_run": true,
                "gaps": detected.gaps.len(),
                "side_views": detected.report,
            })));
        }
    }
    let Some(provider) = state.completion.clone() else {
        return Err(AppError(CogniGraphError::BackendError(
            "No completion provider configured. Set OPENAI_API_KEY or GEMINI_API_KEY \
             (model via COGNIGRAPH_COMPLETION_MODEL)."
                .into(),
        )));
    };
    let space = load_space(&*state.managed_backend, &req.space_type).await?;

    let mut from_side_views = None;
    let source = if req.side_views.is_some() {
        "side_views"
    } else if req.gaps.is_some() {
        "gaps"
    } else {
        "evaluation"
    };
    let gaps: Vec<Fact> = match req.gaps {
        None if req.side_views.is_some() => {
            let selection = req.side_views.as_ref().expect("checked above");
            let detected =
                side_view_gaps(&*state.managed_backend, &space, &req.space_type, selection).await?;
            let gaps = detected.gaps.clone();
            from_side_views = Some((selection.collection.clone(), detected));
            gaps
        }
        Some(lines) => lines
            .iter()
            .map(|line| {
                Fact::parse(line).ok_or_else(|| {
                    AppError(CogniGraphError::ValidationError(format!(
                        "gap `{line}` is not `A --REL--> B`"
                    )))
                })
            })
            .collect::<Result<_, _>>()?,
        None => {
            let spec = resolve_spec(&state, &req.space_type, req.eval).await?;
            evaluate(&*state.managed_backend, &req.space_type, &spec)
                .await?
                .missing
        }
    };
    let side_view_report = from_side_views
        .as_ref()
        .map(|(_, detected)| detected.report.clone());
    if gaps.is_empty() {
        let mut response = json!({
            "space_type": req.space_type,
            "source": source,
            "gaps": 0,
            "proposed": [],
            "skipped": [],
            "stored": 0,
            "note": "no gaps to repair",
        });
        if let Some(report) = side_view_report {
            response["side_views"] = report;
        }
        return Ok(Json(response));
    }

    let report = propose_neurons_via_backend(
        &*provider,
        &space,
        &gaps,
        &*state.managed_backend,
        &req.space_type,
    )
    .await
    .map_err(|e| AppError(CogniGraphError::BackendError(format!("proposing: {e}"))))?;

    // Store every proposal as `proposed` with authorship — the same shape
    // POST /api/neurons writes, so the review queue and acceptance validation
    // are identical. Validation failures and id conflicts become skips,
    // never partial silent drops.
    let proposed_by = actor(&user);
    let mut stored: Vec<Value> = Vec::new();
    let mut skipped: Vec<Value> = report
        .skipped
        .iter()
        .map(|s| {
            json!({
                "fact": format!("{} --{}--> {}", s.fact.source, s.fact.relation, s.fact.target),
                "reason": s.reason,
            })
        })
        .collect();
    let mut refusals: Vec<RefusalRow> = report
        .skipped
        .iter()
        .map(|s| {
            proposal_refusal(
                s.gate,
                &s.fact.source,
                &s.fact.relation,
                &s.fact.target,
                &s.reason,
            )
        })
        .collect();
    for mut neuron in report.set.neurons {
        neuron.status = NeuronStatus::Proposed;
        let set = NeuronSet {
            space_type: req.space_type.clone(),
            neurons: vec![neuron.clone()],
        };
        if let Err(e) = validate_neurons(&set, &space) {
            let reason = format!("validation: {e}");
            skipped.push(json!({ "fact": neuron.id, "reason": reason }));
            refusals.push(proposal_refusal(
                "validation_failed",
                &neuron.source,
                &neuron.relation,
                &neuron.target,
                &reason,
            ));
            continue;
        }
        let mut doc = serde_json::to_value(&neuron).map_err(CogniGraphError::from)?;
        if let Some(fields) = doc.as_object_mut() {
            fields.insert("_key".into(), json!(neuron.id));
            fields.insert("space_type".into(), json!(req.space_type));
            fields.insert("proposed_by".into(), json!(proposed_by));
            fields.insert("proposed_at".into(), json!(now_secs()));
            if let Some((collection, detected)) = &from_side_views
                && let Some(provenance) =
                    side_view_provenance(collection, &detected.support, &neuron)
            {
                fields.insert("proposed_from".into(), json!("side_views"));
                fields.insert("side_view_source".into(), provenance);
            }
        }
        match state.managed_backend.create_document(NEURONS, doc).await {
            Ok(_) => {
                state.invalidate_search_results().await;
                stored.push(json!({
                    "id": neuron.id,
                    "fact": format!("{} --{}--> {}", neuron.source, neuron.relation, neuron.target),
                    "triggers": neuron.triggers,
                }));
            }
            Err(CogniGraphError::DocumentConflict(_)) => {
                skipped.push(json!({ "fact": neuron.id, "reason": "neuron id already exists" }));
                refusals.push(proposal_refusal(
                    "duplicate_id",
                    &neuron.source,
                    &neuron.relation,
                    &neuron.target,
                    "neuron id already exists",
                ));
            }
            Err(e) => return Err(AppError(e)),
        }
    }
    let attribution = format!("propose:{}", provider.model_name());
    let ledger = record_refusals(
        &*state.managed_backend,
        &RefusalContext {
            space_type: &req.space_type,
            origin: "propose",
            policy: None,
            attribution: &attribution,
            actor: &proposed_by,
        },
        &refusals,
    )
    .await;
    let mut response = json!({
        "space_type": req.space_type,
        "source": source,
        "gaps": gaps.len(),
        "stored": stored.len(),
        "proposed": stored,
        "skipped": skipped,
        "proposed_by": proposed_by,
        "note": "proposals are inert until accepted via POST /api/neurons/{key}/accept",
    });
    if let Some(report) = side_view_report {
        response["side_views"] = report;
    }
    attach(&mut response, ledger);
    Ok(Json(response))
}

/// A proposal-side refusal: no chunk or quote, the gap triple as identity.
fn proposal_refusal(
    gate: &'static str,
    source: &str,
    relation: &str,
    target: &str,
    reason: &str,
) -> RefusalRow {
    RefusalRow {
        gate: gate.into(),
        source: source.into(),
        relation: relation.into(),
        target: target.into(),
        chunk_id: None,
        evidence: None,
        reason: reason.into(),
    }
}
pub(super) const REVIEW_POLICIES: &str = "review_policies";
