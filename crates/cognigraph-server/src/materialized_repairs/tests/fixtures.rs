//! Fixtures.

use super::*;

pub(super) fn digest(label: &str) -> String {
    digest_bytes(label.as_bytes())
}
pub(super) fn fact(source: &str, relation: &str, target: &str, chunk: &str) -> VerifiedGraphFact {
    VerifiedGraphFact {
        source: source.into(),
        relation: relation.into(),
        target: target.into(),
        evidence_chunk_id: chunk.into(),
    }
}
pub(super) fn deployment_payload_value() -> Value {
    json!({
        "requested_action": "activate",
        "target": {"space_type": "pharma", "channel": "stable"},
        "semantic_repair_generation_id": "a".repeat(64),
        "semantic_repair_generation_digest": digest("generation"),
        "impact_digest": digest("impact"),
        "promotion_head_decision_id": "b".repeat(64),
        "promotion_head_projection_digest": digest("promotion-head"),
        "candidate_digest": digest("candidate"),
        "semantic_repair_revision_id": "c".repeat(64),
        "semantic_repair_revision_digest": digest("revision"),
        "semantic_repair_review_id": "d".repeat(64),
        "semantic_repair_review_digest": digest("review"),
        "expected_deployment_head_decision_id": null,
        "rollback_target_generation_id": null,
        "reason": "activate verified generation",
        "idempotency_key_hash": digest("idempotency"),
        "promoter_registration_id": "registration-1",
        "promoter_principal_id": "principal-1",
        "signed_at_ms": 1,
    })
}
pub(super) struct TestCasRoot(pub(super) PathBuf);
impl TestCasRoot {
    pub(super) fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "cognigraph-m26-capability-cas-{}-{}",
            std::process::id(),
            now_millis()
        ));
        if root.exists() {
            fs::remove_dir_all(&root).expect("remove stale M26 capability CAS fixture");
        }
        fs::create_dir_all(root.join("tenants")).expect("create M26 capability CAS fixture");
        Self(root)
    }
}
impl Drop for TestCasRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
pub(super) fn generation_capacity_entries(
    count: usize,
    space_type: impl Fn(usize) -> String,
    canonical_record_bytes: impl Fn(usize) -> u64,
) -> Vec<(String, String, u64)> {
    (0..count)
        .map(|index| {
            (
                space_type(index),
                format!("idempotency-{index}"),
                canonical_record_bytes(index),
            )
        })
        .collect()
}
pub(super) fn validate_test_generation_capacity(
    entries: &[(String, String, u64)],
) -> Result<(), GenerationCapacityViolation> {
    validate_generation_capacity_entries(entries.iter().map(
        |(space_type, idempotency_key_hash, canonical_record_bytes)| GenerationCapacityEntry {
            space_type,
            idempotency_key_hash,
            canonical_record_bytes: *canonical_record_bytes,
        },
    ))
}
