//! Deployment contracts.

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticRepairDeploymentAction {
    Activate,
    Rollback,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticRepairDeploymentIntentPayload {
    pub requested_action: SemanticRepairDeploymentAction,
    pub target: PromotionTarget,
    pub semantic_repair_generation_id: String,
    pub semantic_repair_generation_digest: String,
    pub impact_digest: String,
    pub promotion_head_decision_id: String,
    pub promotion_head_projection_digest: String,
    pub candidate_digest: String,
    pub semantic_repair_revision_id: String,
    pub semantic_repair_revision_digest: String,
    pub semantic_repair_review_id: String,
    pub semantic_repair_review_digest: String,
    #[serde(deserialize_with = "deserialize_required_option")]
    pub expected_deployment_head_decision_id: Option<String>,
    #[serde(deserialize_with = "deserialize_required_option")]
    pub rollback_target_generation_id: Option<String>,
    pub reason: String,
    pub idempotency_key_hash: String,
    pub promoter_registration_id: String,
    pub promoter_principal_id: String,
    pub signed_at_ms: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticRepairDeploymentIntentSubmission {
    pub statement: GovernanceStatement<SemanticRepairDeploymentIntentPayload>,
    pub promoter_signature: SignatureEnvelope,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignedSemanticRepairDeploymentIntent {
    pub statement: GovernanceStatement<SemanticRepairDeploymentIntentPayload>,
    pub promoter_signature: SignatureEnvelope,
    pub promoter_registration: GovernanceKeyRecord,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticRepairDeploymentSelection {
    pub generation: u64,
    pub semantic_repair_generation_id: String,
    pub semantic_repair_generation_digest: String,
    pub target: PromotionTarget,
    pub candidate_digest: String,
    pub prior_semantic_repair_generation_id: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticRepairDeploymentDecision {
    #[serde(rename = "_key")]
    pub key: String,
    pub schema_version: u32,
    pub digest_algorithm: String,
    pub deployment_decision_id: String,
    pub deployment_decision_digest: String,
    pub tenant: String,
    pub tenant_incarnation: String,
    pub space_type: String,
    pub action: SemanticRepairDeploymentAction,
    pub target: PromotionTarget,
    pub semantic_repair_generation_id: String,
    pub semantic_repair_generation_digest: String,
    pub impact_digest: String,
    pub promotion_head_decision_id: String,
    pub promotion_head_projection_digest: String,
    pub candidate_digest: String,
    pub expected_deployment_head_decision_id: Option<String>,
    pub predecessor_deployment_decision_id: Option<String>,
    pub resulting_selection: SemanticRepairDeploymentSelection,
    pub deployment_intent: SignedSemanticRepairDeploymentIntent,
    pub actor: GovernanceActor,
    pub reason: String,
    pub created_at_ms: u64,
    pub idempotency_key_hash: String,
    pub request_digest: String,
}
impl SemanticRepairDeploymentDecision {
    pub fn public_value(&self) -> Result<Value, CogniGraphError> {
        let mut value = public_value(self)?;
        if let Some(registration) = value
            .pointer_mut("/deployment_intent/promoter_registration")
            .and_then(Value::as_object_mut)
        {
            registration.remove("_key");
            registration.remove("idempotency_key_hash");
        }
        Ok(value)
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticRepairDeploymentHead {
    #[serde(rename = "_key")]
    pub key: String,
    pub schema_version: u32,
    pub digest_algorithm: String,
    pub tenant: String,
    pub tenant_incarnation: String,
    pub space_type: String,
    pub applied_deployment_decision_id: String,
    pub selection: SemanticRepairDeploymentSelection,
    pub updated_at_ms: u64,
    pub projection_digest: String,
}
impl SemanticRepairDeploymentHead {
    pub fn public_value(&self) -> Result<Value, CogniGraphError> {
        public_value(self)
    }
}
