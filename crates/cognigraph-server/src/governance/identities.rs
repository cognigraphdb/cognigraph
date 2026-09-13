//! Identities.

use super::*;

pub(super) fn is_semantic_repair_collection(collection: &str) -> bool {
    matches!(
        collection,
        SEMANTIC_REPAIR_REVISIONS_COLLECTION | SEMANTIC_REPAIR_REVIEWS_COLLECTION
    )
}
pub fn key_registration_id(tenant: &str, incarnation: &str, key_id: &str) -> String {
    governance_id(tenant, incarnation, "key-registration", key_id)
}
pub fn key_revocation_id(tenant: &str, incarnation: &str, registration_id: &str) -> String {
    governance_id(tenant, incarnation, "key-revocation", registration_id)
}
pub fn approval_id(tenant: &str, incarnation: &str, policy_revision_id: &str) -> String {
    governance_id(tenant, incarnation, "policy-approval", policy_revision_id)
}
pub fn policy_revision_id(
    tenant: &str,
    incarnation: &str,
    target: &PromotionTarget,
    policy: &ResolvedPromotionPolicy,
) -> Result<String, CogniGraphError> {
    let identity = json!({
        "tenant": tenant,
        "tenant_incarnation": incarnation,
        "target": target,
        "policy_id": policy.policy_id,
        "policy_revision": policy.policy_revision,
    });
    Ok(canonical_digest(&identity)?
        .trim_start_matches("sha256:")
        .to_string())
}
pub(super) fn governance_id(tenant: &str, incarnation: &str, kind: &str, identity: &str) -> String {
    digest_bytes(format!("{tenant}\0{incarnation}\0{kind}\0{identity}").as_bytes())
        .trim_start_matches("sha256:")
        .to_string()
}
