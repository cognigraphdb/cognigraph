//! B7: the self-healing loop, deterministically pinned. A space at full
//! recall suffers a document REVISION that paraphrases every passage its
//! accepted neurons' triggers rely on (fixtures: crowdstrike_2024
//! chunks.jsonl vs chunks.v2.jsonl). The loop: degradation detected →
//! gap-directed proposing rewires through the new wording → review
//! accepts → recall restores → graduation flags the dead neurons for
//! pruning. Failure → rewire → prune, with review at the joint.

use std::path::Path;

use cognigraph_construct::*;
use cognigraph_embeddings::completion::CompletionProvider;
use cognigraph_native::NativeBackend;
use serde_json::{Value, json};

fn dir() -> &'static Path {
    Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/semantic-neurons/generalization"
    ))
}

fn load_chunks(name: &str) -> Vec<Chunk> {
    std::fs::read_to_string(dir().join(name))
        .unwrap()
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

/// Per-gap scripted proposer: picks its response by the fact named in the
/// prompt — triggers are verbatim v2 wording.
struct GapScripted;

#[async_trait::async_trait]
impl CompletionProvider for GapScripted {
    fn model_name(&self) -> &str {
        "scripted"
    }
    async fn complete_json(&self, _s: &str, user: &str, _schema: &Value) -> anyhow::Result<Value> {
        let (id, trigger) = if user.contains("Falcon Sensor --RUNS_ON-->") {
            (
                "falcon-agent-on-windows-v2",
                "the Falcon agent deployed across Microsoft Windows workstations and servers",
            )
        } else if user.contains("--AFFECTS--> Delta Air Lines") {
            (
                "delta-most-affected-v2",
                "Delta, the most affected of the US major airlines",
            )
        } else if user.contains("George Kurtz --LEADS-->") {
            (
                "kurtz-chief-executive-v2",
                "George Kurtz, chief executive of CrowdStrike",
            )
        } else if user.contains("--RESPONDED_WITH--> Faulty update") {
            (
                "defective-push-v2",
                "a defective configuration push went out from CrowdStrike",
            )
        } else if user.contains("CrowdStrike --CAUSED-->") {
            // The revision also killed BASE-rule pathways — neurons are
            // the only rewiring mechanism for those (rules are immutable
            // to the loop).
            (
                "crowdstrike-caused-outage-v2",
                "push went out from CrowdStrike to its Falcon Sensor security software that caused widespread problems",
            )
        } else if user.contains("Faulty update --AFFECTS--> Falcon Sensor") {
            (
                "defective-push-hit-falcon-v2",
                "a defective configuration push went out from CrowdStrike to its Falcon Sensor",
            )
        } else {
            anyhow::bail!("unexpected gap prompt");
        };
        let fact_line = user.lines().next().unwrap_or_default().to_string();
        Ok(json!({
            "id": id,
            "type": "relation_hint",
            "confidence": 0.85,
            "rationale": format!("v2 wording for: {fact_line}"),
            "evidence": [trigger],
            "source": "", "relation": "", "target": "",  // forced from the gap
            "triggers": [trigger]
        }))
    }
}

#[tokio::test]
async fn document_revision_degrades_rewires_and_prunes() {
    let space: SpaceType = serde_json::from_str(
        &std::fs::read_to_string(dir().join("crowdstrike_2024.space_type.json")).unwrap(),
    )
    .unwrap();
    let set: NeuronSet = serde_json::from_str(
        &std::fs::read_to_string(dir().join("crowdstrike_2024.neurons.accepted.json")).unwrap(),
    )
    .unwrap();
    let spec: EvalSpec = serde_json::from_str(
        &std::fs::read_to_string(dir().join("crowdstrike_2024.eval.json")).unwrap(),
    )
    .unwrap();
    let v1 = load_chunks("crowdstrike_2024.chunks.jsonl");
    let v2 = load_chunks("crowdstrike_2024.chunks.v2.jsonl");

    // 1. Healthy state on v1: full recall, no degraded neuron pathways.
    let backend = NativeBackend::new();
    let config = effective_config(&space, &set.neurons);
    ingest_chunks(&backend, &spec.space_id, &config, &v1, &[])
        .await
        .unwrap();
    let healthy = evaluate(&backend, &spec.space_id, &spec).await.unwrap();
    assert!(healthy.recall_ok(), "v1 baseline must be 6/6");
    assert!(
        degradation_report(&space, &set.neurons, &v1)
            .iter()
            .all(|p| p.kind != PathwayKind::Neuron),
        "no neuron pathway is dead on v1"
    );

    // 2. The document is revised: every neuron trigger passage paraphrased.
    let backend = NativeBackend::new();
    ingest_chunks(&backend, &spec.space_id, &config, &v2, &[])
        .await
        .unwrap();
    let degraded_eval = evaluate(&backend, &spec.space_id, &spec).await.unwrap();
    assert!(
        !degraded_eval.recall_ok(),
        "the revision must break recall (got {}/{})",
        degraded_eval.expected_found,
        degraded_eval.expected_total
    );

    // 3. Detection: all four accepted neurons are dead pathways.
    let dead: Vec<String> = degradation_report(&space, &set.neurons, &v2)
        .into_iter()
        .filter(|p| p.kind == PathwayKind::Neuron)
        .map(|p| p.id)
        .collect();
    assert_eq!(dead.len(), 4, "all four neuron pathways died: {dead:?}");

    // 4. Rewiring: gap-directed proposing over the REVISED corpus.
    let report = propose_neurons_via_backend(
        &GapScripted,
        &space,
        &degraded_eval.missing,
        &backend,
        &spec.space_id,
    )
    .await
    .unwrap();
    assert!(
        report.skipped.is_empty(),
        "every gap rewired: {:?}",
        report.skipped
    );
    assert!(
        report
            .set
            .neurons
            .iter()
            .all(|n| n.status == NeuronStatus::Proposed),
        "rewired paths arrive inert — review stays human"
    );

    // 5. Review accepts; the combined set restores recall on v2.
    let mut combined = set.neurons.clone();
    combined.extend(report.set.neurons.iter().cloned().map(|mut n| {
        n.status = NeuronStatus::Accepted;
        n
    }));
    let backend = NativeBackend::new();
    ingest_chunks(
        &backend,
        &spec.space_id,
        &effective_config(&space, &combined),
        &v2,
        &[],
    )
    .await
    .unwrap();
    let healed = evaluate(&backend, &spec.space_id, &spec).await.unwrap();
    assert!(
        healed.recall_ok(),
        "recall restored after rewiring (got {}/{}, missing {:?})",
        healed.expected_found,
        healed.expected_total,
        healed.missing
    );
    assert!(healed.restraint_ok(), "healing must not open violations");

    // 6. Pruning: graduation now flags exactly the DEAD originals — their
    //    facts are covered by the new pathways (CoveredByOthers) — and
    //    none of the rewired neurons.
    let candidates = graduation_report(&space, &combined, &v2);
    let flagged: Vec<&str> = candidates.iter().map(|c| c.neuron_id.as_str()).collect();
    for old in &dead {
        assert!(flagged.contains(&old.as_str()), "dead `{old}` must flag");
    }
    for new in report.set.neurons.iter().map(|n| n.id.as_str()) {
        assert!(!flagged.contains(&new), "rewired `{new}` must NOT flag");
    }
    // Either coverage reason means prunable: the rewired pathway carries
    // the fact (CoveredByOthers), or the revision incidentally satisfied a
    // naive base trigger (CoveredByBase).
    assert!(candidates.iter().all(|c| matches!(
        c.reason,
        GraduationReason::CoveredByOthers | GraduationReason::CoveredByBase
    )));
}
