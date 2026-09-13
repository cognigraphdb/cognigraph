//! Policy contracts.

use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyRevisionPayload {
    pub policy_revision_id: String,
    pub target: PromotionTarget,
    pub resolved_policy: ResolvedPromotionPolicy,
    pub resolved_policy_digest: String,
    pub author_registration_id: String,
    pub author_principal_id: String,
    pub signed_at_ms: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreatePolicyRevisionRequest {
    pub statement: GovernanceStatement<PolicyRevisionPayload>,
    pub author_signature: SignatureEnvelope,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyRevisionRecord {
    #[serde(rename = "_key")]
    pub key: String,
    pub schema_version: u32,
    pub digest_algorithm: String,
    pub tenant: String,
    pub tenant_incarnation: String,
    pub policy_revision_id: String,
    pub target: PromotionTarget,
    pub resolved_policy: ResolvedPromotionPolicy,
    pub resolved_policy_digest: String,
    pub author_registration_id: String,
    pub author_registration_digest: String,
    pub author_principal_id: String,
    pub signed_at_ms: u64,
    pub author_signature: SignatureEnvelope,
    pub created_by: GovernanceActor,
    pub created_at_ms: u64,
    pub idempotency_key_hash: String,
    pub request_digest: String,
    pub policy_revision_digest: String,
}
impl PolicyRevisionRecord {
    pub fn public_value(&self) -> Result<Value, CogniGraphError> {
        public_value(self)
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyApprovalPayload {
    pub approval_id: String,
    pub policy_revision_id: String,
    pub policy_revision_digest: String,
    pub target: PromotionTarget,
    pub resolved_policy_digest: String,
    pub author_principal_id: String,
    pub approver_registration_id: String,
    pub approver_principal_id: String,
    pub decision: String,
    pub reason: String,
    pub signed_at_ms: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovePolicyRevisionRequest {
    pub statement: GovernanceStatement<PolicyApprovalPayload>,
    pub approval_signature: SignatureEnvelope,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyApprovalRecord {
    #[serde(rename = "_key")]
    pub key: String,
    pub schema_version: u32,
    pub digest_algorithm: String,
    pub tenant: String,
    pub tenant_incarnation: String,
    pub approval_id: String,
    pub policy_revision_id: String,
    pub policy_revision_digest: String,
    pub target: PromotionTarget,
    pub resolved_policy_digest: String,
    pub author_principal_id: String,
    pub approver_registration_id: String,
    pub approver_registration_digest: String,
    pub approver_principal_id: String,
    pub decision: String,
    pub reason: String,
    pub signed_at_ms: u64,
    pub approval_signature: SignatureEnvelope,
    pub approved_by: GovernanceActor,
    pub approved_at_ms: u64,
    pub idempotency_key_hash: String,
    pub request_digest: String,
    pub approval_digest: String,
}
impl PolicyApprovalRecord {
    pub fn public_value(&self) -> Result<Value, CogniGraphError> {
        public_value(self)
    }
}
