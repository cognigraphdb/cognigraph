use super::*;

struct Seq(std::sync::Mutex<Vec<Value>>);

#[async_trait::async_trait]
impl CompletionProvider for Seq {
    async fn complete_json(
        &self,
        _system: &str,
        _user: &str,
        _schema: &Value,
    ) -> anyhow::Result<Value> {
        Ok(self.0.lock().unwrap().remove(0))
    }
    fn model_name(&self) -> &str {
        "seq"
    }
}

fn chunk(id: &str, text: &str) -> Chunk {
    Chunk {
        id: id.into(),
        title: String::new(),
        text: text.into(),
    }
}

#[tokio::test]
async fn self_checks_enforce_verbatim_closure_and_affirmation() {
    let chunks = vec![
        chunk("c1", "Harbor selected the Stack+ platform in 2024."),
        chunk("c2", "It is not true that Harbor deployed HelperBot."),
    ];
    let provider = Seq(std::sync::Mutex::new(vec![
        json!({ "entities": [
            // Good: name + one verbatim alias, one paraphrased alias.
            { "name": "Harbor", "type": "org", "aliases": ["Stack+ platform", "Harbor Pharma Co"] },
            { "name": "Stack+", "type": "platform", "aliases": [] },
            // Invented canonical name: dropped whole.
            { "name": "Global BioPharm Inc", "type": "org", "aliases": [] }
        ]}),
        json!({ "rules": [
            // Survives: endpoints known, trigger verbatim + affirmed.
            { "source": "Harbor", "relation": "SELECTED", "target": "Stack+",
              "when_any": ["selected the stack+ platform", "chose stack+ eventually"] },
            // Closure violation: endpoint never drafted.
            { "source": "Harbor", "relation": "USES", "target": "HelperBot",
              "when_any": ["deployed helperbot"] },
            // Trigger occurs ONLY negated: rule dropped.
            { "source": "Stack+", "relation": "RELATES", "target": "Harbor",
              "when_any": ["harbor deployed helperbot"] },
            // Template trigger stripped (D4) -> no survivor -> dropped.
            { "source": "Harbor", "relation": "PICKED", "target": "Stack+",
              "when_any": ["{source} selected {target}"] }
        ]}),
    ]));
    let report = draft_space_type(&provider, "pilot", &chunks, 10)
        .await
        .unwrap();

    assert_eq!(report.space.id, "pilot");
    assert_eq!(report.space.entities.len(), 2);
    let harbor = &report.space.entities[0];
    assert_eq!(harbor.aliases, vec!["Stack+ platform"]); // paraphrase dropped
    assert_eq!(report.space.relation_rules.len(), 1);
    let rule = &report.space.relation_rules[0];
    assert_eq!(rule.when_any, vec!["selected the stack+ platform"]);
    assert!(rule.require_in_sentence.is_empty());

    let joined = report.skips.join("\n");
    assert!(joined.contains("Global BioPharm Inc"));
    assert!(joined.contains("Harbor Pharma Co"));
    assert!(joined.contains("chose stack+ eventually"));
    assert!(joined.contains("closure"));
    assert!(joined.contains("never occurs affirmed")); // negated-only trigger
    assert!(joined.contains("author-only (D4)")); // template stripped
    assert_eq!(report.advisor.rules.len(), 1);
}

#[tokio::test]
async fn sampling_is_deterministic_and_capped() {
    let chunks: Vec<Chunk> = (0..100).map(|i| chunk(&format!("c{i}"), "text")).collect();
    let a: Vec<&str> = sample(&chunks, 10).iter().map(|c| c.id.as_str()).collect();
    let b: Vec<&str> = sample(&chunks, 10).iter().map(|c| c.id.as_str()).collect();
    assert_eq!(a, b);
    assert_eq!(a.len(), 10);
    assert_eq!(a[0], "c0");
    assert_eq!(a[9], "c90");
    assert_eq!(sample(&chunks, 200).len(), 100);
}

#[tokio::test]
async fn per_document_drafting_merges_catalogues_across_documents() {
    // Each document is drafted against its OWN chunk, so both conditions and
    // both drug→condition rules survive — the exact starvation a single broad
    // draft (which samples ~evenly and misses per-document entities) produces.
    let doc_a = vec![chunk("a1", "DrugA is indicated for CondX.")];
    let doc_b = vec![chunk(
        "b1",
        "DrugB is indicated for CondY. DrugA also exists.",
    )];
    let provider = Seq(std::sync::Mutex::new(vec![
        // doc A: entities, then rules
        json!({ "entities": [
            { "name": "DrugA", "type": "drug", "aliases": [] },
            { "name": "CondX", "type": "condition", "aliases": [] }
        ]}),
        json!({ "rules": [
            { "source": "DrugA", "relation": "INDICATED_FOR", "target": "CondX",
              "when_any": ["druga is indicated for condx"] }
        ]}),
        // doc B: DrugA repeats with a conflicting type + a verbatim alias
        json!({ "entities": [
            { "name": "DrugB", "type": "drug", "aliases": [] },
            { "name": "CondY", "type": "condition", "aliases": [] },
            { "name": "DrugA", "type": "medication", "aliases": ["DrugA also"] }
        ]}),
        json!({ "rules": [
            { "source": "DrugB", "relation": "INDICATED_FOR", "target": "CondY",
              "when_any": ["drugb is indicated for condy"] }
        ]}),
    ]));

    let documents = vec![doc_a, doc_b];
    let report = draft_space_type_per_document(&provider, "meds", &documents, 10)
        .await
        .unwrap();

    let names: Vec<&str> = report
        .space
        .entities
        .iter()
        .map(|e| e.name.as_str())
        .collect();
    assert_eq!(report.space.entities.len(), 4, "DrugA deduped: {names:?}");
    let conditions = report
        .space
        .entities
        .iter()
        .filter(|e| e.entity_type == "condition")
        .count();
    assert_eq!(conditions, 2, "both CondX and CondY survive the merge");
    assert_eq!(report.space.relation_rules.len(), 2);
    assert!(
        report
            .space
            .relation_rules
            .iter()
            .all(|r| r.relation == "INDICATED_FOR")
    );

    // DrugA merged once: first-seen type kept, alias accumulated, conflict surfaced.
    let drug_a = report
        .space
        .entities
        .iter()
        .find(|e| e.name == "DrugA")
        .unwrap();
    assert_eq!(drug_a.entity_type, "drug");
    assert!(drug_a.aliases.contains(&"DrugA also".to_string()));
    assert!(
        report
            .skips
            .join("\n")
            .contains("type `drug` and `medication`"),
        "type conflict must be surfaced for the reviewer"
    );
}

#[tokio::test]
async fn incremental_merge_and_finalize_equal_the_whole_corpus_drafter() {
    // The durable `construct.draft` job cannot call
    // `draft_space_type_per_document` — it drafts a BATCH of documents per
    // pass and checkpoints between them — so it composes the same result
    // from `draft_space_type` + `merge_drafted` + `finalize_draft`. This
    // pins the two paths together: if the per-document drafter ever grows a
    // step the public pieces do not expose, the job would silently diverge.
    let responses = || {
        vec![
            json!({ "entities": [
                { "name": "MAXALT", "type": "drug", "aliases": [] },
                { "name": "FDA", "type": "org", "aliases": [] },
                { "name": "migraine", "type": "condition", "aliases": [] }
            ]}),
            json!({ "rules": [
                { "source": "MAXALT", "relation": "CONTACT", "target": "FDA",
                  "when_any": ["contact fda at 1-800-fda-1088"] },
                { "source": "MAXALT", "relation": "TREATS", "target": "migraine",
                  "when_any": ["maxalt is a migraine treatment"] }
            ]}),
            json!({ "entities": [
                { "name": "Atropine", "type": "drug", "aliases": [] },
                { "name": "FDA", "type": "agency", "aliases": [] }
            ]}),
            json!({ "rules": [
                { "source": "Atropine", "relation": "TREATS", "target": "FDA",
                  "when_any": ["atropine is an anticholinergic"] }
            ]}),
        ]
    };
    let documents = vec![
        vec![chunk(
            "a1",
            "MAXALT is a migraine treatment. To report adverse reactions, contact FDA at 1-800-FDA-1088.",
        )],
        vec![chunk(
            "b1",
            "Atropine is an anticholinergic. To report adverse reactions, contact FDA at 1-800-FDA-1088.",
        )],
    ];

    let whole = draft_space_type_per_document(
        &Seq(std::sync::Mutex::new(responses())),
        "labels",
        &documents,
        10,
    )
    .await
    .unwrap();

    // Batched exactly as the job runs it: one document per pass, merged into
    // an accumulator, finalized once over the whole corpus at the end.
    let batched = Seq(std::sync::Mutex::new(responses()));
    let mut space = empty_per_document_draft("labels");
    let mut skips: Vec<String> = Vec::new();
    let mut sampled = 0usize;
    for (index, doc) in documents.iter().enumerate() {
        let report = draft_space_type(&batched, "labels", doc, 10).await.unwrap();
        sampled += report.sampled_chunks;
        merge_drafted(&mut space, report.space, &mut skips);
        for skip in report.skips {
            skips.push(format!("[doc {index}] {skip}"));
        }
    }
    finalize_draft(&mut space, &documents, &mut skips);

    assert_eq!(sampled, whole.sampled_chunks);
    assert_eq!(skips, whole.skips, "skip transcripts must match verbatim");
    assert_eq!(
        serde_json::to_value(&space).unwrap(),
        serde_json::to_value(&whole.space).unwrap(),
        "incrementally composed draft must equal the per-document drafter's"
    );
    // Not a vacuous comparison: the corpus-wide boilerplate rule is gone and
    // the cross-document type conflict was surfaced.
    assert!(
        space
            .relation_rules
            .iter()
            .all(|rule| rule.relation != "CONTACT"),
        "boilerplate rule survived: {:?}",
        space.relation_rules
    );
    assert!(skips.join("\n").contains("type `org` and `agency`"));
}

#[tokio::test]
async fn cross_document_boilerplate_triggers_are_dropped() {
    // The real MAXALT --CONTACT--> FDA leak: a regulatory footer present in
    // nearly every label licensed a fact about a drug the other labels never
    // mention. It is only detectable as boilerplate once the whole corpus is
    // in hand — which is exactly what per-document drafting has.
    let doc_a = vec![chunk(
        "a1",
        "MAXALT is a migraine treatment. To report adverse reactions, contact FDA at 1-800-FDA-1088.",
    )];
    let doc_b = vec![chunk(
        "b1",
        "Atropine is an anticholinergic. To report adverse reactions, contact FDA at 1-800-FDA-1088.",
    )];
    let provider = Seq(std::sync::Mutex::new(vec![
        json!({ "entities": [
            { "name": "MAXALT", "type": "drug", "aliases": [] },
            { "name": "FDA", "type": "org", "aliases": [] },
            { "name": "migraine", "type": "condition", "aliases": [] }
        ]}),
        json!({ "rules": [
            // Boilerplate: fires in every label's footer.
            { "source": "MAXALT", "relation": "CONTACT", "target": "FDA",
              "when_any": ["contact fda at 1-800-fda-1088"] },
            // Specific: only ever appears in MAXALT's own text.
            { "source": "MAXALT", "relation": "TREATS", "target": "migraine",
              "when_any": ["maxalt is a migraine treatment"] }
        ]}),
        json!({ "entities": [
            { "name": "Atropine", "type": "drug", "aliases": [] },
            { "name": "FDA", "type": "org", "aliases": [] }
        ]}),
        json!({ "rules": [] }),
    ]));

    let documents = vec![doc_a, doc_b];
    let report = draft_space_type_per_document(&provider, "labels", &documents, 10)
        .await
        .unwrap();

    // The boilerplate rule is gone; the drug-specific one survives.
    assert_eq!(
        report.space.relation_rules.len(),
        1,
        "boilerplate rule must not survive: {:?}",
        report.space.relation_rules
    );
    let kept = &report.space.relation_rules[0];
    assert_eq!(kept.relation, "TREATS");
    assert_eq!(kept.target, "migraine");
    assert_eq!(kept.when_any, vec!["maxalt is a migraine treatment"]);

    // Both the dropped trigger and the dropped rule stay visible, never silent.
    let joined = report.skips.join("\n");
    assert!(joined.contains("corpus boilerplate"), "skips: {joined}");
    assert!(
        joined.contains("no trigger survived the cross-document specificity check"),
        "skips: {joined}"
    );
}

fn entity(name: &str, entity_type: &str) -> EntityDef {
    EntityDef {
        name: name.into(),
        entity_type: entity_type.into(),
        aliases: Vec::new(),
    }
}

fn rule(source: &str, relation: &str, target: &str, trigger: &str) -> RelationRule {
    RelationRule {
        source: source.into(),
        relation: relation.into(),
        target: target.into(),
        when_any: vec![trigger.into()],
        require_in_sentence: Vec::new(),
        trigger_provenance: Default::default(),
    }
}

/// Independent per-document drafts capitalize the same entity differently.
/// Those are ONE entity to the store (`entity_key` case-folds), and
/// `ingest_chunks` fails closed on the collision — so the draft must collapse
/// them itself rather than hand the operator an un-ingestable ontology.
#[test]
fn case_variant_entities_collapse_to_one_graph_identity() {
    let mut space = SpaceType {
        entities: vec![
            entity("Epinephrine", "drug"),
            entity("epinephrine", "drug"),
            entity("anaphylaxis", "condition"),
        ],
        relation_rules: vec![rule(
            "epinephrine",
            "TREATS",
            "anaphylaxis",
            "epinephrine is indicated for anaphylaxis",
        )],
        ..empty_per_document_draft("labels")
    };
    let mut skips = Vec::new();
    finalize_draft(&mut space, &[], &mut skips);

    let names: Vec<&str> = space.entities.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, vec!["Epinephrine", "anaphylaxis"]);
    assert_eq!(entity_key("Epinephrine"), entity_key("epinephrine"));
    assert!(
        skips.iter().any(|s| s.contains("same graph identity")),
        "the collapse must be visible: {skips:?}"
    );
}

/// The trap in collapsing entities: rules name their endpoints by literal
/// name and grounding resolves them by exact string, so a rule pointing at
/// the dropped variant would silently never ground again.
#[test]
fn collapsing_entities_rewrites_the_rules_that_named_the_dropped_variant() {
    let mut space = SpaceType {
        entities: vec![
            entity("Epinephrine", "drug"),
            entity("anaphylaxis", "condition"),
        ],
        relation_rules: vec![
            rule("epinephrine", "TREATS", "anaphylaxis", "treats anaphylaxis"),
            rule("Epinephrine", "TREATS", "anaphylaxis", "for anaphylaxis"),
        ],
        ..empty_per_document_draft("labels")
    };
    space.entities.push(entity("epinephrine", "drug"));
    let mut skips = Vec::new();
    finalize_draft(&mut space, &[], &mut skips);

    // One surviving rule, pointing at the kept entity, with BOTH triggers —
    // no rule orphaned and no evidence lost to the rewrite.
    assert_eq!(space.relation_rules.len(), 1, "{:?}", space.relation_rules);
    let kept = &space.relation_rules[0];
    assert_eq!(kept.source, "Epinephrine");
    assert_eq!(
        kept.when_any,
        vec![
            "treats anaphylaxis".to_string(),
            "for anaphylaxis".to_string()
        ]
    );
}

/// Regulatory scaffolding is not knowledge: on the 100-label pilot these
/// rules were 11% of the graph at 19.4% precision.
#[test]
fn administrative_endpoints_do_not_become_rules() {
    let mut space = SpaceType {
        entities: vec![
            entity("ENTRESTO", "drug"),
            entity("Sample Pharmaceuticals Corporation", "organization"),
            entity("heart failure", "condition"),
        ],
        relation_rules: vec![
            rule(
                "ENTRESTO",
                "REPORT_ADVERSE_REACTIONS_TO",
                "Sample Pharmaceuticals Corporation",
                "contact sample pharma",
            ),
            rule("ENTRESTO", "INDICATED_FOR", "heart failure", "is indicated"),
        ],
        ..empty_per_document_draft("labels")
    };
    let mut skips = Vec::new();
    finalize_draft(&mut space, &[], &mut skips);

    assert_eq!(space.relation_rules.len(), 1, "{:?}", space.relation_rules);
    assert_eq!(space.relation_rules[0].relation, "INDICATED_FOR");
    assert!(
        skips.iter().any(|s| s.contains("administrative")),
        "the drop must be visible: {skips:?}"
    );
}

/// The trap in the filter: `person` holds the PATIENT POPULATIONS that
/// contraindication rules depend on, and `parasite` / `anatomical site`
/// would die to a substring match on an administrative keyword.
#[test]
fn clinical_endpoints_that_look_administrative_survive() {
    let mut space = SpaceType {
        entities: vec![
            entity("Isotretinoin", "drug"),
            entity("pregnant woman", "person"),
            entity("Plasmodium falciparum", "parasite"),
            entity("injection site", "anatomical site"),
        ],
        relation_rules: vec![
            rule(
                "Isotretinoin",
                "CONTRAINDICATED_IN",
                "pregnant woman",
                "contraindicated in pregnant",
            ),
            rule(
                "Isotretinoin",
                "TREATS",
                "Plasmodium falciparum",
                "active against plasmodium",
            ),
            rule(
                "Isotretinoin",
                "CAUSES",
                "injection site",
                "injection site reactions",
            ),
        ],
        ..empty_per_document_draft("labels")
    };
    let mut skips = Vec::new();
    finalize_draft(&mut space, &[], &mut skips);

    assert_eq!(
        space.relation_rules.len(),
        3,
        "no clinical rule may be dropped: {:?} / {skips:?}",
        space.relation_rules
    );
}

/// A type disagreement between case variants is a human decision, exactly
/// like the same-name conflict `merge_entities` already surfaces (D3).
#[test]
fn case_variant_type_conflicts_are_surfaced_for_review() {
    let mut space = SpaceType {
        entities: vec![entity("Insulin", "substance"), entity("insulin", "drug")],
        ..empty_per_document_draft("labels")
    };
    let mut skips = Vec::new();
    finalize_draft(&mut space, &[], &mut skips);

    assert_eq!(space.entities.len(), 1);
    assert_eq!(
        space.entities[0].entity_type, "substance",
        "first-seen wins"
    );
    let joined = skips.join("\n");
    assert!(joined.contains("(review)"), "skips: {joined}");
    assert!(
        joined.contains("substance") && joined.contains("drug"),
        "skips: {joined}"
    );
}
