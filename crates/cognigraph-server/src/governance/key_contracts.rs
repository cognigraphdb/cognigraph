//! Key contracts.

use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GovernanceActor {
    pub user_key: String,
    pub username: String,
    pub role: Role,
}
impl GovernanceActor {
    pub fn from_user(user: User) -> Self {
        Self {
            user_key: user.key,
            username: user.username,
            role: user.role,
        }
    }

    pub(crate) fn require_role(&self, role: Role) -> Result<(), CogniGraphError> {
        validate_identifier("actor.user_key", &self.user_key)?;
        validate_identifier("actor.username", &self.username)?;
        if self.role != role {
            return Err(CogniGraphError::Forbidden(format!(
                "signed governance mutation requires the `{}` role",
                role_name(role)
            )));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyRegistrationPayload {
    pub registration_id: String,
    pub principal_id: String,
    pub subject_user_key: String,
    pub verification_key: VerificationKey,
    pub not_before_ms: u64,
    #[serde(deserialize_with = "deserialize_required_option")]
    pub not_after_ms: Option<u64>,
    pub signed_at_ms: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegisterGovernanceKeyRequest {
    pub statement: GovernanceStatement<KeyRegistrationPayload>,
    pub root_signature: SignatureEnvelope,
    pub possession_signature: SignatureEnvelope,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GovernanceKeyRecord {
    #[serde(rename = "_key")]
    pub key: String,
    pub schema_version: u32,
    pub digest_algorithm: String,
    pub tenant: String,
    pub tenant_incarnation: String,
    pub registration_id: String,
    pub principal_id: String,
    pub subject_user_key: String,
    pub verification_key: VerificationKey,
    pub public_key_digest: String,
    pub not_before_ms: u64,
    pub not_after_ms: Option<u64>,
    pub signed_at_ms: u64,
    pub root_key_id: String,
    pub root_signature: SignatureEnvelope,
    pub possession_signature: SignatureEnvelope,
    pub registered_by: GovernanceActor,
    pub registered_at_ms: u64,
    pub idempotency_key_hash: String,
    pub request_digest: String,
    pub registration_digest: String,
}
impl GovernanceKeyRecord {
    pub fn public_value(&self) -> Result<Value, CogniGraphError> {
        public_value(self)
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyRevocationPayload {
    pub revocation_id: String,
    pub registration_id: String,
    pub key_id: String,
    pub public_key_digest: String,
    pub reason: String,
    pub signed_at_ms: u64,
    pub effective_at_ms: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RevokeGovernanceKeyRequest {
    pub statement: GovernanceStatement<KeyRevocationPayload>,
    pub root_signature: SignatureEnvelope,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GovernanceKeyRevocation {
    #[serde(rename = "_key")]
    pub key: String,
    pub schema_version: u32,
    pub digest_algorithm: String,
    pub tenant: String,
    pub tenant_incarnation: String,
    pub revocation_id: String,
    pub registration_id: String,
    pub key_id: String,
    pub public_key_digest: String,
    pub reason: String,
    pub signed_at_ms: u64,
    pub effective_at_ms: u64,
    pub root_key_id: String,
    pub root_signature: SignatureEnvelope,
    pub revoked_by: GovernanceActor,
    pub recorded_at_ms: u64,
    pub idempotency_key_hash: String,
    pub request_digest: String,
    pub revocation_digest: String,
}
impl GovernanceKeyRevocation {
    pub fn public_value(&self) -> Result<Value, CogniGraphError> {
        public_value(self)
    }
}
