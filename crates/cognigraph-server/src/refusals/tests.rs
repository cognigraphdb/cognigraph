//! The refusal ledger: deterministic, capped, generated, never a reason for a
//! completed construction to fail.

use std::collections::HashMap;
use std::sync::Arc;

use cognigraph_core::GraphBackend;
use cognigraph_native::NativeBackend;
use serde_json::json;

use super::{
    LedgerOutcome, RefusalContext, RefusalRow, attach, record_refusals, record_refusals_capped,
    refusal_key,
};
use crate::system_collections::{
    GuardedBackend, REFUSALS_COLLECTION, is_generated_collection, is_managed_collection,
};

fn ctx<'a>(space: &'a str, origin: &'a str) -> RefusalContext<'a> {
    RefusalContext {
        space_type: space,
        origin,
        policy: Some("directed-policy-v2"),
        attribution: "directed:test-model@directed-policy-v2",
        actor: "admin",
    }
}

fn row(gate: &str, source: &str, target: &str, chunk: Option<&str>) -> RefusalRow {
    RefusalRow {
        gate: gate.into(),
        source: source.into(),
        relation: "OWNS".into(),
        target: target.into(),
        chunk_id: chunk.map(str::to_string),
        evidence: Some(format!("{source} owns {target}.")),
        reason: format!("`{source} --OWNS--> {target}`: refused by {gate}"),
    }
}

async fn count(backend: &dyn GraphBackend) -> u64 {
    backend
        .list_collections()
        .await
        .unwrap()
        .into_iter()
        .find(|c| c.name == REFUSALS_COLLECTION)
        .map(|c| c.count)
        .unwrap_or(0)
}

#[tokio::test]
async fn records_rows_with_deterministic_keys_and_full_context() {
    let backend = NativeBackend::new();
    let rows = [
        row("relation_not_in_taxonomy", "Ann", "Acme", Some("c1")),
        row("target_not_in_sentence", "Ann", "Zed", Some("c1")),
    ];
    let out: LedgerOutcome = record_refusals(&backend, &ctx("acme", "directed"), &rows)
        .await
        .unwrap();
    assert_eq!((out.stored, out.dropped), (2, 0));
    assert_eq!(out.rows.len(), 2);
    assert_eq!(count(&backend).await, 2);

    let key = refusal_key(&ctx("acme", "directed"), &rows[0]);
    assert_eq!(out.rows[0]["_key"], key);
    assert_eq!(out.rows[0]["stored"], true);
    let stored = backend
        .get_document(REFUSALS_COLLECTION, &key)
        .await
        .unwrap()
        .expect("row stored");
    for (field, expected) in [
        ("space_type", json!("acme")),
        ("origin", json!("directed")),
        ("gate", json!("relation_not_in_taxonomy")),
        ("source", json!("Ann")),
        ("relation", json!("OWNS")),
        ("target", json!("Acme")),
        ("chunk_id", json!("c1")),
        ("evidence", json!("Ann owns Acme.")),
        ("policy", json!("directed-policy-v2")),
        (
            "attribution",
            json!("directed:test-model@directed-policy-v2"),
        ),
        ("actor", json!("admin")),
    ] {
        assert_eq!(stored[field], expected, "{field}: {stored}");
    }
    assert!(stored["reason"].as_str().unwrap().contains("refused by"));
    assert!(stored["recorded_at"].as_f64().unwrap() > 0.0);
}

#[tokio::test]
async fn empty_input_writes_nothing_and_creates_no_collection() {
    let backend = NativeBackend::new();
    let out = record_refusals(&backend, &ctx("acme", "directed"), &[])
        .await
        .unwrap();
    assert_eq!((out.stored, out.dropped, out.rows.len()), (0, 0, 0));
    assert!(
        backend
            .list_collections()
            .await
            .unwrap()
            .iter()
            .all(|c| c.name != REFUSALS_COLLECTION)
    );
}

#[tokio::test]
async fn identical_refusal_is_recorded_once_and_still_counts_as_stored() {
    let backend = NativeBackend::new();
    let rows = [row("empty_endpoint", "", "Acme", Some("c1"))];
    let first = record_refusals(&backend, &ctx("acme", "directed"), &rows)
        .await
        .unwrap();
    let key = first.rows[0]["_key"].as_str().unwrap().to_string();
    let recorded_at = backend
        .get_document(REFUSALS_COLLECTION, &key)
        .await
        .unwrap()
        .unwrap()["recorded_at"]
        .clone();

    let second = record_refusals(&backend, &ctx("acme", "directed"), &rows)
        .await
        .unwrap();
    assert_eq!((second.stored, second.dropped), (1, 0));
    assert_eq!(second.rows[0]["stored"], true);
    assert_eq!(count(&backend).await, 1);
    // First writer wins: the original record is immutable.
    assert_eq!(
        backend
            .get_document(REFUSALS_COLLECTION, &key)
            .await
            .unwrap()
            .unwrap()["recorded_at"],
        recorded_at
    );
}

#[tokio::test]
async fn space_origin_gate_chunk_and_evidence_all_partition_the_key() {
    let base = row("evidence_not_verbatim", "Ann", "Acme", Some("c1"));
    let k = |c: &RefusalContext<'_>, r: &RefusalRow| refusal_key(c, r);
    let d = ctx("acme", "directed");
    assert_ne!(k(&d, &base), k(&ctx("beta", "directed"), &base));
    assert_ne!(k(&d, &base), k(&ctx("acme", "propose"), &base));
    let mut other = base.clone();
    other.gate = "source_not_in_sentence".into();
    assert_ne!(k(&d, &base), k(&d, &other));
    let mut other = base.clone();
    other.chunk_id = Some("c2".into());
    assert_ne!(k(&d, &base), k(&d, &other));
    let mut other = base.clone();
    other.evidence = Some("Ann owns Acme".into());
    assert_ne!(k(&d, &base), k(&d, &other));
    // Attribution, actor and reason text are context, not identity.
    let mut same = base.clone();
    same.reason = "different wording".into();
    assert_eq!(k(&d, &base), k(&d, &same));
    assert_eq!(
        k(&d, &base),
        k(
            &RefusalContext {
                attribution: "other-model",
                actor: "someone-else",
                ..d
            },
            &base
        )
    );
}

#[tokio::test]
async fn duplicates_within_one_batch_collapse_to_one_row() {
    let backend = NativeBackend::new();
    let r = row("chunk_not_in_request", "Ann", "Acme", Some("missing"));
    let out = record_refusals(
        &backend,
        &ctx("acme", "directed"),
        &[r.clone(), r.clone(), r],
    )
    .await
    .unwrap();
    assert_eq!((out.stored, out.dropped), (1, 0));
    assert_eq!(out.rows.len(), 3, "every input refusal is echoed back");
    assert!(out.rows.iter().all(|row| row["stored"] == true));
    assert_eq!(count(&backend).await, 1);
}

#[tokio::test]
async fn cap_drops_the_newest_and_keeps_existing_rows() {
    let backend = NativeBackend::new();
    let c = ctx("acme", "directed");
    let existing = [row("g", "A", "B", None), row("g", "A", "C", None)];
    record_refusals_capped(&backend, &c, &existing, 3)
        .await
        .unwrap();
    let incoming = [
        row("g", "A", "D", None),
        row("g", "A", "E", None),
        row("g", "A", "F", None),
    ];
    let out = record_refusals_capped(&backend, &c, &incoming, 3)
        .await
        .unwrap();
    assert_eq!((out.stored, out.dropped), (1, 2), "{out:?}");
    assert_eq!(out.rows.len(), 3);
    assert_eq!(out.rows[0]["stored"], true);
    assert_eq!(out.rows[1]["stored"], false);
    assert_eq!(out.rows[2]["stored"], false);
    assert_eq!(count(&backend).await, 3);
    // The existing rows survive untouched; the newest were the ones dropped.
    for r in &existing {
        assert!(
            backend
                .get_document(REFUSALS_COLLECTION, &refusal_key(&c, r))
                .await
                .unwrap()
                .is_some()
        );
    }
}

#[tokio::test]
async fn at_cap_an_already_recorded_refusal_counts_as_stored_not_dropped() {
    let backend = NativeBackend::new();
    let c = ctx("acme", "directed");
    let known = row("g", "A", "B", None);
    record_refusals_capped(&backend, &c, std::slice::from_ref(&known), 1)
        .await
        .unwrap();
    let out = record_refusals_capped(&backend, &c, &[known, row("g", "A", "Z", None)], 1)
        .await
        .unwrap();
    assert_eq!((out.stored, out.dropped), (1, 1));
    assert_eq!(out.rows[0]["stored"], true);
    assert_eq!(out.rows[1]["stored"], false);
    assert_eq!(count(&backend).await, 1);
}

#[tokio::test]
async fn a_zero_cap_stores_nothing_but_still_reports_every_refusal() {
    let backend = NativeBackend::new();
    let out = record_refusals_capped(
        &backend,
        &ctx("acme", "directed"),
        &[row("g", "A", "B", None)],
        0,
    )
    .await
    .unwrap();
    assert_eq!((out.stored, out.dropped, out.rows.len()), (0, 1, 1));
    assert_eq!(count(&backend).await, 0);
}

#[tokio::test]
async fn optional_fields_and_unicode_round_trip() {
    let backend = NativeBackend::new();
    let mut r = row("proposal_rejected", "Acm\u{e9}", "Zo\u{eb}", None);
    r.evidence = None;
    r.relation = "".into();
    let out = record_refusals(&backend, &ctx("acm\u{e9}", "propose"), &[r])
        .await
        .unwrap();
    let stored = backend
        .get_document(REFUSALS_COLLECTION, out.rows[0]["_key"].as_str().unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored["chunk_id"], json!(null));
    assert_eq!(stored["evidence"], json!(null));
    assert_eq!(stored["relation"], json!(""));
    assert_eq!(stored["source"], json!("Acm\u{e9}"));
    assert_eq!(stored["space_type"], json!("acm\u{e9}"));
}

#[tokio::test]
async fn ledger_is_generated_read_only_through_public_surfaces_and_exported() {
    assert!(is_generated_collection(REFUSALS_COLLECTION));
    assert!(!is_managed_collection(REFUSALS_COLLECTION));

    let raw = Arc::new(NativeBackend::new());
    record_refusals(
        raw.as_ref(),
        &ctx("acme", "directed"),
        &[row("empty_endpoint", "", "Acme", Some("c1"))],
    )
    .await
    .unwrap();
    let guarded = GuardedBackend::new(raw.clone());
    let rows = guarded
        .query(
            &format!("FOR r IN {REFUSALS_COLLECTION} RETURN r.gate"),
            HashMap::new(),
        )
        .await
        .unwrap();
    assert_eq!(rows, vec![json!("empty_endpoint")]);
    assert!(
        guarded
            .create_document(REFUSALS_COLLECTION, json!({"_key": "forged"}))
            .await
            .is_err()
    );
    assert!(
        guarded
            .query(
                &format!("INSERT {{ _key: \"forged\" }} INTO {REFUSALS_COLLECTION}"),
                HashMap::new()
            )
            .await
            .is_err()
    );
    let snapshot = guarded.export_snapshot().await.unwrap();
    assert_eq!(
        snapshot["collections"][REFUSALS_COLLECTION]["documents"]
            .as_object()
            .map(|d| d.len()),
        Some(1),
        "{snapshot}"
    );
}

#[tokio::test]
async fn attach_reports_a_ledger_failure_without_hiding_the_construction_result() {
    let mut response = json!({"facts_grounded": 2, "skips": ["x"]});
    attach(
        &mut response,
        Err(cognigraph_core::CogniGraphError::BackendError(
            "disk full".into(),
        )),
    );
    assert_eq!(response["facts_grounded"], 2);
    assert_eq!(response["refusals"], json!([]));
    assert_eq!(response["refusals_stored"], 0);
    assert_eq!(response["refusals_dropped"], 0);
    assert!(
        response["refusals_error"]
            .as_str()
            .unwrap()
            .contains("disk full")
    );

    let mut ok = json!({});
    attach(
        &mut ok,
        Ok(LedgerOutcome {
            rows: vec![json!({"_key": "k", "stored": false})],
            stored: 0,
            dropped: 1,
        }),
    );
    assert_eq!(ok["refusals"][0]["_key"], "k");
    assert_eq!(ok["refusals_dropped"], 1);
    assert!(ok.get("refusals_error").is_none());
}
