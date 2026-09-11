//! Reusable cryptographic primitives for CogniGraph governance records.
//!
//! This crate deliberately owns no storage and no authorization policy. It
//! provides one strict canonical JSON representation, locally held Ed25519
//! signing material, portable public verification keys, and domain-separated
//! governance statements. Private key material belongs in CLI/operator
//! custody and must never be persisted by the server.

use std::collections::BTreeMap;
use std::fmt;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;

pub const CANONICAL_DIGEST_ALGORITHM: &str = "cognigraph-canonical-json-nfc-v1+sha256";
pub const GOVERNANCE_SCHEMA_VERSION: u32 = 1;
pub const SIGNATURE_ALGORITHM: &str = "ed25519";

const SIGNING_PREFIX: &[u8] = b"cognigraph-governance-signature-v1\0";
const PUBLIC_KEY_BYTES: usize = 32;
const SECRET_KEY_BYTES: usize = 32;
const SIGNATURE_BYTES: usize = 64;
const MAX_SCOPE_BYTES: usize = 256;

pub type Result<T> = std::result::Result<T, GovernanceError>;

#[derive(Debug, thiserror::Error)]
pub enum GovernanceError {
    #[error("canonical JSON serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("canonical JSON does not permit floating-point numbers")]
    FloatingPoint,
    #[error("canonical JSON object keys collide after NFC normalization")]
    NfcKeyCollision,
    #[error("{0} is invalid")]
    InvalidField(&'static str),
    #[error("unsupported governance schema version {0}")]
    UnsupportedSchema(u32),
    #[error("unsupported signature algorithm `{0}`")]
    UnsupportedAlgorithm(String),
    #[error("{0} is not canonical unpadded base64url")]
    InvalidBase64(&'static str),
    #[error("{field} must decode to exactly {expected} bytes")]
    InvalidLength {
        field: &'static str,
        expected: usize,
    },
    #[error("governance key is weak")]
    WeakKey,
    #[error("governance public key does not match its key id")]
    KeyIdMismatch,
    #[error("governance public key does not match its secret key")]
    KeyMaterialMismatch,
    #[error("governance statement digest mismatch")]
    DigestMismatch,
    #[error("governance signature verification failed")]
    InvalidSignature,
    #[error("governance statement {0} does not match the verification context")]
    ContextMismatch(&'static str),
    #[error("governance key purpose does not match the required purpose")]
    PurposeMismatch,
    #[error("operating-system randomness failed: {0}")]
    Random(String),
}

/// The one operation class for which a governance key may be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyPurpose {
    /// Out-of-band trust anchor used only to certify or revoke governance
    /// keys. It is not a tenant principal and cannot author, approve, or
    /// promote policy-controlled artifacts.
    TrustRoot,
    PolicyAuthor,
    PolicyApprover,
    Promoter,
    /// Attests the content identity of external evaluation artifacts. This
    /// purpose is independent from policy authorship, approval, and promotion.
    ArtifactAttestor,
}

/// Domain-separated content signed by governance participants.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GovernanceStatement<T> {
    pub schema_version: u32,
    pub domain: String,
    pub tenant: String,
    pub tenant_incarnation: String,
    pub payload: T,
}

impl<T> GovernanceStatement<T> {
    pub fn new(
        domain: impl Into<String>,
        tenant: impl Into<String>,
        tenant_incarnation: impl Into<String>,
        payload: T,
    ) -> Self {
        Self {
            schema_version: GOVERNANCE_SCHEMA_VERSION,
            domain: domain.into(),
            tenant: tenant.into(),
            tenant_incarnation: tenant_incarnation.into(),
            payload,
        }
    }

    fn validate_context(&self) -> Result<()> {
        validate_schema(self.schema_version)?;
        validate_scope("domain", &self.domain)?;
        validate_scope("tenant", &self.tenant)?;
        validate_scope("tenant_incarnation", &self.tenant_incarnation)
    }
}

/// Detached proof for one canonical [`GovernanceStatement`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignatureEnvelope {
    pub schema_version: u32,
    pub algorithm: String,
    pub key_id: String,
    pub statement_digest: String,
    pub signature: String,
}

/// Public, server-safe half of one purpose-bound governance key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationKey {
    pub schema_version: u32,
    pub algorithm: String,
    pub key_id: String,
    pub purpose: KeyPurpose,
    pub public_key: String,
}

impl VerificationKey {
    pub fn validate(&self) -> Result<()> {
        validate_schema(self.schema_version)?;
        validate_algorithm(&self.algorithm)?;
        let key = decode_verifying_key(&self.public_key)?;
        if key_id_for_bytes(&key.to_bytes()) != self.key_id {
            return Err(GovernanceError::KeyIdMismatch);
        }
        Ok(())
    }

    /// Verify a statement against its complete expected scope and key purpose.
    #[allow(clippy::too_many_arguments)]
    pub fn verify<T: Serialize>(
        &self,
        statement: &GovernanceStatement<T>,
        envelope: &SignatureEnvelope,
        expected_domain: &str,
        expected_tenant: &str,
        expected_tenant_incarnation: &str,
        expected_purpose: KeyPurpose,
    ) -> Result<()> {
        self.validate()?;
        if self.purpose != expected_purpose {
            return Err(GovernanceError::PurposeMismatch);
        }
        statement.validate_context()?;
        for (actual, expected, field) in [
            (statement.domain.as_str(), expected_domain, "domain"),
            (statement.tenant.as_str(), expected_tenant, "tenant"),
            (
                statement.tenant_incarnation.as_str(),
                expected_tenant_incarnation,
                "tenant_incarnation",
            ),
        ] {
            if actual != expected {
                return Err(GovernanceError::ContextMismatch(field));
            }
        }
        validate_schema(envelope.schema_version)?;
        validate_algorithm(&envelope.algorithm)?;
        if envelope.key_id != self.key_id {
            return Err(GovernanceError::KeyIdMismatch);
        }

        let canonical = canonical_json_bytes(statement)?;
        let digest = digest_bytes(&canonical);
        validate_digest(&envelope.statement_digest)?;
        if envelope.statement_digest != digest {
            return Err(GovernanceError::DigestMismatch);
        }

        let signature_bytes = decode_exact("signature", &envelope.signature, SIGNATURE_BYTES)?;
        let signature = Signature::from_bytes(
            &signature_bytes
                .try_into()
                .expect("signature length was checked"),
        );
        let verifying_key = decode_verifying_key(&self.public_key)?;
        verifying_key
            .verify_strict(&signing_bytes(&canonical), &signature)
            .map_err(|_| GovernanceError::InvalidSignature)
    }
}

/// JSON-serializable local signing material.
///
/// The secret seed is intentionally serializable so the CLI can read/write a
/// local key file. It is private, omitted from `Debug`, and has no accessor.
/// Server APIs must accept [`VerificationKey`], never this type.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SigningKeyMaterial {
    pub schema_version: u32,
    pub algorithm: String,
    pub key_id: String,
    pub purpose: KeyPurpose,
    pub public_key: String,
    secret_key: String,
}

impl fmt::Debug for SigningKeyMaterial {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SigningKeyMaterial")
            .field("schema_version", &self.schema_version)
            .field("algorithm", &self.algorithm)
            .field("key_id", &self.key_id)
            .field("purpose", &self.purpose)
            .field("public_key", &self.public_key)
            .field("secret_key", &"[REDACTED]")
            .finish()
    }
}

impl SigningKeyMaterial {
    /// Generate a fresh Ed25519 seed directly from `getrandom`.
    pub fn generate(purpose: KeyPurpose) -> Result<Self> {
        // Weak generated material is fantastically unlikely, but generation is
        // still fail-closed and bounded rather than returning it.
        for _ in 0..8 {
            let mut secret = [0u8; SECRET_KEY_BYTES];
            getrandom::fill(&mut secret)
                .map_err(|error| GovernanceError::Random(format!("{error:?}")))?;
            if secret.iter().all(|byte| *byte == 0) {
                continue;
            }
            let signing_key = SigningKey::from_bytes(&secret);
            let verifying_key = signing_key.verifying_key();
            if verifying_key.is_weak() {
                continue;
            }
            let public = verifying_key.to_bytes();
            return Ok(Self {
                schema_version: GOVERNANCE_SCHEMA_VERSION,
                algorithm: SIGNATURE_ALGORITHM.into(),
                key_id: key_id_for_bytes(&public),
                purpose,
                public_key: URL_SAFE_NO_PAD.encode(public),
                secret_key: URL_SAFE_NO_PAD.encode(secret),
            });
        }
        Err(GovernanceError::WeakKey)
    }

    pub fn validate(&self) -> Result<()> {
        self.verification_key()?.validate()?;
        let secret = decode_exact("secret_key", &self.secret_key, SECRET_KEY_BYTES)?;
        if secret.iter().all(|byte| *byte == 0) {
            return Err(GovernanceError::WeakKey);
        }
        let signing_key =
            SigningKey::from_bytes(&secret.try_into().expect("secret key length was checked"));
        if signing_key.verifying_key().to_bytes()
            != decode_verifying_key(&self.public_key)?.to_bytes()
        {
            return Err(GovernanceError::KeyMaterialMismatch);
        }
        Ok(())
    }

    pub fn verification_key(&self) -> Result<VerificationKey> {
        let key = VerificationKey {
            schema_version: self.schema_version,
            algorithm: self.algorithm.clone(),
            key_id: self.key_id.clone(),
            purpose: self.purpose,
            public_key: self.public_key.clone(),
        };
        key.validate()?;
        Ok(key)
    }

    pub fn sign<T: Serialize>(
        &self,
        statement: &GovernanceStatement<T>,
    ) -> Result<SignatureEnvelope> {
        self.validate()?;
        statement.validate_context()?;
        let canonical = canonical_json_bytes(statement)?;
        let secret = decode_exact("secret_key", &self.secret_key, SECRET_KEY_BYTES)?;
        let signing_key =
            SigningKey::from_bytes(&secret.try_into().expect("secret key length was checked"));
        let signature = signing_key.sign(&signing_bytes(&canonical));
        Ok(SignatureEnvelope {
            schema_version: GOVERNANCE_SCHEMA_VERSION,
            algorithm: SIGNATURE_ALGORITHM.into(),
            key_id: self.key_id.clone(),
            statement_digest: digest_bytes(&canonical),
            signature: URL_SAFE_NO_PAD.encode(signature.to_bytes()),
        })
    }
}

/// Canonical NFC JSON bytes compatible with the M18 canonicalizer for the
/// accepted integer-only subset. Object keys are NFC-normalized and sorted;
/// strings are NFC-normalized; array order is retained. Floats are rejected.
pub fn canonical_json_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    let value = serde_json::to_value(value)?;
    Ok(serde_json::to_vec(&canonicalize(&value)?)?)
}

pub fn canonical_digest<T: Serialize>(value: &T) -> Result<String> {
    Ok(digest_bytes(&canonical_json_bytes(value)?))
}

pub fn key_id_from_public_key(public_key: &str) -> Result<String> {
    Ok(key_id_for_bytes(
        &decode_verifying_key(public_key)?.to_bytes(),
    ))
}

fn canonicalize(value: &Value) -> Result<Value> {
    match value {
        Value::Null | Value::Bool(_) => Ok(value.clone()),
        Value::Number(number) if number.is_f64() => Err(GovernanceError::FloatingPoint),
        Value::Number(_) => Ok(value.clone()),
        Value::String(value) => Ok(Value::String(value.nfc().collect())),
        Value::Array(values) => values
            .iter()
            .map(canonicalize)
            .collect::<Result<Vec<_>>>()
            .map(Value::Array),
        Value::Object(values) => {
            let mut normalized = BTreeMap::new();
            for (key, value) in values {
                let key = key.nfc().collect::<String>();
                if normalized.insert(key, canonicalize(value)?).is_some() {
                    return Err(GovernanceError::NfcKeyCollision);
                }
            }
            Ok(Value::Object(normalized.into_iter().collect()))
        }
    }
}

/// Return the strict algorithm-prefixed SHA-256 content address for exact bytes.
pub fn digest_bytes(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(64);
    for byte in digest {
        use fmt::Write;
        let _ = write!(hex, "{byte:02x}");
    }
    format!("sha256:{hex}")
}

fn key_id_for_bytes(public_key: &[u8; PUBLIC_KEY_BYTES]) -> String {
    format!("ed25519:{}", digest_bytes(public_key))
}

fn signing_bytes(canonical: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(SIGNING_PREFIX.len() + canonical.len());
    bytes.extend_from_slice(SIGNING_PREFIX);
    bytes.extend_from_slice(canonical);
    bytes
}

fn decode_verifying_key(encoded: &str) -> Result<VerifyingKey> {
    let bytes = decode_exact("public_key", encoded, PUBLIC_KEY_BYTES)?;
    let key = VerifyingKey::from_bytes(&bytes.try_into().expect("public key length was checked"))
        .map_err(|_| GovernanceError::InvalidField("public_key"))?;
    if key.is_weak() {
        return Err(GovernanceError::WeakKey);
    }
    Ok(key)
}

fn decode_exact(field: &'static str, encoded: &str, expected: usize) -> Result<Vec<u8>> {
    if encoded.is_empty() || encoded.contains('=') {
        return Err(GovernanceError::InvalidBase64(field));
    }
    let bytes = URL_SAFE_NO_PAD
        .decode(encoded)
        .map_err(|_| GovernanceError::InvalidBase64(field))?;
    if URL_SAFE_NO_PAD.encode(&bytes) != encoded {
        return Err(GovernanceError::InvalidBase64(field));
    }
    if bytes.len() != expected {
        return Err(GovernanceError::InvalidLength { field, expected });
    }
    Ok(bytes)
}

fn validate_schema(schema_version: u32) -> Result<()> {
    if schema_version != GOVERNANCE_SCHEMA_VERSION {
        return Err(GovernanceError::UnsupportedSchema(schema_version));
    }
    Ok(())
}

fn validate_algorithm(algorithm: &str) -> Result<()> {
    if algorithm != SIGNATURE_ALGORITHM {
        return Err(GovernanceError::UnsupportedAlgorithm(algorithm.into()));
    }
    Ok(())
}

fn validate_scope(field: &'static str, value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > MAX_SCOPE_BYTES
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b':'))
    {
        return Err(GovernanceError::InvalidField(field));
    }
    Ok(())
}

fn validate_digest(digest: &str) -> Result<()> {
    let Some(hex) = digest.strip_prefix("sha256:") else {
        return Err(GovernanceError::InvalidField("statement_digest"));
    };
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(GovernanceError::InvalidField("statement_digest"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Map, json};

    const DOMAIN: &str = "cognigraph.policy-revision.v1";
    const TENANT: &str = "tenant-a";
    const INCARNATION: &str = "incarnation-1";

    fn statement() -> GovernanceStatement<Value> {
        GovernanceStatement::new(
            DOMAIN,
            TENANT,
            INCARNATION,
            json!({"policy_id": "clinical", "threshold": {"numerator": 9, "denominator": 10}}),
        )
    }

    fn assert_verifies(
        material: &SigningKeyMaterial,
        statement: &GovernanceStatement<Value>,
        envelope: &SignatureEnvelope,
    ) {
        material
            .verification_key()
            .unwrap()
            .verify(
                statement,
                envelope,
                DOMAIN,
                TENANT,
                INCARNATION,
                material.purpose,
            )
            .unwrap();
    }

    #[test]
    fn canonical_json_matches_m18_ordering_and_nfc() {
        let value = json!({"b": 2, "a": "e\u{301}"});
        let bytes = canonical_json_bytes(&value).unwrap();
        assert_eq!(String::from_utf8(bytes).unwrap(), "{\"a\":\"é\",\"b\":2}");
        assert_eq!(
            canonical_digest(&value).unwrap(),
            digest_bytes("{\"a\":\"é\",\"b\":2}".as_bytes())
        );
    }

    #[test]
    fn canonical_json_rejects_floats_at_any_depth() {
        for value in [
            json!(1.5),
            json!([1, 2.0]),
            json!({"nested": {"float": 3.25}}),
        ] {
            assert!(matches!(
                canonical_json_bytes(&value),
                Err(GovernanceError::FloatingPoint)
            ));
        }
    }

    #[test]
    fn canonical_json_rejects_nfc_colliding_keys() {
        let mut object = Map::new();
        object.insert("é".into(), json!(1));
        object.insert("e\u{301}".into(), json!(2));
        assert!(matches!(
            canonical_json_bytes(&Value::Object(object)),
            Err(GovernanceError::NfcKeyCollision)
        ));
    }

    #[test]
    fn generated_material_round_trips_without_debug_secret_exposure() {
        for purpose in [
            KeyPurpose::TrustRoot,
            KeyPurpose::PolicyAuthor,
            KeyPurpose::PolicyApprover,
            KeyPurpose::Promoter,
            KeyPurpose::ArtifactAttestor,
        ] {
            let material = SigningKeyMaterial::generate(purpose).unwrap();
            material.validate().unwrap();
            assert!(material.key_id.starts_with("ed25519:sha256:"));
            assert_eq!(material.key_id.len(), "ed25519:sha256:".len() + 64);
            assert_eq!(
                key_id_from_public_key(&material.public_key).unwrap(),
                material.key_id
            );
            let encoded = serde_json::to_string(&material).unwrap();
            let decoded: SigningKeyMaterial = serde_json::from_str(&encoded).unwrap();
            assert_eq!(material, decoded);
            let debug = format!("{material:?}");
            assert!(debug.contains("[REDACTED]"));
            assert!(!debug.contains(&material.secret_key));
        }
    }

    #[test]
    fn key_purposes_use_stable_snake_case_wire_names() {
        for (purpose, expected) in [
            (KeyPurpose::TrustRoot, "\"trust_root\""),
            (KeyPurpose::PolicyAuthor, "\"policy_author\""),
            (KeyPurpose::PolicyApprover, "\"policy_approver\""),
            (KeyPurpose::Promoter, "\"promoter\""),
            (KeyPurpose::ArtifactAttestor, "\"artifact_attestor\""),
        ] {
            assert_eq!(serde_json::to_string(&purpose).unwrap(), expected);
            assert_eq!(
                serde_json::from_str::<KeyPurpose>(expected).unwrap(),
                purpose
            );
        }
    }

    #[test]
    fn sign_and_strict_verify_succeeds() {
        let material = SigningKeyMaterial::generate(KeyPurpose::PolicyAuthor).unwrap();
        let statement = statement();
        let envelope = material.sign(&statement).unwrap();
        assert_verifies(&material, &statement, &envelope);
        assert_eq!(
            envelope.statement_digest,
            canonical_digest(&statement).unwrap()
        );
    }

    #[test]
    fn canonical_nfc_equivalents_share_signatures() {
        let material = SigningKeyMaterial::generate(KeyPurpose::PolicyAuthor).unwrap();
        let decomposed =
            GovernanceStatement::new(DOMAIN, TENANT, INCARNATION, json!({"label": "e\u{301}"}));
        let composed = GovernanceStatement::new(DOMAIN, TENANT, INCARNATION, json!({"label": "é"}));
        let envelope = material.sign(&decomposed).unwrap();
        material
            .verification_key()
            .unwrap()
            .verify(
                &composed,
                &envelope,
                DOMAIN,
                TENANT,
                INCARNATION,
                KeyPurpose::PolicyAuthor,
            )
            .unwrap();
    }

    #[test]
    fn payload_tampering_and_another_key_fail() {
        let material = SigningKeyMaterial::generate(KeyPurpose::PolicyAuthor).unwrap();
        let other = SigningKeyMaterial::generate(KeyPurpose::PolicyAuthor).unwrap();
        let statement = statement();
        let envelope = material.sign(&statement).unwrap();
        let mut tampered = statement.clone();
        tampered.payload["policy_id"] = json!("replaced");
        assert!(
            material
                .verification_key()
                .unwrap()
                .verify(
                    &tampered,
                    &envelope,
                    DOMAIN,
                    TENANT,
                    INCARNATION,
                    KeyPurpose::PolicyAuthor,
                )
                .is_err()
        );
        assert!(
            other
                .verification_key()
                .unwrap()
                .verify(
                    &statement,
                    &envelope,
                    DOMAIN,
                    TENANT,
                    INCARNATION,
                    KeyPurpose::PolicyAuthor,
                )
                .is_err()
        );
    }

    #[test]
    fn wrong_domain_tenant_incarnation_and_purpose_fail() {
        let material = SigningKeyMaterial::generate(KeyPurpose::PolicyAuthor).unwrap();
        let statement = statement();
        let envelope = material.sign(&statement).unwrap();
        let key = material.verification_key().unwrap();
        for result in [
            key.verify(
                &statement,
                &envelope,
                "cognigraph.policy-approval.v1",
                TENANT,
                INCARNATION,
                KeyPurpose::PolicyAuthor,
            ),
            key.verify(
                &statement,
                &envelope,
                DOMAIN,
                "tenant-b",
                INCARNATION,
                KeyPurpose::PolicyAuthor,
            ),
            key.verify(
                &statement,
                &envelope,
                DOMAIN,
                TENANT,
                "incarnation-2",
                KeyPurpose::PolicyAuthor,
            ),
            key.verify(
                &statement,
                &envelope,
                DOMAIN,
                TENANT,
                INCARNATION,
                KeyPurpose::PolicyApprover,
            ),
        ] {
            assert!(result.is_err());
        }
    }

    #[test]
    fn malformed_base64_padding_and_lengths_fail() {
        let material = SigningKeyMaterial::generate(KeyPurpose::PolicyAuthor).unwrap();
        let statement = statement();
        let envelope = material.sign(&statement).unwrap();

        let mut padded_key = material.verification_key().unwrap();
        padded_key.public_key.push('=');
        assert!(padded_key.validate().is_err());

        let mut short_key = material.verification_key().unwrap();
        short_key.public_key = URL_SAFE_NO_PAD.encode([7u8; 31]);
        assert!(short_key.validate().is_err());

        let mut padded_signature = envelope.clone();
        padded_signature.signature.push('=');
        assert!(
            material
                .verification_key()
                .unwrap()
                .verify(
                    &statement,
                    &padded_signature,
                    DOMAIN,
                    TENANT,
                    INCARNATION,
                    KeyPurpose::PolicyAuthor,
                )
                .is_err()
        );

        let mut short_signature = envelope;
        short_signature.signature = URL_SAFE_NO_PAD.encode([3u8; 63]);
        assert!(
            material
                .verification_key()
                .unwrap()
                .verify(
                    &statement,
                    &short_signature,
                    DOMAIN,
                    TENANT,
                    INCARNATION,
                    KeyPurpose::PolicyAuthor,
                )
                .is_err()
        );
    }

    #[test]
    fn weak_keys_and_mismatched_key_material_fail() {
        let material = SigningKeyMaterial::generate(KeyPurpose::PolicyAuthor).unwrap();
        let mut weak = material.verification_key().unwrap();
        weak.public_key = URL_SAFE_NO_PAD.encode([0u8; PUBLIC_KEY_BYTES]);
        weak.key_id = key_id_for_bytes(&[0u8; PUBLIC_KEY_BYTES]);
        assert!(matches!(weak.validate(), Err(GovernanceError::WeakKey)));

        let other = SigningKeyMaterial::generate(KeyPurpose::PolicyAuthor).unwrap();
        let mut mismatched = material.clone();
        mismatched.public_key = other.public_key;
        mismatched.key_id = other.key_id;
        assert!(matches!(
            mismatched.validate(),
            Err(GovernanceError::KeyMaterialMismatch)
        ));
    }

    #[test]
    fn wrong_algorithm_key_digest_signature_and_schema_fail() {
        let material = SigningKeyMaterial::generate(KeyPurpose::Promoter).unwrap();
        let statement = statement();
        let envelope = material.sign(&statement).unwrap();
        let key = material.verification_key().unwrap();

        let mut wrong_algorithm = envelope.clone();
        wrong_algorithm.algorithm = "EdDSA".into();
        let mut wrong_key = envelope.clone();
        wrong_key.key_id = format!("ed25519:sha256:{}", "0".repeat(64));
        let mut wrong_digest = envelope.clone();
        wrong_digest.statement_digest = format!("sha256:{}", "0".repeat(64));
        let mut wrong_signature = envelope.clone();
        let mut signature_bytes = URL_SAFE_NO_PAD.decode(&wrong_signature.signature).unwrap();
        signature_bytes[0] ^= 1;
        wrong_signature.signature = URL_SAFE_NO_PAD.encode(signature_bytes);
        let mut wrong_schema = envelope;
        wrong_schema.schema_version = 2;

        for candidate in [
            wrong_algorithm,
            wrong_key,
            wrong_digest,
            wrong_signature,
            wrong_schema,
        ] {
            assert!(
                key.verify(
                    &statement,
                    &candidate,
                    DOMAIN,
                    TENANT,
                    INCARNATION,
                    KeyPurpose::Promoter,
                )
                .is_err()
            );
        }
    }

    #[test]
    fn signing_rejects_invalid_context_and_floating_payload() {
        let material = SigningKeyMaterial::generate(KeyPurpose::PolicyAuthor).unwrap();
        let invalid_domain = GovernanceStatement::new(
            "domain with spaces",
            TENANT,
            INCARNATION,
            json!({"value": 1}),
        );
        assert!(material.sign(&invalid_domain).is_err());
        let float = GovernanceStatement::new(DOMAIN, TENANT, INCARNATION, json!({"value": 1.0}));
        assert!(matches!(
            material.sign(&float),
            Err(GovernanceError::FloatingPoint)
        ));
    }

    #[test]
    fn unknown_json_fields_are_rejected() {
        assert!(
            serde_json::from_value::<SignatureEnvelope>(json!({
                "schema_version": 1,
                "algorithm": "ed25519",
                "key_id": format!("ed25519:sha256:{}", "0".repeat(64)),
                "statement_digest": format!("sha256:{}", "0".repeat(64)),
                "signature": "x",
                "unexpected": true
            }))
            .is_err()
        );
        assert!(
            serde_json::from_value::<GovernanceStatement<Value>>(json!({
                "schema_version": 1,
                "domain": DOMAIN,
                "tenant": TENANT,
                "tenant_incarnation": INCARNATION,
                "payload": {},
                "unexpected": true
            }))
            .is_err()
        );
    }
}
