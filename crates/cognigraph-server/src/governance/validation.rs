//! Validation.

use super::*;

pub(super) fn validate_key_registration_payload(
    payload: &KeyRegistrationPayload,
    now: u64,
) -> Result<(), CogniGraphError> {
    validate_identifier("registration_id", &payload.registration_id)?;
    validate_identifier("principal_id", &payload.principal_id)?;
    validate_identifier("subject_user_key", &payload.subject_user_key)?;
    if payload.verification_key.purpose == KeyPurpose::TrustRoot {
        return Err(validation(
            "the externally configured trust root cannot be registered as a tenant principal",
        ));
    }
    payload
        .verification_key
        .validate()
        .map_err(governance_validation_error)?;
    validate_signed_at(payload.signed_at_ms, now)?;
    if payload.not_before_ms == 0
        || payload.not_before_ms > now.saturating_add(MAX_CLOCK_SKEW_MS)
        || payload.signed_at_ms < payload.not_before_ms
        || payload
            .not_after_ms
            .is_some_and(|until| until <= payload.not_before_ms)
        || payload
            .not_after_ms
            .is_some_and(|until| payload.signed_at_ms >= until)
        || payload.not_after_ms.is_some_and(|until| until <= now)
    {
        return Err(validation("governance key validity interval is invalid"));
    }
    Ok(())
}
pub(super) fn validate_statement_scope<T>(
    statement: &GovernanceStatement<T>,
    domain: &str,
    tenant: &str,
    incarnation: &str,
) -> Result<(), CogniGraphError> {
    if statement.schema_version != cognigraph_governance::GOVERNANCE_SCHEMA_VERSION
        || statement.domain != domain
        || statement.tenant != tenant
        || statement.tenant_incarnation != incarnation
    {
        return Err(validation(
            "signed governance statement domain or tenant scope mismatch",
        ));
    }
    Ok(())
}
pub(super) fn promotion_intent_domain(payload: &PromotionIntentPayload) -> &'static str {
    if payload.preparation_authority_digest.is_some() {
        M23_PROMOTION_INTENT_DOMAIN
    } else if payload.derivation_authority_digest.is_some() {
        M22_PROMOTION_INTENT_DOMAIN
    } else if payload.consumption_authority_digest.is_some() {
        M21_PROMOTION_INTENT_DOMAIN
    } else {
        PROMOTION_INTENT_DOMAIN
    }
}
pub(super) fn validate_signed_at(signed_at_ms: u64, now: u64) -> Result<(), CogniGraphError> {
    if signed_at_ms == 0 || signed_at_ms > now.saturating_add(MAX_CLOCK_SKEW_MS) {
        return Err(validation(
            "governance signed_at_ms is zero or more than five minutes in the future",
        ));
    }
    Ok(())
}
pub(super) fn require_key_actor(
    key: &GovernanceKeyRecord,
    actor: &GovernanceActor,
    principal_id: &str,
) -> Result<(), CogniGraphError> {
    if key.subject_user_key != actor.user_key
        || key.principal_id != principal_id
        || key.principal_id.trim().is_empty()
    {
        return Err(CogniGraphError::Forbidden(
            "authenticated user does not own the signing principal".into(),
        ));
    }
    Ok(())
}
/// Validate authority as it existed when the server accepted a signed record.
/// A revocation is prospective: it blocks an acceptance only when the
/// revocation had already been recorded and was effective at that acceptance
/// time. A later revocation therefore does not rewrite valid history.
pub(crate) fn validate_historical_key_use(
    key: &GovernanceKeyRecord,
    purpose: KeyPurpose,
    signed_at_ms: u64,
    accepted_at_ms: u64,
    revocation: Option<&GovernanceKeyRevocation>,
) -> Result<(), CogniGraphError> {
    if key.verification_key.purpose != purpose
        || accepted_at_ms == 0
        || key.registered_at_ms > accepted_at_ms
        || signed_at_ms < key.not_before_ms
        || accepted_at_ms < key.not_before_ms
        || key
            .not_after_ms
            .is_some_and(|until| signed_at_ms >= until || accepted_at_ms >= until)
        || signed_at_ms > accepted_at_ms.saturating_add(MAX_CLOCK_SKEW_MS)
    {
        return Err(conflict(
            "signed governance record was not accepted within its key validity interval",
        ));
    }
    if revocation.is_some_and(|revocation| {
        revocation.recorded_at_ms <= accepted_at_ms && revocation.effective_at_ms <= accepted_at_ms
    }) {
        return Err(conflict(
            "signed governance record was accepted after its key was revoked",
        ));
    }
    Ok(())
}
pub(super) fn insert_unique<T: PartialEq>(
    records: &mut BTreeMap<String, T>,
    id: String,
    record: T,
    label: &str,
) -> Result<(), CogniGraphError> {
    match records.entry(id) {
        std::collections::btree_map::Entry::Vacant(entry) => {
            entry.insert(record);
        }
        std::collections::btree_map::Entry::Occupied(entry) if entry.get() == &record => {}
        std::collections::btree_map::Entry::Occupied(_) => {
            return Err(conflict(format!(
                "duplicate {label} identity has divergent records"
            )));
        }
    }
    Ok(())
}
pub(super) fn snapshot_documents<'a>(
    snapshot: &'a Value,
    collection: &str,
) -> Result<Option<&'a serde_json::Map<String, Value>>, CogniGraphError> {
    let Some(collection_value) = snapshot.pointer(&format!("/collections/{collection}")) else {
        return Ok(None);
    };
    collection_value
        .get("documents")
        .and_then(Value::as_object)
        .map(Some)
        .ok_or_else(|| {
            conflict(format!(
                "snapshot governance collection `{collection}` has malformed documents"
            ))
        })
}
pub(super) fn deserialize_stored<T: DeserializeOwned>(
    mut value: Value,
) -> Result<T, CogniGraphError> {
    if let Some(fields) = value.as_object_mut() {
        fields.remove("_id");
        fields.remove("_rev");
        fields.remove("created_at");
        fields.remove("updated_at");
    }
    serde_json::from_value(value).map_err(CogniGraphError::from)
}
pub(super) fn public_value<T: Serialize>(record: &T) -> Result<Value, CogniGraphError> {
    let mut value = serde_json::to_value(record)?;
    if let Some(fields) = value.as_object_mut() {
        fields.remove("_key");
        fields.remove("idempotency_key_hash");
    }
    Ok(value)
}
pub(super) fn role_name(role: Role) -> &'static str {
    match role {
        Role::Admin => "admin",
        Role::PolicyAuthor => "policy-author",
        Role::PolicyApprover => "policy-approver",
        Role::Promoter => "promoter",
        Role::ArtifactAttestor => "artifact-attestor",
        Role::Editor => "editor",
        Role::Viewer => "viewer",
        Role::ScriptRunner => "script-runner",
        Role::HostAdmin => "host-admin",
    }
}
pub(super) fn role_matches_purpose(role: Role, purpose: KeyPurpose) -> bool {
    matches!(
        (role, purpose),
        (Role::PolicyAuthor, KeyPurpose::PolicyAuthor)
            | (Role::PolicyApprover, KeyPurpose::PolicyApprover)
            | (Role::Promoter, KeyPurpose::Promoter)
            | (Role::ArtifactAttestor, KeyPurpose::ArtifactAttestor)
    )
}
pub(super) fn key_registration_domain(purpose: KeyPurpose) -> &'static str {
    if purpose == KeyPurpose::ArtifactAttestor {
        M20_KEY_REGISTRATION_DOMAIN
    } else {
        KEY_REGISTRATION_DOMAIN
    }
}
pub(super) fn validate_identifier(label: &str, value: &str) -> Result<(), CogniGraphError> {
    if value.trim().is_empty()
        || value.len() > MAX_IDENTIFIER_BYTES
        || value.chars().any(char::is_control)
        || !is_nfc(value)
    {
        return Err(validation(format!(
            "{label} must be non-empty, NFC-normalized, control-free, and at most {MAX_IDENTIFIER_BYTES} bytes"
        )));
    }
    Ok(())
}
pub(super) fn deserialize_required_option<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}
pub(super) fn deserialize_non_null_option<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    // Missing preserves historical intent generations. If the new field is
    // present, deserialize its inner value directly so a signed wire cannot
    // smuggle unbound JSON null that canonicalizes back to omission.
    T::deserialize(deserializer).map(Some)
}
pub(super) fn is_digest(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}
pub(super) fn governance_config_error(
    error: cognigraph_governance::GovernanceError,
) -> CogniGraphError {
    CogniGraphError::ValidationError(format!("invalid governance root: {error}"))
}
pub(super) fn governance_validation_error(
    error: cognigraph_governance::GovernanceError,
) -> CogniGraphError {
    validation(format!("invalid governance key: {error}"))
}
pub(super) fn signature_error(error: cognigraph_governance::GovernanceError) -> CogniGraphError {
    CogniGraphError::Forbidden(format!("governance signature rejected: {error}"))
}
pub(super) fn stored_signature_error(
    error: cognigraph_governance::GovernanceError,
) -> CogniGraphError {
    conflict(format!("stored governance signature is invalid: {error}"))
}
pub(super) fn validation(message: impl Into<String>) -> CogniGraphError {
    CogniGraphError::ValidationError(message.into())
}
pub(super) fn conflict(message: impl Into<String>) -> CogniGraphError {
    CogniGraphError::DocumentConflict(message.into())
}
pub(super) fn not_found(collection: &str, id: &str) -> CogniGraphError {
    CogniGraphError::DocumentNotFound {
        collection: collection.into(),
        key: id.into(),
    }
}
