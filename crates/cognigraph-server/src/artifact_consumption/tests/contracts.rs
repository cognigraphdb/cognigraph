//! Contracts.

use super::*;

#[test]
fn canonical_digest_cannot_mask_non_nfc_oracle_substitution() {
    let make_spec = |target: &str| EvalSpec {
        space_id: "medical".into(),
        questions: vec![EvalQuestion {
            id: "q1".into(),
            question: "What is supplied?".into(),
            expected_facts: vec![format!("Meridian --SUPPLIES--> {target}")],
            forbidden_facts: vec!["Meridian --OWNS--> Compound X".into()],
        }],
    };
    let nfc = make_spec("Caf\u{e9}");
    let nfd = make_spec("Cafe\u{301}");

    assert_eq!(
        canonical_digest(&nfc).unwrap(),
        canonical_digest(&nfd).unwrap()
    );
    assert_ne!(
        serde_json::to_value(&nfc).unwrap(),
        serde_json::to_value(&nfd).unwrap()
    );
    validate_eval_spec_text(&nfc).unwrap();
    assert!(validate_eval_spec_text(&nfd).is_err());
}
#[test]
fn checked_consumption_plan_matches_the_supported_loader_contract() {
    let checked: ArtifactConsumptionPlan = serde_json::from_str(include_str!(
        "../../../../../docs/examples/m21-consumption-plan.json"
    ))
    .unwrap();
    assert_eq!(checked, ArtifactConsumptionPlan::supported_v1());
    checked.validate().unwrap();
}
#[test]
fn checked_m22_plan_matches_the_supported_derivation_contract() {
    let checked: ArtifactConsumptionPlan = serde_json::from_str(include_str!(
        "../../../../../docs/examples/m22-derivation-plan.json"
    ))
    .unwrap();
    assert_eq!(checked, ArtifactConsumptionPlan::supported_v2());
    checked.validate().unwrap();

    let mut explicit_null = serde_json::to_value(&checked).unwrap();
    explicit_null["preparation"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<ArtifactConsumptionPlan>(explicit_null).is_err());
}
#[test]
fn checked_m23_plan_matches_the_supported_preparation_contract() {
    let checked: ArtifactConsumptionPlan = serde_json::from_str(include_str!(
        "../../../../../docs/examples/m23-preparation-plan.json"
    ))
    .unwrap();
    assert_eq!(checked, ArtifactConsumptionPlan::supported_v3());
    checked.validate().unwrap();
}
#[test]
fn pinned_plan_rejects_an_explicit_null_entrypoint() {
    let mut plan: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../docs/examples/m21-consumption-plan.json"
    ))
    .unwrap();
    plan["corpus"]["entrypoint"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<ArtifactConsumptionPlan>(plan).is_err());
}
#[test]
fn derivation_receipt_rejects_explicit_null_generation_fields() {
    for field in ["chunk_count", "preparation"] {
        let mut object = serde_json::Map::new();
        object.insert(field.into(), serde_json::Value::Null);
        let error = serde_json::from_value::<CorpusGraphDerivationReceipt>(
            serde_json::Value::Object(object),
        )
        .unwrap_err();
        assert!(
            error.to_string().contains("invalid type: null"),
            "explicit null for {field} reached later required-field validation: {error}"
        );
    }
}
#[test]
fn derivation_authority_rejects_explicit_null_preparation() {
    let authority = EvidenceDerivationAuthority {
        candidate_original_derivation_digest: format!("sha256:{}", "1".repeat(64)),
        candidate_replay_derivation_digest: format!("sha256:{}", "2".repeat(64)),
        candidate_graph_digest: format!("sha256:{}", "3".repeat(64)),
        baseline_original_derivation_digest: format!("sha256:{}", "4".repeat(64)),
        baseline_replay_derivation_digest: format!("sha256:{}", "5".repeat(64)),
        baseline_graph_digest: format!("sha256:{}", "6".repeat(64)),
        derivation_plan_digest: format!("sha256:{}", "7".repeat(64)),
        preparation: None,
        authority_digest: format!("sha256:{}", "8".repeat(64)),
    };
    let mut explicit_null = serde_json::to_value(authority).unwrap();
    explicit_null["preparation"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<EvidenceDerivationAuthority>(explicit_null).is_err());
}
