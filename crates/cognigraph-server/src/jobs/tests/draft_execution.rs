//! Draft execution.

use super::*;

#[tokio::test]
async fn draft_job_accumulates_across_passes_into_the_inert_stored_draft() {
    // batch_size 1 over 2 documents forces a real multi-pass run: pass one
    // can only draft document 0, so the second document's entities can only
    // join it by RESUMING the accumulator out of the stored draft. Asserting
    // the merged catalogue therefore proves both the checkpointed
    // `Continue` and the accumulator round-trip.
    let raw = Arc::new(NativeBackend::new());
    seed_raw(&*raw).await;
    let state = AppState::new_shared(raw.clone()).with_completion(SeqCompletion(Mutex::new(vec![
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
            { "name": "FDA", "type": "org", "aliases": [] }
        ]}),
        json!({ "rules": [] }),
    ])));
    state.jobs.set_batch_size(1);

    let submission = state
        .jobs
        .submit(
            state.clone(),
            "default".into(),
            "default".into(),
            JobActor::request(None),
            "draft-r1",
            JobKind::ConstructDraft,
            json!({
                "space_type": "labels",
                "chunks": [
                    { "id": "a1", "title": "Label A",
                      "text": "MAXALT is a migraine treatment. To report adverse reactions, contact FDA at 1-800-FDA-1088." },
                    { "id": "b1", "title": "Label B",
                      "text": "Atropine is an anticholinergic. To report adverse reactions, contact FDA at 1-800-FDA-1088." },
                ],
            }),
        )
        .await
        .unwrap();
    assert_eq!(submission.job.progress.total, 2, "one unit per document");

    let job = wait_terminal(&state, &submission.job.id).await;
    assert_eq!(job.status, JobStatus::Succeeded, "job: {:?}", job.error);
    assert_eq!(job.progress.completed, 2);
    let result = job.result.expect("a succeeded draft job carries a result");
    assert_eq!(result["space_type"], json!("labels"));
    assert_eq!(result["documents"], json!(2));
    assert_eq!(result["entities"], json!(4));
    assert_eq!(result["relation_rules"], json!(1));

    let stored = raw
        .get_document("space_type_drafts", "labels")
        .await
        .unwrap()
        .expect("the draft artifact is written");
    assert_eq!(stored["status"], json!("draft"), "final pass flips status");
    assert_eq!(
        stored["drafted_by"],
        json!("draft:scripted@draft-policy-v1")
    );
    assert_eq!(stored["requested_by"], json!("anonymous"));
    assert_eq!(stored["documents_drafted"], json!(2));
    assert_eq!(stored["documents_total"], json!(2));
    assert!(stored["drafted_at"].as_u64().is_some());
    assert!(stored["advisor"].is_array(), "advisor annotations attached");

    // Document 0's catalogue survived into the final artifact, so the second
    // pass resumed the accumulator instead of starting over.
    let entities: Vec<&str> = stored["entities"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|entity| entity["name"].as_str())
        .collect();
    assert_eq!(entities, vec!["MAXALT", "FDA", "migraine", "Atropine"]);

    // The cross-document specificity check ran once over the WHOLE corpus on
    // the final pass: the MedWatch footer rule is gone, the specific one is
    // not, and the drop stays visible in `skips`.
    let rules = stored["relation_rules"].as_array().unwrap();
    assert_eq!(rules.len(), 1, "boilerplate rule survived: {rules:?}");
    assert_eq!(rules[0]["relation"], json!("TREATS"));
    let skips = stored["skips"].as_array().unwrap();
    assert!(
        skips
            .iter()
            .filter_map(Value::as_str)
            .any(|skip| skip.contains("corpus boilerplate")),
        "skips: {skips:?}"
    );

    // Inert (D1): drafting never writes vocabulary and never auto-accepts.
    assert!(
        raw.get_document("space_types", "labels")
            .await
            .unwrap()
            .is_none(),
        "a draft job must never write `space_types`"
    );
    state.jobs.shutdown().await;
}
