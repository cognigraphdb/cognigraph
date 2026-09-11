//! Decision contracts.

use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PromotionActor {
    pub user_key: String,
    pub username: String,
    pub role: String,
}
impl PromotionActor {
    pub fn validate(&self) -> Result<(), CogniGraphError> {
        validate_text("actor.user_key", &self.user_key, MAX_IDENTIFIER_BYTES)?;
        validate_text("actor.username", &self.username, MAX_IDENTIFIER_BYTES)?;
        if !matches!(self.role.as_str(), "admin" | "promoter") {
            return Err(CogniGraphError::Forbidden(
                "promotion mutations require an authenticated legacy Admin or M19 Promoter".into(),
            ));
        }
        Ok(())
    }

    pub(super) fn require_role(&self, expected: &str) -> Result<(), CogniGraphError> {
        self.validate()?;
        if self.role != expected {
            return Err(CogniGraphError::Forbidden(format!(
                "promotion mutation requires the `{expected}` role"
            )));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromotionAction {
    Promote,
    Reject,
    Blocked,
    Rollback,
}
#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PromoteRequest {
    pub expected_head_decision_id: Option<String>,
    pub reason: String,
}
#[cfg(test)]
impl<'de> Deserialize<'de> for PromoteRequest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            expected_head_decision_id: Option<String>,
            reason: String,
        }
        let value = Value::deserialize(deserializer)?;
        if !value
            .as_object()
            .is_some_and(|object| object.contains_key("expected_head_decision_id"))
        {
            return Err(serde::de::Error::missing_field("expected_head_decision_id"));
        }
        let wire: Wire = serde_json::from_value(value).map_err(serde::de::Error::custom)?;
        Ok(Self {
            expected_head_decision_id: wire.expected_head_decision_id,
            reason: wire.reason,
        })
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RollbackRequest {
    pub expected_head_decision_id: String,
    pub to_evidence_id: String,
    pub reason: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PromotionSelection {
    pub generation: u64,
    pub evidence_id: String,
    pub evidence_digest: String,
    pub candidate_digest: String,
    pub policy_digest: String,
    pub prior_evidence_id: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromotionDecision {
    #[serde(rename = "_key")]
    pub key: String,
    pub schema_version: u32,
    pub digest_algorithm: String,
    pub id: String,
    pub tenant: String,
    pub tenant_incarnation: String,
    pub target: PromotionTarget,
    pub action: PromotionAction,
    pub actor: PromotionActor,
    pub reason: String,
    pub created_at_ms: u64,
    pub idempotency_key_hash: String,
    pub request_digest: String,
    pub decision_digest: String,
    pub evidence_id: String,
    pub evidence_digest: String,
    pub policy_digest: String,
    pub gate_assessment_digest: String,
    pub expected_head_decision_id: Option<String>,
    pub predecessor_decision_id: Option<String>,
    pub resulting_selection: Option<PromotionSelection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub governance: Option<crate::governance::SignedPromotionIntent>,
}
impl PromotionDecision {
    pub fn public_value(&self) -> Result<Value, CogniGraphError> {
        let mut value = public_record_value(self)?;
        if let Some(registration) = value
            .get_mut("governance")
            .and_then(|governance| governance.get_mut("promoter_registration"))
            .and_then(Value::as_object_mut)
        {
            registration.remove("_key");
            registration.remove("idempotency_key_hash");
        }
        Ok(value)
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromotionHead {
    #[serde(rename = "_key")]
    pub key: String,
    pub schema_version: u32,
    pub digest_algorithm: String,
    pub tenant: String,
    pub tenant_incarnation: String,
    pub target: PromotionTarget,
    pub applied_decision_id: String,
    pub selection: PromotionSelection,
    pub updated_at_ms: u64,
    pub projection_digest: String,
}
impl PromotionHead {
    pub fn public_value(&self) -> Result<Value, CogniGraphError> {
        public_record_value(self)
    }
}
