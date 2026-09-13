//! Review.

use super::*;

/// POST /api/construct/review — judge the space's proposed neurons and apply
/// the authored review policy: Lane A auto-accepts (whitelisted kinds,
/// judge accept at/above threshold, non-precision-critical rules, same
/// acceptance validation as a human accept); everything else stays
/// proposed WITH the verdict attached as triage. The judge never
/// auto-rejects.
pub(super) async fn review(
    State(state): State<AppState>,
    Json(req): Json<ReviewRequest>,
) -> Result<Json<Value>, AppError> {
    let policy_doc = state
        .backend
        .get_document(REVIEW_POLICIES, &req.space_type)
        .await?
        .ok_or_else(|| {
            AppError(CogniGraphError::ValidationError(format!(
                "no review policy stored for `{}` in `{REVIEW_POLICIES}` — without one, \
                 review stays fully human; only pre-existing legacy policies remain readable",
                req.space_type
            )))
        })?;
    let policy: ReviewPolicy = serde_json::from_value(policy_doc.clone()).map_err(|e| {
        AppError(CogniGraphError::ValidationError(format!(
            "bad review policy: {e}"
        )))
    })?;
    if !policy.injection_suite_passed {
        return Err(AppError(CogniGraphError::ValidationError(
            "review policy lacks injection_suite_passed: true — run the judge injection \
             suite for this POLICY_REV first (decision_review_policy.md, D5)"
                .into(),
        )));
    }
    for kind in &policy.auto_accept.kinds {
        if !matches!(kind.as_str(), "relation_hint" | "alias") {
            return Err(AppError(CogniGraphError::ValidationError(format!(
                "auto_accept.kinds may only contain relation_hint or legacy alias, got `{kind}` \
                 (aliases always queue; blockers and rank hints are never auto-acceptable)"
            ))));
        }
    }
    if !(0.0..=1.0).contains(&policy.auto_accept.min_confidence)
        || !(0.0..=1.0).contains(&policy.sampling_rate)
    {
        return Err(AppError(CogniGraphError::ValidationError(
            "min_confidence and sampling_rate must be within [0, 1]".into(),
        )));
    }
    if policy.auto_accept.qualified_judges.is_empty() {
        return Err(AppError(CogniGraphError::ValidationError(
            "review policy lacks auto_accept.qualified_judges — Lane A authority is bound \
             to judge models that passed BOTH harnesses for this POLICY_REV \
             (decision_review_policy.md; qualification is per-model, measured)"
                .into(),
        )));
    }
    if let Some(agreement) = &policy.auto_accept.agreement {
        if !agreement.concordance_measured {
            return Err(AppError(CogniGraphError::ValidationError(
                "agreement lane lacks concordance_measured: true — run the offline \
                 concordance simulation for this judge pair first \
                 (decision_agreement_lane.md, examples/concordance_sim.rs)"
                    .into(),
            )));
        }
        if agreement.judges.len() != 2 || agreement.judges[0] == agreement.judges[1] {
            return Err(AppError(CogniGraphError::ValidationError(
                "agreement.judges must name exactly two DISTINCT judge models — the pair \
                 is the attested unit"
                    .into(),
            )));
        }
        for kind in &agreement.kinds {
            if kind != "relation_hint" {
                return Err(AppError(CogniGraphError::ValidationError(format!(
                    "agreement lane kind `{kind}` is not allowed — only relation_hint \
                     (the direction-critical class is a hint class)"
                ))));
            }
        }
        if !(0.0..=1.0).contains(&agreement.min_confidence) {
            return Err(AppError(CogniGraphError::ValidationError(
                "agreement.min_confidence must be within [0, 1]".into(),
            )));
        }
    }

    let Some(judge) = state.judge.clone().or_else(|| state.completion.clone()) else {
        return Err(AppError(CogniGraphError::BackendError(
            "No judge available. Set OPENAI_API_KEY (and optionally COGNIGRAPH_JUDGE_MODEL)."
                .into(),
        )));
    };
    let space = load_space(&*state.managed_backend, &req.space_type).await?;

    use cognigraph_core::{FieldPredicate, PredicateOp};
    let space_predicate = |field: &str| FieldPredicate {
        path: vec![field.into()],
        op: PredicateOp::Eq,
        value: json!(req.space_type),
    };
    let chunks: Vec<Chunk> = state
        .backend
        .list_documents_filtered("chunks", &[space_predicate("space_id")], None, None, None)
        .await
        .unwrap_or_default()
        .into_iter()
        .filter_map(|doc| {
            Some(Chunk {
                id: doc.get("_key")?.as_str()?.to_string(),
                title: String::new(),
                text: doc.get("text")?.as_str()?.to_string(),
            })
        })
        .collect();
    let mut proposed: Vec<Value> = state
        .backend
        .list_documents_filtered(
            NEURONS,
            &[
                space_predicate("space_type"),
                FieldPredicate {
                    path: vec!["status".into()],
                    op: PredicateOp::Eq,
                    value: json!("proposed"),
                },
            ],
            None,
            None,
            None,
        )
        .await
        .unwrap_or_default();
    // A judged-and-queued proposal is awaiting a human (its verdict is
    // the triage); re-judging it would stall a limited drain on the
    // same queue head. `rejudge: true` overrides after a policy change.
    if !req.rejudge {
        proposed.retain(|doc| doc.get("judge_verdict").is_none());
    }
    // Deterministic order so limited calls drain the queue without
    // skips or repeats across invocations.
    proposed.sort_by(|a, b| {
        let key = |v: &Value| {
            v.get("_key")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string()
        };
        key(a).cmp(&key(b))
    });
    let pending_total = proposed.len();
    if let Some(limit) = req.limit {
        proposed.truncate(limit);
    }
    let started = std::time::Instant::now();
    let mut judge_calls = 0usize;

    let judged_by = format!("judge:{}@{}", judge.model_name(), POLICY_REV);
    // Lane A is model-bound: an unqualified judge still produces triage
    // verdicts, but nothing it reviews may auto-accept.
    let judge_qualified = policy
        .auto_accept
        .qualified_judges
        .iter()
        .any(|model| model == judge.model_name());
    // Lane A+ activation: the policy's attested pair must be exactly the
    // two judges this deployment runs. Anything else (partner missing,
    // model mismatch) degrades A+ to queue — never to single-judge.
    let partner = state.judge_partner.clone();
    let agreement_active = policy.auto_accept.agreement.as_ref().is_some_and(|a| {
        partner.as_ref().is_some_and(|p| {
            let running = [judge.model_name(), p.model_name()];
            a.judges.iter().all(|m| running.contains(&m.as_str()))
                && running.iter().all(|m| a.judges.iter().any(|j| j == m))
        })
    });
    let lane_a_plus_status = match &policy.auto_accept.agreement {
        None => json!(Value::Null),
        Some(_) if agreement_active => json!("active"),
        Some(a) => json!(format!(
            "withheld: attested pair {:?} does not match the running judges",
            a.judges
        )),
    };
    let mut skipped: Vec<Value> = Vec::new();
    let mut skipped_pending = 0usize;
    let mut auto_accepted: Vec<Value> = Vec::new();
    let mut queued: Vec<Value> = Vec::new();

    for doc in proposed {
        let Some(key) = doc.get("_key").and_then(Value::as_str).map(str::to_string) else {
            continue;
        };
        let Ok(neuron) = serde_json::from_value::<Neuron>(doc.clone()) else {
            continue;
        };
        judge_calls += 1;
        let verdict = judge_neuron(&*judge, &neuron, &chunks)
            .await
            .map_err(|e| AppError(CogniGraphError::BackendError(format!("judge: {e}"))))?;

        // The verdict is always recorded on the neuron — triage for the
        // human queue, provenance for auto-accepts.
        let mut merge = json!({
            "judge_verdict": verdict.verdict,
            "judge_confidence": verdict.confidence,
            "judge_reasoning": verdict.reasoning,
            "judged_by": judged_by,
            "judged_at": now_secs(),
        });

        let kind_name = serde_json::to_value(neuron.kind)
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_default();
        let eligible = judge_qualified
            && policy.auto_accept.kinds.contains(&kind_name)
            && verdict.verdict == "accept"
            && verdict.confidence >= policy.auto_accept.min_confidence;
        // Ok(lane_label) = auto-accept; Err(reason) = queue.
        let mut lane: Result<&'static str, String> = match lane_a_eligible(&neuron, &space) {
            LaneClass::Eligible if eligible => Ok("A"),
            LaneClass::DirectionCritical(why) => {
                // Lane A+ (decision_agreement_lane.md): concordance of the
                // attested pair may auto-accept this class; anything less
                // queues with both verdicts attached.
                let agreement = policy.auto_accept.agreement.as_ref();
                let plus_candidate = agreement_active
                    && agreement.is_some_and(|a| {
                        a.kinds.contains(&kind_name)
                            && verdict.verdict == "accept"
                            && verdict.confidence >= a.min_confidence
                    });
                if plus_candidate {
                    let (partner, agreement) = (partner.as_ref().unwrap(), agreement.unwrap());
                    judge_calls += 1;
                    let second = judge_neuron(&**partner, &neuron, &chunks)
                        .await
                        .map_err(|e| {
                            AppError(CogniGraphError::BackendError(format!("partner judge: {e}")))
                        })?;
                    merge["partner_verdict"] = json!(second.verdict);
                    merge["partner_confidence"] = json!(second.confidence);
                    merge["partner_reasoning"] = json!(second.reasoning);
                    merge["partner_judged_by"] =
                        json!(format!("judge:{}@{}", partner.model_name(), POLICY_REV));
                    if second.verdict == "accept" && second.confidence >= agreement.min_confidence {
                        Ok("A+")
                    } else {
                        Err(format!(
                            "agreement lane: no concordance (partner verdict: {} at {:.2}) — {why}",
                            second.verdict, second.confidence
                        ))
                    }
                } else if agreement_active {
                    Err(format!("agreement lane: primary below bar — {why}"))
                } else {
                    Err(why.to_string())
                }
            }
            LaneClass::Excluded(reason) => Err(reason.to_string()),
            LaneClass::Eligible if verdict.verdict == "accept" => Err(if judge_qualified {
                "confidence below threshold or kind not in policy".to_string()
            } else {
                format!(
                    "judge model `{}` is not in qualified_judges — auto-accept withheld",
                    judge.model_name()
                )
            }),
            LaneClass::Eligible => Err(format!("judge verdict: {}", verdict.verdict)),
        };

        // The provider ran without a lifecycle lock. Re-read the exact source
        // before recording ANY verdict, including triage on still-proposed rows.
        // Concurrent reviewers cannot overwrite each other's recorded results.
        let _guard = state.neuron_lifecycle.lock().await;
        let current = state.managed_backend.get_document(NEURONS, &key).await?;
        if current.as_ref() != Some(&doc)
            || doc.get("status").and_then(Value::as_str) != Some("proposed")
            || state
                .managed_backend
                .get_document(REVIEW_POLICIES, &req.space_type)
                .await?
                .as_ref()
                != Some(&policy_doc)
            || serde_json::to_value(load_space(&*state.managed_backend, &req.space_type).await?)
                .map_err(CogniGraphError::from)?
                != serde_json::to_value(&space).map_err(CogniGraphError::from)?
        {
            if current.as_ref().is_some_and(|doc| {
                doc["status"] == "proposed" && (req.rejudge || doc.get("judge_verdict").is_none())
            }) {
                skipped_pending += 1;
            }
            skipped.push(
                json!({"key": key, "reason": "review source changed; stale result discarded"}),
            );
            continue;
        }
        if lane.is_ok() {
            match validate_acceptance(&*state.managed_backend, &req.space_type, &key, neuron).await
            {
                Ok(()) => {}
                Err(AppError(CogniGraphError::ValidationError(reason))) => {
                    lane = Err(format!("acceptance validation: {reason}"));
                }
                Err(error) => return Err(error),
            }
        }

        match lane {
            Ok(label) => {
                merge["status"] = json!("accepted");
                merge["lane"] = json!(label);
                merge["reviewed_by"] = if label == "A+" {
                    json!(format!(
                        "judges:{}+{}@{}",
                        judge.model_name(),
                        partner.as_ref().map(|p| p.model_name()).unwrap_or("?"),
                        POLICY_REV
                    ))
                } else {
                    json!(judged_by)
                };
                merge["reviewed_at"] = json!(now_secs());
                let mut note = verdict.reasoning.clone();
                let boundary = note.floor_char_boundary(300.min(note.len()));
                note.truncate(boundary);
                merge["review_note"] = json!(note);
                if sampled(&key, policy.sampling_rate) {
                    merge["audit_sample"] = json!(true);
                }
                state
                    .managed_backend
                    .update_document(NEURONS, &key, merge)
                    .await?;
                state.invalidate_search_results().await;
                auto_accepted.push(json!({
                    "key": key,
                    "lane": label,
                    "confidence": verdict.confidence,
                }));
            }
            Err(reason) => {
                state
                    .managed_backend
                    .update_document(NEURONS, &key, merge)
                    .await?;
                state.invalidate_search_results().await;
                queued.push(json!({
                    "key": key,
                    "verdict": verdict.verdict,
                    "confidence": verdict.confidence,
                    "lane_b_reason": reason,
                }));
            }
        }
    }

    let reviewed = auto_accepted.len() + queued.len();
    Ok(Json(json!({
        "space_type": req.space_type,
        "judge": judged_by,
        "reviewed": reviewed,
        "pending_remaining": pending_total.saturating_sub(reviewed + skipped.len()) + skipped_pending,
        "judge_calls": judge_calls,
        "elapsed_ms": started.elapsed().as_millis() as u64,
        "lane_a_plus": lane_a_plus_status,
        "lane_a": if judge_qualified { json!("active") } else {
            json!(format!("withheld: judge model `{}` not in qualified_judges", judge.model_name()))
        },
        "auto_accepted": auto_accepted,
        "queued": queued,
        "skipped": skipped,
        "note": "queued proposals keep status=proposed with the verdict attached; \
                 the judge never auto-rejects",
    })))
}
