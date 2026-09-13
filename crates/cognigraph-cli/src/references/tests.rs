use super::*;
fn snapshot() -> Vec<u8> {
    serde_json::to_vec(&json!({"collections":{
        "docs":{"type":"document","documents":{"café":{"_key":"café"},"cafe\u{301}":{"_key":"cafe\u{301}"}}},
        "rels":{"type":"edge","documents":{"edge":{"_key":"edge","_from":"docs/café","_to":"docs/missing"}}},
        "side_views":{"type":"document","documents":{"q":{"document_id":"docs/café"}}},
        "_cognigraph_jobs":{"type":"document","documents":{"j":{"_execution":{"collection":"docs","keys":["café"]},"signed":"e\u{301}"}}}
    }})).unwrap()
}
fn plan(bytes: &[u8]) -> Value {
    json!({"snapshot_sha256":digest(bytes),"changes":[{
        "collection":"rels","key":"edge","field":"_from","from":"docs/café","to":"docs/cafe\u{301}"}]})
}
#[test]
fn audit_discloses_existing_wrong_target_risk_and_protected_jobs() {
    let report = audit(&snapshot()).unwrap();
    let findings = report["findings"].as_array().unwrap();
    assert!(findings.iter().any(|f| f["kind"] == "ambiguous_reference"
        && f["exact_target_exists"] == true
        && f["candidate_count"] == 2));
    assert!(findings.iter().any(|f| f["kind"] == "unresolved_reference"));
    assert!(
        findings
            .iter()
            .any(|f| f["collection"] == "_cognigraph_jobs" && f["protected"] == true)
    );
    assert!(!report["truncated"].as_bool().unwrap());
}
#[test]
fn explicit_repair_preserves_identity_and_every_other_value() {
    let bytes = snapshot();
    let (output, report) = repair(&bytes, &serde_json::to_vec(&plan(&bytes)).unwrap()).unwrap();
    let mut expected = parse(&bytes).unwrap();
    expected["collections"]["rels"]["documents"]["edge"]["_from"] = json!("docs/cafe\u{301}");
    assert_eq!(parse(&output).unwrap(), expected);
    assert_eq!(report["changes"], 1);
    assert!(
        repair(&output, &serde_json::to_vec(&plan(&bytes)).unwrap()).is_err(),
        "plan bound to source bytes"
    );
}
#[test]
fn unsafe_or_stale_mappings_fail_without_output() {
    let bytes = snapshot();
    for (field, value) in [
        ("collection", json!("_cognigraph_jobs")),
        ("collection", json!("side_views")),
        ("field", json!("_key")),
        ("from", json!("docs/other")),
        ("to", json!("docs/missing")),
        ("to", json!("docs/café")),
        ("key", json!("missing")),
    ] {
        let mut p = plan(&bytes);
        p["changes"][0][field] = value;
        assert!(
            repair(&bytes, &serde_json::to_vec(&p).unwrap()).is_err(),
            "{p}"
        );
    }
    let mut p = plan(&bytes);
    let duplicate = p["changes"][0].clone();
    p["changes"].as_array_mut().unwrap().push(duplicate);
    assert!(repair(&bytes, &serde_json::to_vec(&p).unwrap()).is_err());
}
#[test]
fn bounded_audit_rejects_bad_shapes_and_reports_truncation() {
    assert!(audit(br#"{"collections": []}"#).is_err());
    let mut source = parse(&snapshot()).unwrap();
    for i in 0..MAX_FINDINGS + 1 {
        source["collections"]["rels"]["documents"][format!("e{i}")] = json!({"_to":"docs/missing"});
    }
    let report = audit(&serde_json::to_vec(&source).unwrap()).unwrap();
    assert_eq!(report["truncated"], true);
    assert_eq!(report["findings"].as_array().unwrap().len(), MAX_FINDINGS);
}

#[test]
fn offline_commands_parse_and_never_replace_an_existing_output() {
    use crate::args::{Command, parse};
    let arguments = |values: &[&str]| values.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    assert_eq!(
        parse(&arguments(&["references", "audit", "snapshot.json"]))
            .unwrap()
            .command,
        Command::ReferencesAudit {
            snapshot: "snapshot.json".into()
        }
    );
    assert!(
        parse(&arguments(&[
            "references",
            "repair",
            "snapshot.json",
            "plan.json"
        ]))
        .is_err()
    );
    let command = parse(&arguments(&[
        "references",
        "repair",
        "snapshot.json",
        "plan.json",
        "--out",
        "fixed.json",
    ]))
    .unwrap()
    .command;
    assert!(matches!(command, Command::ReferencesRepair { .. }));
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("fixed.json");
    crate::write_new_private_file(output.to_str().unwrap(), b"original").unwrap();
    assert!(crate::write_new_private_file(output.to_str().unwrap(), b"replacement").is_err());
    assert_eq!(std::fs::read(output).unwrap(), b"original");
}
