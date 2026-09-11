//! Intent contracts.

use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PromotionIntentPayload {
    pub requested_action: String,
    pub target: PromotionTarget,
    pub evidence_id: String,
    pub evidence_digest: String,
    #[serde(deserialize_with = "deserialize_required_option")]
    pub policy_revision_id: Option<String>,
    #[serde(deserialize_with = "deserialize_required_option")]
    pub policy_revision_digest: Option<String>,
    #[serde(deserialize_with = "deserialize_required_option")]
    pub approval_id: Option<String>,
    #[serde(deserialize_with = "deserialize_required_option")]
    pub approval_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_authority_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consumption_authority_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub derivation_authority_digest: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_non_null_option"
    )]
    pub preparation_authority_digest: Option<String>,
    pub gate_assessment_digest: String,
    #[serde(deserialize_with = "deserialize_required_option")]
    pub expected_head_decision_id: Option<String>,
    #[serde(deserialize_with = "deserialize_required_option")]
    pub rollback_target_evidence_id: Option<String>,
    pub reason: String,
    pub idempotency_key_hash: String,
    pub promoter_registration_id: String,
    pub promoter_principal_id: String,
    pub signed_at_ms: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignedPromotionIntent {
    pub statement: GovernanceStatement<PromotionIntentPayload>,
    pub promoter_signature: SignatureEnvelope,
    pub promoter_registration: GovernanceKeyRecord,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PromotionIntentSubmission {
    pub statement: GovernanceStatement<PromotionIntentPayload>,
    pub promoter_signature: SignatureEnvelope,
}
#[derive(Debug, Clone, Serialize)]
pub struct GovernanceMutation<T> {
    pub record: T,
    pub replayed: bool,
}
/// Complete immutable M19 authority for one tenant incarnation. Recovery and
/// snapshot preflight validate promotion records against this union rather
/// than querying only the destination store, so an additive restore cannot
/// smuggle references that are valid only in the incoming half.
#[derive(Default)]
pub(super) struct GovernanceAuthority {
    pub(super) keys: BTreeMap<String, GovernanceKeyRecord>,
    pub(super) revocations: BTreeMap<String, GovernanceKeyRevocation>,
    pub(super) policies: BTreeMap<String, PolicyRevisionRecord>,
    pub(super) approvals: BTreeMap<String, PolicyApprovalRecord>,
    pub(super) artifacts: BTreeMap<String, ArtifactAttestationRecord>,
    pub(super) semantic_repair_revisions: BTreeMap<String, SemanticRepairRevisionRecord>,
    pub(super) semantic_repair_reviews: BTreeMap<String, SemanticRepairReviewRecord>,
}
