//! Governance binding.

use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyGovernanceBinding {
    pub root_key_id: String,
    pub author_registration_id: String,
    pub author_registration_digest: String,
    pub policy_revision_id: String,
    pub policy_revision_digest: String,
    pub resolved_policy_digest: String,
    pub approval_id: String,
    pub approval_digest: String,
    pub approver_registration_id: String,
    pub approver_registration_digest: String,
    pub author_principal_id: String,
    pub approver_principal_id: String,
}
impl PolicyGovernanceBinding {
    pub(crate) fn validate(&self) -> Result<(), CogniGraphError> {
        validate_text("root_key_id", &self.root_key_id, MAX_IDENTIFIER_BYTES)?;
        validate_record_id("author_registration_id", &self.author_registration_id)?;
        validate_digest(
            "author_registration_digest",
            &self.author_registration_digest,
        )?;
        validate_record_id("policy_revision_id", &self.policy_revision_id)?;
        validate_digest("policy_revision_digest", &self.policy_revision_digest)?;
        validate_digest("resolved_policy_digest", &self.resolved_policy_digest)?;
        validate_record_id("approval_id", &self.approval_id)?;
        validate_digest("approval_digest", &self.approval_digest)?;
        validate_record_id("approver_registration_id", &self.approver_registration_id)?;
        validate_digest(
            "approver_registration_digest",
            &self.approver_registration_digest,
        )?;
        validate_text(
            "author_principal_id",
            &self.author_principal_id,
            MAX_IDENTIFIER_BYTES,
        )?;
        validate_text(
            "approver_principal_id",
            &self.approver_principal_id,
            MAX_IDENTIFIER_BYTES,
        )?;
        if !is_nfc(&self.author_principal_id) || !is_nfc(&self.approver_principal_id) {
            return Err(validation(
                "governance principal ids must be NFC-normalized",
            ));
        }
        if self.author_principal_id == self.approver_principal_id {
            return Err(validation(
                "policy author and approver principals must be distinct",
            ));
        }
        Ok(())
    }
}
