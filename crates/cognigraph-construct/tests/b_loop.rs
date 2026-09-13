//! Arc B machinery, deterministically pinned (scripted providers, no LLM):
//! B4 backend-retrieved proposer evidence, B3 violation-directed blocker
//! proposing with coverage simulation, B2 answer-level recall/restraint.

use std::sync::Mutex;

use cognigraph_construct::*;
use cognigraph_embeddings::completion::CompletionProvider;
use cognigraph_native::NativeBackend;
use serde_json::{Value, json};

/// Returns a fixed JSON value and records every user prompt it saw.
struct Scripted {
    response: Value,
    prompts: Mutex<Vec<String>>,
}

impl Scripted {
    fn new(response: Value) -> Self {
        Self {
            response,
            prompts: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait::async_trait]
impl CompletionProvider for Scripted {
    fn model_name(&self) -> &str {
        "scripted"
    }
    async fn complete_json(&self, _s: &str, user: &str, _schema: &Value) -> anyhow::Result<Value> {
        self.prompts.lock().unwrap().push(user.to_string());
        Ok(self.response.clone())
    }
}

fn pharma_space() -> SpaceType {
    serde_json::from_value(json!({
        "id": "pharma-allegations",
        "entities": [
            {"name": "Novapharm", "type": "organization", "aliases": []},
            {"name": "Meridian", "type": "organization", "aliases": []},
            {"name": "Compound X", "type": "compound", "aliases": ["compound x"]}
        ],
        "relation_rules": [
            {"source": "Novapharm", "relation": "SUPPLIES", "target": "Compound X",
             "when_any": ["novapharm supplies compound x"]},
            {"source": "Meridian", "relation": "SUPPLIES", "target": "Compound X",
             "when_any": ["meridian supplying compound x"]}
        ]
    }))
    .unwrap()
}

fn chunks() -> Vec<Chunk> {
    vec![
        Chunk {
            id: "p-001".into(),
            title: "supply agreement".into(),
            text: "Under the 2024 agreement, Novapharm supplies Compound X to hospital \
                   networks across Europe."
                .into(),
        },
        Chunk {
            id: "p-002".into(),
            title: "regulatory probe".into(),
            text: "Regulators confirmed Meridian is under investigation over claims of \
                   Meridian supplying Compound X to unlicensed distributors."
                .into(),
        },
    ]
}

// ---------------------------------------------------------------------------
// B3: violation-directed blocker proposing
// ---------------------------------------------------------------------------

#[tokio::test]
async fn violation_proposes_blocker_with_coverage() {
    let provider = Scripted::new(json!({
        "id": "meridian-allegation-blocker",
        "type": "relation_blocker",
        "confidence": 0.9,
        "rationale": "Investigation wording, not an assertion.",
        "evidence": ["Meridian is under investigation over claims"],
        "source": "Meridian",
        "relation": "SUPPLIES",
        "target": "Compound X",
        "when_any": ["under investigation", "totally made up phrase"]
    }));
    let violations = [Fact::parse("Meridian --SUPPLIES--> Compound X").unwrap()];
    let report = propose_blockers_report(&provider, &pharma_space(), &violations, &chunks())
        .await
        .unwrap();

    assert_eq!(report.set.neurons.len(), 1);
    let blocker = &report.set.neurons[0];
    assert_eq!(blocker.kind, NeuronKind::RelationBlocker);
    assert_eq!(blocker.status, NeuronStatus::Proposed, "review stays human");
    // The paraphrased veto phrase was filtered by the symbolic self-check.
    assert_eq!(blocker.triggers, ["under investigation"]);
    // Coverage simulation: the veto suppresses the violating chunk.
    assert_eq!(report.coverage.len(), 1);
    assert_eq!(report.coverage[0].suppressed, ["p-002"]);
    assert!(report.coverage[0].uncovered.is_empty());
    // The prompt carried the violating passage and the fired trigger.
    let prompts = provider.prompts.lock().unwrap();
    assert!(prompts[0].contains("under investigation over claims"));
    assert!(prompts[0].contains("meridian supplying compound x"));
}

#[tokio::test]
async fn blocker_proposal_skips_when_nothing_grounds_or_all_paraphrased() {
    // A forbidden fact that never grounds is a skip, not a proposal.
    let provider = Scripted::new(json!({"unused": true}));
    let ghost = [Fact::parse("Novapharm --SUPPLIES--> Compound X").unwrap()];
    let mut space = pharma_space();
    space.relation_rules.retain(|r| r.source == "Meridian");
    let report = propose_blockers_report(&provider, &space, &ghost, &chunks())
        .await
        .unwrap();
    assert!(report.set.neurons.is_empty());
    assert_eq!(report.skipped.len(), 1);
    assert!(report.skipped[0].reason.contains("does not ground"));

    // All-paraphrased veto phrases exhaust retries into a reasoned skip.
    let paraphraser = Scripted::new(json!({
        "id": "bad-blocker",
        "type": "relation_blocker",
        "evidence": ["x"],
        "confidence": 0.5,
        "source": "Meridian",
        "relation": "SUPPLIES",
        "target": "Compound X",
        "when_any": ["wording that appears nowhere"]
    }));
    let violations = [Fact::parse("Meridian --SUPPLIES--> Compound X").unwrap()];
    let report = propose_blockers_report(&paraphraser, &pharma_space(), &violations, &chunks())
        .await
        .unwrap();
    assert!(report.set.neurons.is_empty());
    assert!(report.skipped[0].reason.contains("paraphrased"));
}

// ---------------------------------------------------------------------------
// B4: backend-retrieved proposer evidence
// ---------------------------------------------------------------------------

#[tokio::test]
async fn backend_retrieval_finds_evidence_surface_matching_misses() {
    let space = pharma_space();
    let backend = NativeBackend::new();
    // The evidencing chunk never names either endpoint — surface matching
    // cannot find it; BM25 on the relation wording can.
    let corpus = vec![
        Chunk {
            id: "c-1".into(),
            title: "unrelated".into(),
            text: "Quarterly results were announced in Basel.".into(),
        },
        Chunk {
            id: "c-2".into(),
            title: "supply note".into(),
            text: "The firm supplies the compound to three hospital networks.".into(),
        },
    ];
    ingest_chunks(&backend, "pharma-allegations", &space, &corpus, &[])
        .await
        .unwrap();

    let provider = Scripted::new(json!({
        "id": "novapharm-supplies-compound-x",
        "type": "relation_hint",
        "confidence": 0.8,
        "evidence": ["The firm supplies the compound to three hospital networks."],
        "source": "Novapharm",
        "relation": "SUPPLIES",
        "target": "Compound X",
        "triggers": ["supplies the compound to three hospital networks"]
    }));
    let missing = [Fact::parse("Novapharm --SUPPLIES--> Compound X").unwrap()];

    // Surface-only proposing skips: no chunk mentions the endpoints.
    let surface = propose_neurons_report(&provider, &space, &missing, &corpus)
        .await
        .unwrap();
    assert!(surface.set.neurons.is_empty());
    assert!(surface.skipped[0].reason.contains("no candidate evidence"));

    // Backend retrieval reaches the chunk through BM25 and proposes.
    let report =
        propose_neurons_via_backend(&provider, &space, &missing, &backend, "pharma-allegations")
            .await
            .unwrap();
    assert_eq!(report.set.neurons.len(), 1, "skips: {:?}", report.skipped);
    let prompts = provider.prompts.lock().unwrap();
    assert!(
        prompts.last().unwrap().contains("supplies the compound"),
        "BM25-retrieved chunk reached the prompt"
    );
}

// ---------------------------------------------------------------------------
// B2: answer-level recall and restraint
// ---------------------------------------------------------------------------

#[tokio::test]
async fn answer_eval_scores_recall_and_restraint() {
    let space = pharma_space();
    let backend = NativeBackend::new();
    ingest_chunks(&backend, "pharma-allegations", &space, &chunks(), &[])
        .await
        .unwrap();
    let spec: EvalSpec = serde_json::from_value(json!({
        "space_id": "pharma-allegations",
        "questions": [{
            "id": "q1",
            "question": "Who supplies Compound X to hospitals?",
            "expected_facts": ["Novapharm --SUPPLIES--> Compound X"],
            "forbidden_facts": ["Meridian --SUPPLIES--> Compound X"]
        }]
    }))
    .unwrap();

    // A faithful answerer: asserts exactly the legitimate fact.
    let good = Scripted::new(json!({"facts": ["Novapharm --SUPPLIES--> Compound X"]}));
    let outcome = answer_eval(&backend, "pharma-allegations", &space, &[], &spec, &good)
        .await
        .unwrap();
    assert!(outcome.recall_ok() && outcome.restraint_ok());
    assert_eq!(outcome.questions[0].expected_found, 1);
    // The trace fed to the model used canonical display names.
    {
        let prompts = good.prompts.lock().unwrap();
        assert!(prompts[0].contains("Novapharm --SUPPLIES--> Compound X"));
    }

    // An over-asserting answerer trips answer-level restraint.
    let bad = Scripted::new(json!({"facts": [
        "Novapharm --SUPPLIES--> Compound X",
        "Meridian --SUPPLIES--> Compound X"
    ]}));
    let outcome = answer_eval(&backend, "pharma-allegations", &space, &[], &spec, &bad)
        .await
        .unwrap();
    assert!(outcome.recall_ok());
    assert!(!outcome.restraint_ok());
    assert_eq!(outcome.questions[0].forbidden_asserted, 1);
    // The manufactured allegation grounds without a blocker, so the
    // forbidden fact IS in the trace: asserting it is bad selection, not
    // fabrication -- and restraint catches it either way.
    assert!(outcome.questions[0].fabricated.is_empty());
    assert_eq!(outcome.questions[0].expected_found, 1);

    // A model that invents a fact absent from its trace: flagged as a
    // fabrication, never credited toward recall.
    let inventor = Scripted::new(json!({"facts": [
        "Novapharm --SUPPLIES--> Compound X",
        "Ghost Corp --SUPPLIES--> Compound X"
    ]}));
    let outcome = answer_eval(
        &backend,
        "pharma-allegations",
        &space,
        &[],
        &spec,
        &inventor,
    )
    .await
    .unwrap();
    assert_eq!(
        outcome.questions[0].fabricated,
        vec!["Ghost Corp --SUPPLIES--> Compound X".to_string()]
    );
    assert_eq!(outcome.questions[0].expected_found, 1);
    assert!(outcome.restraint_ok());
}
