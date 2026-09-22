use super::*;

/// Returns scripted proposals in order, one per call.
struct Seq(std::sync::Mutex<Vec<serde_json::Value>>);

#[async_trait::async_trait]
impl CompletionProvider for Seq {
    async fn complete_json(
        &self,
        _system: &str,
        _user: &str,
        _schema: &serde_json::Value,
    ) -> anyhow::Result<serde_json::Value> {
        Ok(self.0.lock().unwrap().remove(0))
    }
    fn model_name(&self) -> &str {
        "seq"
    }
}

fn space() -> SpaceType {
    serde_json::from_value(json!({
        "id": "s",
        "entities": [
            {"name": "Harbor", "type": "org", "aliases": []},
            {"name": "Harbor Chat", "type": "product", "aliases": []}
        ],
        "relation_rules": [
            {"source": "Harbor", "relation": "CO_DEVELOPED", "target": "Harbor Chat",
             "when_any": ["co-developed harbor chat"]}
        ]
    }))
    .unwrap()
}

fn chunk(id: &str, text: &str) -> Chunk {
    Chunk {
        id: id.into(),
        title: String::new(),
        text: text.into(),
    }
}

fn blocker(id: &str, phrase: &str) -> serde_json::Value {
    json!({
        "id": id, "type": "relation_blocker", "confidence": 0.9,
        "rationale": "test", "source": "Harbor", "relation": "CO_DEVELOPED",
        "target": "Harbor Chat", "when_any": [phrase],
        "evidence": ["quoted"]
    })
}

#[tokio::test]
async fn iteration_covers_the_remainder_round_by_round() {
    let space = space();
    let fact = Fact::parse("Harbor --CO_DEVELOPED--> Harbor Chat").unwrap();
    // Two violating chunks with DISJOINT illegitimate-context wording:
    // one veto phrase cannot cover both — the one-shot failure shape.
    let chunks = vec![
        chunk(
            "v1",
            "Rumors allege a rival co-developed Harbor Chat with partners.",
        ),
        chunk(
            "v2",
            "An unverified filing claims a vendor co-developed Harbor Chat.",
        ),
    ];
    // Round 1 covers v1 only; round 2 is asked ONLY about v2.
    let provider = Seq(std::sync::Mutex::new(vec![
        blocker("veto-allegation", "rumors allege"),
        blocker("veto-filing", "unverified filing"),
    ]));

    let report = propose_blockers_covering(&provider, &space, &[fact], &chunks, 5)
        .await
        .unwrap();
    assert_eq!(report.set.neurons.len(), 2);
    assert_eq!(report.facts.len(), 1);
    let outcome = &report.facts[0];
    assert!(outcome.covered, "two rounds must cover both chunks");
    assert_eq!(outcome.rounds, 2);
    assert_eq!(outcome.violating_total, 2);
    assert!(outcome.uncovered.is_empty());
    assert_eq!(outcome.stopped, None);
    // Round records: round 1 suppressed v1 leaving v2, round 2 closed it.
    assert_eq!(report.coverage[0].suppressed, vec!["v1"]);
    assert_eq!(report.coverage[0].uncovered, vec!["v2"]);
    assert_eq!(report.coverage[1].suppressed, vec!["v2"]);
    assert!(report.coverage[1].uncovered.is_empty());
}

#[tokio::test]
async fn iteration_stops_at_the_round_cap_and_discloses_the_remainder() {
    let space = space();
    let fact = Fact::parse("Harbor --CO_DEVELOPED--> Harbor Chat").unwrap();
    let chunks = vec![
        chunk(
            "v1",
            "Rumors allege a rival co-developed Harbor Chat with partners.",
        ),
        chunk(
            "v2",
            "An unverified filing claims a vendor co-developed Harbor Chat.",
        ),
    ];
    let provider = Seq(std::sync::Mutex::new(vec![blocker(
        "veto-allegation",
        "rumors allege",
    )]));
    let report = propose_blockers_covering(&provider, &space, &[fact], &chunks, 1)
        .await
        .unwrap();
    let outcome = &report.facts[0];
    assert!(!outcome.covered);
    assert_eq!(outcome.uncovered, vec!["v2"]);
    assert_eq!(outcome.stopped.as_deref(), Some("round cap 1 reached"));
}

#[tokio::test]
async fn a_failed_round_is_a_visible_skip_never_a_silent_stop() {
    let space = space();
    let fact = Fact::parse("Harbor --CO_DEVELOPED--> Harbor Chat").unwrap();
    let chunks = vec![chunk(
        "v1",
        "Rumors allege a rival co-developed Harbor Chat with partners.",
    )];
    // The proposal's phrase is a paraphrase (not verbatim): the
    // symbolic self-check rejects it on both attempts.
    let provider = Seq(std::sync::Mutex::new(vec![
        blocker("veto-bad", "gossip suggests"),
        blocker("veto-bad", "gossip suggests"),
    ]));
    let report = propose_blockers_covering(&provider, &space, &[fact], &chunks, 3)
        .await
        .unwrap();
    assert!(report.set.neurons.is_empty());
    let outcome = &report.facts[0];
    assert!(!outcome.covered);
    assert_eq!(outcome.rounds, 1);
    assert!(outcome.stopped.as_deref().unwrap().contains("round 1"));
    assert_eq!(report.skipped.len(), 1);
    assert!(report.skipped[0].reason.contains("paraphrased"));
}
