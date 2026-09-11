//! M20 content-addressed external artifact attestations.
//!
//! Attestations authenticate an external Artifact Attestor's exact-byte
//! manifest claim. The server validates canonical manifests, signatures, key
//! authority, and promotion bindings, but deliberately does not fetch signed
//! locations or claim that an evaluation consumed those bytes.

use std::collections::{BTreeMap, HashSet};

use cognigraph_auth::Role;
use cognigraph_core::CogniGraphError;
use cognigraph_governance::{GovernanceStatement, KeyPurpose, SignatureEnvelope};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use unicode_normalization::is_nfc;

use crate::governance::{
    GovernanceActor, GovernanceKeyRecord, GovernanceMutation, validate_historical_key_use,
};
use crate::promotions::{
    DIGEST_ALGORITHM, PromotionContext, PromotionManager, PromotionPage, RevisionAttestation,
    canonical_digest, digest_bytes, now_millis, record_digest, scoped_key,
    validate_idempotency_key,
};

pub const ARTIFACT_ATTESTATIONS_COLLECTION: &str = "_cognigraph_artifact_attestations";
pub const ARTIFACT_ATTESTATION_DOMAIN: &str = "cognigraph.artifact-attestation.v1";
pub const ARTIFACT_ATTESTATION_SCHEMA_VERSION: u32 = 1;
pub const ARTIFACT_MANIFEST_SCHEMA_VERSION: u32 = 1;

pub(crate) const MAX_ARTIFACT_ATTESTATIONS_PER_TENANT: usize = 10_000;
pub(crate) const MAX_ARTIFACT_ATTESTATION_REQUEST_BYTES: usize = 17 * 1024 * 1024;
pub(crate) const MAX_ARTIFACT_MANIFEST_BYTES_PER_TENANT: usize = 64 * 1024 * 1024;
const MAX_ARTIFACT_LIST_PAGE_SIZE: usize = 50;
const MAX_ARTIFACT_ENTRIES: usize = 100_000;
const MAX_ARTIFACT_LOCATIONS: usize = 8;
const MAX_ARTIFACT_TOTAL_BYTES: u64 = 1 << 40;
const MAX_MANIFEST_BYTES: usize = 16 * 1024 * 1024;
const MAX_IDENTIFIER_BYTES: usize = 256;
const MAX_PATH_BYTES: usize = 1_024;
const MAX_URI_BYTES: usize = 2_048;
const MAX_LOCATION_METADATA_BYTES: usize = 1_024;
const MAX_CLOCK_SKEW_MS: u64 = 5 * 60 * 1_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    Corpus,
    Graph,
    Oracle,
    Scorer,
    Verifier,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactManifestEntry {
    pub logical_path: String,
    pub media_type: String,
    pub byte_length: u64,
    pub blob_digest: String,
    pub executable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactManifest {
    pub schema_version: u32,
    pub artifact_kind: ArtifactKind,
    pub artifact_format: String,
    pub entries: Vec<ArtifactManifestEntry>,
    pub entry_count: u64,
    pub total_bytes: u64,
}

impl ArtifactManifest {
    fn validate(&self) -> Result<(), CogniGraphError> {
        if self.schema_version != ARTIFACT_MANIFEST_SCHEMA_VERSION {
            return Err(validation(format!(
                "unsupported artifact manifest schema version {}",
                self.schema_version
            )));
        }
        validate_identifier("manifest.artifact_format", &self.artifact_format)?;
        if self.entries.is_empty() || self.entries.len() > MAX_ARTIFACT_ENTRIES {
            return Err(validation(format!(
                "artifact manifest must contain 1..={MAX_ARTIFACT_ENTRIES} entries"
            )));
        }
        if self.entry_count != self.entries.len() as u64 {
            return Err(validation(
                "artifact manifest entry_count does not match entries",
            ));
        }
        let mut total = 0_u64;
        let mut previous_path: Option<&str> = None;
        let mut portable_paths = HashSet::new();
        for entry in &self.entries {
            validate_logical_path(&entry.logical_path)?;
            validate_identifier("manifest.entries.media_type", &entry.media_type)?;
            validate_digest("manifest.entries.blob_digest", &entry.blob_digest)?;
            if previous_path.is_some_and(|previous| previous >= entry.logical_path.as_str()) {
                return Err(validation(
                    "artifact manifest entries must be sorted by unique NFC logical_path",
                ));
            }
            if !portable_paths.insert(entry.logical_path.to_lowercase()) {
                return Err(validation(
                    "artifact manifest contains a portability-threatening case-only path alias",
                ));
            }
            previous_path = Some(&entry.logical_path);
            total = total
                .checked_add(entry.byte_length)
                .ok_or_else(|| validation("artifact manifest total byte length overflows"))?;
        }
        if total == 0 || total != self.total_bytes || total > MAX_ARTIFACT_TOTAL_BYTES {
            return Err(validation(format!(
                "artifact manifest total_bytes must equal a positive checked sum no larger than {MAX_ARTIFACT_TOTAL_BYTES}"
            )));
        }
        if self.canonical_size_bytes()? > MAX_MANIFEST_BYTES {
            return Err(validation(format!(
                "artifact manifest canonical form exceeds {MAX_MANIFEST_BYTES} bytes"
            )));
        }
        Ok(())
    }

    pub(crate) fn canonical_size_bytes(&self) -> Result<usize, CogniGraphError> {
        cognigraph_governance::canonical_json_bytes(self)
            .map(|bytes| bytes.len())
            .map_err(|error| validation(format!("artifact manifest is not canonical: {error}")))
    }
}

/// Closed usage claim for the bytes in an artifact manifest.
///
/// Legacy scorer/verifier hashes remain semantics identities. The manifest
/// digest independently names the newly attested raw bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ArtifactSubject {
    Corpus {
        revision: RevisionAttestation,
        preprocessing_digest: String,
    },
    Graph {
        revision: RevisionAttestation,
        candidate_digest: String,
        construction_config_digest: String,
        corpus_manifest_digest: String,
        preprocessing_digest: String,
    },
    Oracle {
        revision: RevisionAttestation,
        eval_spec_digest: String,
        case_manifest_digest: String,
        corpus_manifest_digest: String,
    },
    Scorer {
        scorer_id: String,
        scorer_version: String,
        semantics_identity_digest: String,
        metric_semantics_version: String,
        executable_digest: String,
    },
    Verifier {
        verifier_name: String,
        verifier_version: String,
        semantics_identity_digest: String,
    },
}

impl ArtifactSubject {
    pub fn kind(&self) -> ArtifactKind {
        match self {
            Self::Corpus { .. } => ArtifactKind::Corpus,
            Self::Graph { .. } => ArtifactKind::Graph,
            Self::Oracle { .. } => ArtifactKind::Oracle,
            Self::Scorer { .. } => ArtifactKind::Scorer,
            Self::Verifier { .. } => ArtifactKind::Verifier,
        }
    }

    fn validate(&self) -> Result<(), CogniGraphError> {
        match self {
            Self::Corpus {
                revision,
                preprocessing_digest,
            } => {
                validate_revision_subject(revision, "corpus")?;
                validate_digest("subject.preprocessing_digest", preprocessing_digest)
            }
            Self::Graph {
                revision,
                candidate_digest,
                construction_config_digest,
                corpus_manifest_digest,
                preprocessing_digest,
            } => {
                validate_revision_subject(revision, "graph")?;
                for (label, digest) in [
                    ("subject.candidate_digest", candidate_digest),
                    (
                        "subject.construction_config_digest",
                        construction_config_digest,
                    ),
                    ("subject.corpus_manifest_digest", corpus_manifest_digest),
                    ("subject.preprocessing_digest", preprocessing_digest),
                ] {
                    validate_digest(label, digest)?;
                }
                if revision.candidate_digest.as_ref() != Some(candidate_digest)
                    || revision.configuration_digest.as_ref() != Some(construction_config_digest)
                {
                    return Err(validation(
                        "graph subject does not match its revision candidate/configuration binding",
                    ));
                }
                Ok(())
            }
            Self::Oracle {
                revision,
                eval_spec_digest,
                case_manifest_digest,
                corpus_manifest_digest,
            } => {
                validate_revision_subject(revision, "oracle")?;
                for (label, digest) in [
                    ("subject.eval_spec_digest", eval_spec_digest),
                    ("subject.case_manifest_digest", case_manifest_digest),
                    ("subject.corpus_manifest_digest", corpus_manifest_digest),
                ] {
                    validate_digest(label, digest)?;
                }
                Ok(())
            }
            Self::Scorer {
                scorer_id,
                scorer_version,
                semantics_identity_digest,
                metric_semantics_version,
                executable_digest,
            } => {
                for (label, value) in [
                    ("subject.scorer_id", scorer_id),
                    ("subject.scorer_version", scorer_version),
                    ("subject.metric_semantics_version", metric_semantics_version),
                ] {
                    validate_identifier(label, value)?;
                }
                validate_digest(
                    "subject.semantics_identity_digest",
                    semantics_identity_digest,
                )?;
                validate_digest("subject.executable_digest", executable_digest)
            }
            Self::Verifier {
                verifier_name,
                verifier_version,
                semantics_identity_digest,
            } => {
                validate_identifier("subject.verifier_name", verifier_name)?;
                validate_identifier("subject.verifier_version", verifier_version)?;
                validate_digest(
                    "subject.semantics_identity_digest",
                    semantics_identity_digest,
                )
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactLocationObservation {
    pub uri: String,
    pub observed_at_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub etag: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_modified: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declared_length: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_encoding: Option<String>,
}

impl ArtifactLocationObservation {
    fn validate(&self) -> Result<(), CogniGraphError> {
        validate_location_uri(&self.uri)?;
        if self.observed_at_ms == 0 {
            return Err(validation(
                "artifact location observed_at_ms must be positive",
            ));
        }
        for (label, value) in [
            ("object_version", self.object_version.as_deref()),
            ("etag", self.etag.as_deref()),
            ("last_modified", self.last_modified.as_deref()),
            ("content_encoding", self.content_encoding.as_deref()),
        ] {
            if let Some(value) = value {
                validate_text(
                    &format!("artifact location {label}"),
                    value,
                    MAX_LOCATION_METADATA_BYTES,
                )?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactAttestationPayload {
    pub attestation_id: String,
    pub manifest: ArtifactManifest,
    pub manifest_digest: String,
    pub subject: ArtifactSubject,
    pub locations: Vec<ArtifactLocationObservation>,
    pub attestor_registration_id: String,
    pub attestor_principal_id: String,
    pub hash_completed_at_ms: u64,
    pub signed_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateArtifactAttestationRequest {
    pub statement: GovernanceStatement<ArtifactAttestationPayload>,
    pub attestor_signature: SignatureEnvelope,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactAttestationRecord {
    #[serde(rename = "_key")]
    pub key: String,
    pub schema_version: u32,
    pub digest_algorithm: String,
    pub tenant: String,
    pub tenant_incarnation: String,
    pub attestation_id: String,
    pub artifact_kind: ArtifactKind,
    pub manifest: ArtifactManifest,
    pub manifest_digest: String,
    pub subject: ArtifactSubject,
    pub subject_digest: String,
    pub locations: Vec<ArtifactLocationObservation>,
    pub attestor_registration_id: String,
    pub attestor_registration_digest: String,
    pub attestor_principal_id: String,
    pub root_key_id: String,
    pub hash_completed_at_ms: u64,
    pub signed_at_ms: u64,
    pub attestor_signature: SignatureEnvelope,
    pub accepted_by: GovernanceActor,
    pub accepted_at_ms: u64,
    pub idempotency_key_hash: String,
    pub request_digest: String,
    pub attestation_digest: String,
}

impl ArtifactAttestationRecord {
    pub fn public_value(&self) -> Result<Value, CogniGraphError> {
        let mut value = serde_json::to_value(self)?;
        if let Some(fields) = value.as_object_mut() {
            fields.remove("_key");
            fields.remove("idempotency_key_hash");
        }
        Ok(value)
    }

    pub(crate) fn binding(&self) -> ArtifactAttestationBinding {
        ArtifactAttestationBinding {
            kind: self.artifact_kind,
            attestation_id: self.attestation_id.clone(),
            attestation_digest: self.attestation_digest.clone(),
            manifest_digest: self.manifest_digest.clone(),
            subject_digest: self.subject_digest.clone(),
            entry_count: self.manifest.entry_count,
            total_bytes: self.manifest.total_bytes,
            attestor_registration_id: self.attestor_registration_id.clone(),
            attestor_registration_digest: self.attestor_registration_digest.clone(),
            attestor_principal_id: self.attestor_principal_id.clone(),
            root_key_id: self.root_key_id.clone(),
        }
    }

    fn summary(&self) -> ArtifactAttestationSummary {
        ArtifactAttestationSummary {
            schema_version: self.schema_version,
            digest_algorithm: self.digest_algorithm.clone(),
            tenant: self.tenant.clone(),
            tenant_incarnation: self.tenant_incarnation.clone(),
            attestation_id: self.attestation_id.clone(),
            artifact_kind: self.artifact_kind,
            manifest_digest: self.manifest_digest.clone(),
            subject_digest: self.subject_digest.clone(),
            entry_count: self.manifest.entry_count,
            total_bytes: self.manifest.total_bytes,
            attestor_registration_id: self.attestor_registration_id.clone(),
            attestor_registration_digest: self.attestor_registration_digest.clone(),
            attestor_principal_id: self.attestor_principal_id.clone(),
            root_key_id: self.root_key_id.clone(),
            hash_completed_at_ms: self.hash_completed_at_ms,
            signed_at_ms: self.signed_at_ms,
            accepted_by: self.accepted_by.clone(),
            accepted_at_ms: self.accepted_at_ms,
            attestation_digest: self.attestation_digest.clone(),
        }
    }
}

/// Bounded list representation. The complete manifest, subject, locations,
/// and detached signature remain available from the detail endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactAttestationSummary {
    pub schema_version: u32,
    pub digest_algorithm: String,
    pub tenant: String,
    pub tenant_incarnation: String,
    pub attestation_id: String,
    pub artifact_kind: ArtifactKind,
    pub manifest_digest: String,
    pub subject_digest: String,
    pub entry_count: u64,
    pub total_bytes: u64,
    pub attestor_registration_id: String,
    pub attestor_registration_digest: String,
    pub attestor_principal_id: String,
    pub root_key_id: String,
    pub hash_completed_at_ms: u64,
    pub signed_at_ms: u64,
    pub accepted_by: GovernanceActor,
    pub accepted_at_ms: u64,
    pub attestation_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactAttestationBinding {
    pub kind: ArtifactKind,
    pub attestation_id: String,
    pub attestation_digest: String,
    pub manifest_digest: String,
    pub subject_digest: String,
    pub entry_count: u64,
    pub total_bytes: u64,
    pub attestor_registration_id: String,
    pub attestor_registration_digest: String,
    pub attestor_principal_id: String,
    pub root_key_id: String,
}

impl ArtifactAttestationBinding {
    fn validate(&self, expected_kind: ArtifactKind) -> Result<(), CogniGraphError> {
        if self.kind != expected_kind || self.entry_count == 0 || self.total_bytes == 0 {
            return Err(validation(
                "artifact attestation binding kind or counts mismatch",
            ));
        }
        validate_record_id("artifact binding attestation_id", &self.attestation_id)?;
        validate_record_id(
            "artifact binding attestor_registration_id",
            &self.attestor_registration_id,
        )?;
        validate_identifier(
            "artifact binding attestor_principal_id",
            &self.attestor_principal_id,
        )?;
        validate_identifier("artifact binding root_key_id", &self.root_key_id)?;
        for (label, digest) in [
            ("attestation_digest", &self.attestation_digest),
            ("manifest_digest", &self.manifest_digest),
            ("subject_digest", &self.subject_digest),
            (
                "attestor_registration_digest",
                &self.attestor_registration_digest,
            ),
        ] {
            validate_digest(&format!("artifact binding {label}"), digest)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactAttestationSet {
    pub corpus: ArtifactAttestationBinding,
    pub graph: ArtifactAttestationBinding,
    pub oracle: ArtifactAttestationBinding,
    pub scorer: ArtifactAttestationBinding,
    pub verifier: ArtifactAttestationBinding,
    pub set_digest: String,
}

#[derive(Debug, Clone)]
pub(crate) struct ActiveArtifactAttestations {
    pub corpus: ArtifactAttestationRecord,
    pub graph: ArtifactAttestationRecord,
    pub oracle: ArtifactAttestationRecord,
    pub scorer: ArtifactAttestationRecord,
    pub verifier: ArtifactAttestationRecord,
}

impl ArtifactAttestationSet {
    pub fn validate(&self) -> Result<(), CogniGraphError> {
        self.corpus.validate(ArtifactKind::Corpus)?;
        self.graph.validate(ArtifactKind::Graph)?;
        self.oracle.validate(ArtifactKind::Oracle)?;
        self.scorer.validate(ArtifactKind::Scorer)?;
        self.verifier.validate(ArtifactKind::Verifier)?;
        let ids = [
            &self.corpus.attestation_id,
            &self.graph.attestation_id,
            &self.oracle.attestation_id,
            &self.scorer.attestation_id,
            &self.verifier.attestation_id,
        ];
        if ids.into_iter().collect::<HashSet<_>>().len() != 5 {
            return Err(validation(
                "artifact attestation set requires five distinct records",
            ));
        }
        validate_digest("artifact set_digest", &self.set_digest)?;
        if record_digest(self, "set_digest")? != self.set_digest {
            return Err(validation("artifact attestation set digest mismatch"));
        }
        Ok(())
    }

    pub(crate) fn bindings(&self) -> [&ArtifactAttestationBinding; 5] {
        [
            &self.corpus,
            &self.graph,
            &self.oracle,
            &self.scorer,
            &self.verifier,
        ]
    }

    pub fn attestor_principals(&self) -> HashSet<&str> {
        self.bindings()
            .into_iter()
            .map(|binding| binding.attestor_principal_id.as_str())
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolveArtifactBindingsRequest {
    pub corpus_attestation_id: String,
    pub graph_attestation_id: String,
    pub oracle_attestation_id: String,
    pub scorer_attestation_id: String,
    pub verifier_attestation_id: String,
}

impl ResolveArtifactBindingsRequest {
    fn ids(&self) -> [(ArtifactKind, &str); 5] {
        [
            (ArtifactKind::Corpus, &self.corpus_attestation_id),
            (ArtifactKind::Graph, &self.graph_attestation_id),
            (ArtifactKind::Oracle, &self.oracle_attestation_id),
            (ArtifactKind::Scorer, &self.scorer_attestation_id),
            (ArtifactKind::Verifier, &self.verifier_attestation_id),
        ]
    }

    fn validate(&self) -> Result<(), CogniGraphError> {
        let ids = self.ids();
        for (_, id) in ids {
            validate_record_id("artifact attestation id", id)?;
        }
        if ids
            .into_iter()
            .map(|(_, id)| id)
            .collect::<HashSet<_>>()
            .len()
            != 5
        {
            return Err(validation(
                "artifact binding resolver requires five distinct attestation ids",
            ));
        }
        Ok(())
    }
}

impl PromotionManager {
    pub(crate) async fn artifact_attestation_counts_locked(
        &self,
        tenant: &str,
        incarnation: &str,
    ) -> Result<(usize, BTreeMap<&'static str, usize>), CogniGraphError> {
        let records = self
            .scoped_records::<ArtifactAttestationRecord>(
                ARTIFACT_ATTESTATIONS_COLLECTION,
                tenant,
                incarnation,
                "aa",
                MAX_ARTIFACT_ATTESTATIONS_PER_TENANT + 1,
            )
            .await?;
        if records.len() > MAX_ARTIFACT_ATTESTATIONS_PER_TENANT {
            return Err(conflict("artifact attestation capacity exceeded"));
        }
        let mut counts = BTreeMap::from([
            ("corpus", 0),
            ("graph", 0),
            ("oracle", 0),
            ("scorer", 0),
            ("verifier", 0),
        ]);
        let mut manifest_bytes = 0_usize;
        for record in &records {
            manifest_bytes = manifest_bytes
                .checked_add(record.manifest.canonical_size_bytes()?)
                .ok_or_else(|| conflict("artifact manifest storage accounting overflowed"))?;
            let label = match record.artifact_kind {
                ArtifactKind::Corpus => "corpus",
                ArtifactKind::Graph => "graph",
                ArtifactKind::Oracle => "oracle",
                ArtifactKind::Scorer => "scorer",
                ArtifactKind::Verifier => "verifier",
            };
            *counts
                .get_mut(label)
                .expect("all artifact kinds are seeded") += 1;
        }
        if manifest_bytes > MAX_ARTIFACT_MANIFEST_BYTES_PER_TENANT {
            return Err(conflict("artifact manifest storage capacity exceeded"));
        }
        Ok((records.len(), counts))
    }

    pub async fn create_artifact_attestation(
        &self,
        tenant: &str,
        incarnation: &str,
        actor: GovernanceActor,
        idempotency_key: &str,
        request: CreateArtifactAttestationRequest,
    ) -> Result<GovernanceMutation<ArtifactAttestationRecord>, CogniGraphError> {
        actor.require_role(Role::ArtifactAttestor)?;
        validate_idempotency_key(idempotency_key)?;
        self.ensure_governance_repository(tenant).await?;
        let _guard = self.transition_lock.lock().await;
        self.ensure_tenant_active(tenant)?;
        let now = now_millis();
        validate_statement_scope(&request.statement, tenant, incarnation)?;
        let payload = &request.statement.payload;
        validate_payload(payload, tenant, incarnation, now)?;

        let key_hash = digest_bytes(idempotency_key.as_bytes());
        let request_digest = canonical_digest(&request)?;
        let key = scoped_key(tenant, incarnation, "aa", &payload.attestation_id);
        if let Some(existing) = self
            .get_authority_raw::<ArtifactAttestationRecord>(
                tenant,
                ARTIFACT_ATTESTATIONS_COLLECTION,
                &key,
            )
            .await?
        {
            self.validate_stored_artifact_attestation(&existing, tenant, incarnation)
                .await?;
            if existing.idempotency_key_hash == key_hash
                && existing.request_digest == request_digest
            {
                return Ok(GovernanceMutation {
                    record: existing,
                    replayed: true,
                });
            }
            return Err(conflict(
                "artifact attestation identity already has different authority",
            ));
        }
        self.ensure_mutations_healthy(tenant)?;

        let attestor = self
            .get_active_key_locked(
                tenant,
                incarnation,
                &payload.attestor_registration_id,
                KeyPurpose::ArtifactAttestor,
                now,
            )
            .await?;
        require_attestor(&attestor, &actor, &payload.attestor_principal_id)?;
        attestor
            .verification_key
            .verify(
                &request.statement,
                &request.attestor_signature,
                ARTIFACT_ATTESTATION_DOMAIN,
                tenant,
                incarnation,
                KeyPurpose::ArtifactAttestor,
            )
            .map_err(signature_error)?;

        self.ensure_governance_idempotency_available_locked(tenant, incarnation, &key_hash)
            .await?;
        let existing = self
            .scoped_records::<ArtifactAttestationRecord>(
                ARTIFACT_ATTESTATIONS_COLLECTION,
                tenant,
                incarnation,
                "aa",
                MAX_ARTIFACT_ATTESTATIONS_PER_TENANT + 1,
            )
            .await?;
        if existing.len() >= MAX_ARTIFACT_ATTESTATIONS_PER_TENANT {
            return Err(conflict(format!(
                "tenant artifact attestation limit of {MAX_ARTIFACT_ATTESTATIONS_PER_TENANT} is exhausted"
            )));
        }
        let existing_manifest_bytes = existing.iter().try_fold(0_usize, |total, record| {
            total
                .checked_add(record.manifest.canonical_size_bytes()?)
                .ok_or_else(|| conflict("artifact manifest storage accounting overflowed"))
        })?;
        let requested_manifest_bytes = payload.manifest.canonical_size_bytes()?;
        if existing_manifest_bytes
            .checked_add(requested_manifest_bytes)
            .is_none_or(|total| total > MAX_ARTIFACT_MANIFEST_BYTES_PER_TENANT)
        {
            return Err(conflict(format!(
                "tenant artifact manifest storage limit of {MAX_ARTIFACT_MANIFEST_BYTES_PER_TENANT} canonical bytes is exhausted"
            )));
        }

        let subject_digest = canonical_digest(&payload.subject)?;
        let mut record = ArtifactAttestationRecord {
            key,
            schema_version: ARTIFACT_ATTESTATION_SCHEMA_VERSION,
            digest_algorithm: DIGEST_ALGORITHM.into(),
            tenant: tenant.into(),
            tenant_incarnation: incarnation.into(),
            attestation_id: payload.attestation_id.clone(),
            artifact_kind: payload.manifest.artifact_kind,
            manifest: payload.manifest.clone(),
            manifest_digest: payload.manifest_digest.clone(),
            subject: payload.subject.clone(),
            subject_digest,
            locations: payload.locations.clone(),
            attestor_registration_id: attestor.registration_id.clone(),
            attestor_registration_digest: attestor.registration_digest.clone(),
            attestor_principal_id: attestor.principal_id.clone(),
            root_key_id: attestor.root_key_id.clone(),
            hash_completed_at_ms: payload.hash_completed_at_ms,
            signed_at_ms: payload.signed_at_ms,
            attestor_signature: request.attestor_signature,
            accepted_by: actor,
            accepted_at_ms: now,
            idempotency_key_hash: key_hash,
            request_digest,
            attestation_digest: String::new(),
        };
        record.attestation_digest = record_digest(&record, "attestation_digest")?;
        self.validate_artifact_record_against(&record, &attestor, tenant, incarnation)?;
        validate_historical_key_use(
            &attestor,
            KeyPurpose::ArtifactAttestor,
            record.signed_at_ms,
            record.accepted_at_ms,
            None,
        )?;
        self.insert_immutable(
            tenant,
            ARTIFACT_ATTESTATIONS_COLLECTION,
            &record.key,
            &record,
        )
        .await?;
        Ok(GovernanceMutation {
            record,
            replayed: false,
        })
    }

    pub async fn list_artifact_attestations(
        &self,
        tenant: &str,
        incarnation: &str,
        limit: usize,
        cursor: Option<&str>,
    ) -> Result<PromotionPage<ArtifactAttestationSummary>, CogniGraphError> {
        if !(1..=MAX_ARTIFACT_LIST_PAGE_SIZE).contains(&limit) {
            return Err(validation(format!(
                "artifact attestation list limit must be between 1 and {MAX_ARTIFACT_LIST_PAGE_SIZE}"
            )));
        }
        self.ensure_governance_repository(tenant).await?;
        let prefix = scoped_key(tenant, incarnation, "aa", "");
        if cursor.is_some_and(|cursor| !cursor.starts_with(&prefix)) {
            return Err(validation(
                "artifact attestation cursor belongs to another scope",
            ));
        }
        let after = cursor.unwrap_or(&prefix);
        let rows = self
            .backend
            .list_documents_after_key(
                ARTIFACT_ATTESTATIONS_COLLECTION,
                Some(after),
                &[],
                limit.saturating_add(1),
            )
            .await?;
        let mut keys = Vec::new();
        for value in rows {
            let key = value
                .get("_key")
                .and_then(Value::as_str)
                .ok_or_else(|| conflict("artifact repository record is missing `_key`"))?;
            if !key.starts_with(&prefix) {
                break;
            }
            keys.push(key.to_string());
        }
        let has_more = keys.len() > limit;
        if has_more {
            keys.truncate(limit);
        }
        let mut records = Vec::with_capacity(keys.len());
        for key in &keys {
            let record = self
                .get_authority_raw::<ArtifactAttestationRecord>(
                    tenant,
                    ARTIFACT_ATTESTATIONS_COLLECTION,
                    key,
                )
                .await?
                .ok_or_else(|| conflict("artifact attestation disappeared during listing"))?;
            self.validate_stored_artifact_attestation(&record, tenant, incarnation)
                .await?;
            records.push(record.summary());
        }
        Ok(PromotionPage {
            next_cursor: has_more.then(|| keys.last().cloned()).flatten(),
            records,
        })
    }

    pub async fn get_artifact_attestation(
        &self,
        tenant: &str,
        incarnation: &str,
        id: &str,
    ) -> Result<ArtifactAttestationRecord, CogniGraphError> {
        validate_record_id("artifact attestation id", id)?;
        self.ensure_governance_repository(tenant).await?;
        let key = scoped_key(tenant, incarnation, "aa", id);
        let record = self
            .get_authority_raw::<ArtifactAttestationRecord>(
                tenant,
                ARTIFACT_ATTESTATIONS_COLLECTION,
                &key,
            )
            .await?
            .ok_or_else(|| not_found("artifact attestation", id))?;
        self.validate_stored_artifact_attestation(&record, tenant, incarnation)
            .await?;
        Ok(record)
    }

    pub async fn resolve_artifact_bindings(
        &self,
        tenant: &str,
        incarnation: &str,
        request: &ResolveArtifactBindingsRequest,
    ) -> Result<ArtifactAttestationSet, CogniGraphError> {
        request.validate()?;
        self.ensure_governance_repository(tenant).await?;
        let _guard = self.transition_lock.lock().await;
        let now = now_millis();
        let mut bindings = Vec::with_capacity(5);
        for (expected_kind, id) in request.ids() {
            let record = self
                .get_active_artifact_attestation_locked(tenant, incarnation, id, now)
                .await?;
            if record.artifact_kind != expected_kind {
                return Err(validation(
                    "artifact attestation id was supplied in the wrong kind slot",
                ));
            }
            bindings.push(record.binding());
        }
        let mut iterator = bindings.into_iter();
        let mut set = ArtifactAttestationSet {
            corpus: iterator.next().expect("five bindings were collected"),
            graph: iterator.next().expect("five bindings were collected"),
            oracle: iterator.next().expect("five bindings were collected"),
            scorer: iterator.next().expect("five bindings were collected"),
            verifier: iterator.next().expect("five bindings were collected"),
            set_digest: String::new(),
        };
        set.set_digest = record_digest(&set, "set_digest")?;
        set.validate()?;
        Ok(set)
    }

    pub(crate) async fn validate_artifact_binding_set_active(
        &self,
        tenant: &str,
        incarnation: &str,
        context: &PromotionContext,
        set: &ArtifactAttestationSet,
        at_ms: u64,
    ) -> Result<(), CogniGraphError> {
        self.active_artifact_attestations(tenant, incarnation, context, set, at_ms)
            .await?;
        Ok(())
    }

    pub(crate) async fn active_artifact_attestations(
        &self,
        tenant: &str,
        incarnation: &str,
        context: &PromotionContext,
        set: &ArtifactAttestationSet,
        at_ms: u64,
    ) -> Result<ActiveArtifactAttestations, CogniGraphError> {
        set.validate()?;
        validate_context_subject_bindings(context, set)?;
        let mut records = Vec::with_capacity(5);
        for binding in set.bindings() {
            let record = self
                .get_active_artifact_attestation_locked(
                    tenant,
                    incarnation,
                    &binding.attestation_id,
                    at_ms,
                )
                .await?;
            if record.binding() != *binding {
                return Err(conflict(
                    "promotion artifact binding does not match signed authority",
                ));
            }
            records.push(record);
        }
        let mut records = records.into_iter();
        Ok(ActiveArtifactAttestations {
            corpus: records.next().expect("five artifact records were loaded"),
            graph: records.next().expect("five artifact records were loaded"),
            oracle: records.next().expect("five artifact records were loaded"),
            scorer: records.next().expect("five artifact records were loaded"),
            verifier: records.next().expect("five artifact records were loaded"),
        })
    }

    pub(crate) fn validate_artifact_record_against(
        &self,
        record: &ArtifactAttestationRecord,
        attestor: &GovernanceKeyRecord,
        tenant: &str,
        incarnation: &str,
    ) -> Result<(), CogniGraphError> {
        if record.schema_version != ARTIFACT_ATTESTATION_SCHEMA_VERSION
            || record.digest_algorithm != DIGEST_ALGORITHM
            || record.tenant != tenant
            || record.tenant_incarnation != incarnation
            || record.key != scoped_key(tenant, incarnation, "aa", &record.attestation_id)
            || record.artifact_kind != record.manifest.artifact_kind
            || record.artifact_kind != record.subject.kind()
            || record.manifest_digest != canonical_digest(&record.manifest)?
            || record.subject_digest != canonical_digest(&record.subject)?
            || record.attestation_digest != record_digest(record, "attestation_digest")?
            || record.attestor_registration_id != attestor.registration_id
            || record.attestor_registration_digest != attestor.registration_digest
            || record.attestor_principal_id != attestor.principal_id
            || record.root_key_id != attestor.root_key_id
        {
            return Err(conflict("malformed or foreign artifact attestation record"));
        }
        record.accepted_by.require_role(Role::ArtifactAttestor)?;
        require_attestor(attestor, &record.accepted_by, &record.attestor_principal_id)?;
        record.manifest.validate()?;
        record.subject.validate()?;
        for location in &record.locations {
            location.validate()?;
        }
        if record.locations.is_empty() || record.locations.len() > MAX_ARTIFACT_LOCATIONS {
            return Err(conflict("artifact attestation location count is invalid"));
        }
        validate_digest(
            "artifact attestation request_digest",
            &record.request_digest,
        )?;
        validate_digest(
            "artifact attestation idempotency_key_hash",
            &record.idempotency_key_hash,
        )?;
        validate_digest(
            "artifact attestation attestation_digest",
            &record.attestation_digest,
        )?;
        if record.accepted_at_ms == 0
            || record.hash_completed_at_ms == 0
            || record.signed_at_ms < record.hash_completed_at_ms
            || record.signed_at_ms > record.accepted_at_ms.saturating_add(MAX_CLOCK_SKEW_MS)
            || record
                .locations
                .iter()
                .any(|location| location.observed_at_ms > record.hash_completed_at_ms)
        {
            return Err(conflict(
                "artifact attestation timestamp ordering is invalid",
            ));
        }
        let expected_id = artifact_attestation_id(
            tenant,
            incarnation,
            record.artifact_kind,
            &record.manifest_digest,
            &record.subject_digest,
            &record.attestor_registration_id,
        )?;
        if record.attestation_id != expected_id {
            return Err(conflict(
                "artifact attestation id does not match its natural identity",
            ));
        }
        let statement = GovernanceStatement::new(
            ARTIFACT_ATTESTATION_DOMAIN,
            tenant,
            incarnation,
            ArtifactAttestationPayload {
                attestation_id: record.attestation_id.clone(),
                manifest: record.manifest.clone(),
                manifest_digest: record.manifest_digest.clone(),
                subject: record.subject.clone(),
                locations: record.locations.clone(),
                attestor_registration_id: record.attestor_registration_id.clone(),
                attestor_principal_id: record.attestor_principal_id.clone(),
                hash_completed_at_ms: record.hash_completed_at_ms,
                signed_at_ms: record.signed_at_ms,
            },
        );
        let expected_request_digest = canonical_digest(&CreateArtifactAttestationRequest {
            statement: statement.clone(),
            attestor_signature: record.attestor_signature.clone(),
        })?;
        if record.request_digest != expected_request_digest {
            return Err(conflict("artifact attestation request authority mismatch"));
        }
        attestor
            .verification_key
            .verify(
                &statement,
                &record.attestor_signature,
                ARTIFACT_ATTESTATION_DOMAIN,
                tenant,
                incarnation,
                KeyPurpose::ArtifactAttestor,
            )
            .map_err(stored_signature_error)
    }

    pub(crate) async fn validate_stored_artifact_attestation(
        &self,
        record: &ArtifactAttestationRecord,
        tenant: &str,
        incarnation: &str,
    ) -> Result<(), CogniGraphError> {
        let attestor = self
            .get_key_locked(tenant, incarnation, &record.attestor_registration_id)
            .await?;
        self.validate_artifact_record_against(record, &attestor, tenant, incarnation)?;
        let revocation_id = crate::governance::key_revocation_id(
            tenant,
            incarnation,
            &record.attestor_registration_id,
        );
        let revocation_key = scoped_key(tenant, incarnation, "gkr", &revocation_id);
        let revocation = self
            .get_authority_raw::<crate::governance::GovernanceKeyRevocation>(
                tenant,
                crate::governance::GOVERNANCE_KEY_REVOCATIONS_COLLECTION,
                &revocation_key,
            )
            .await?;
        validate_historical_key_use(
            &attestor,
            KeyPurpose::ArtifactAttestor,
            record.signed_at_ms,
            record.accepted_at_ms,
            revocation.as_ref(),
        )
    }

    async fn get_active_artifact_attestation_locked(
        &self,
        tenant: &str,
        incarnation: &str,
        id: &str,
        at_ms: u64,
    ) -> Result<ArtifactAttestationRecord, CogniGraphError> {
        let key = scoped_key(tenant, incarnation, "aa", id);
        let record = self
            .get_authority_raw::<ArtifactAttestationRecord>(
                tenant,
                ARTIFACT_ATTESTATIONS_COLLECTION,
                &key,
            )
            .await?
            .ok_or_else(|| not_found("artifact attestation", id))?;
        self.validate_stored_artifact_attestation(&record, tenant, incarnation)
            .await?;
        self.get_active_key_locked(
            tenant,
            incarnation,
            &record.attestor_registration_id,
            KeyPurpose::ArtifactAttestor,
            at_ms,
        )
        .await?;
        Ok(record)
    }
}

pub fn artifact_attestation_id(
    tenant: &str,
    incarnation: &str,
    kind: ArtifactKind,
    manifest_digest: &str,
    subject_digest: &str,
    attestor_registration_id: &str,
) -> Result<String, CogniGraphError> {
    validate_digest("manifest_digest", manifest_digest)?;
    validate_digest("subject_digest", subject_digest)?;
    validate_record_id("attestor_registration_id", attestor_registration_id)?;
    Ok(canonical_digest(&json!({
        "tenant": tenant,
        "tenant_incarnation": incarnation,
        "artifact_kind": kind,
        "manifest_digest": manifest_digest,
        "subject_digest": subject_digest,
        "attestor_registration_id": attestor_registration_id,
    }))?
    .trim_start_matches("sha256:")
    .to_string())
}

pub(crate) fn validate_context_subject_bindings(
    context: &PromotionContext,
    set: &ArtifactAttestationSet,
) -> Result<(), CogniGraphError> {
    set.validate()?;
    let expected = [
        (
            &set.corpus,
            ArtifactSubject::Corpus {
                revision: context.revisions.corpus.clone(),
                preprocessing_digest: context.effective_configuration.preprocessing_digest.clone(),
            },
        ),
        (
            &set.graph,
            ArtifactSubject::Graph {
                revision: context.revisions.graph.clone(),
                candidate_digest: context.candidate.candidate_digest.clone(),
                construction_config_digest: context
                    .effective_configuration
                    .construction_config_digest
                    .clone(),
                corpus_manifest_digest: set.corpus.manifest_digest.clone(),
                preprocessing_digest: context.effective_configuration.preprocessing_digest.clone(),
            },
        ),
        (
            &set.oracle,
            ArtifactSubject::Oracle {
                revision: context.revisions.oracle.clone(),
                eval_spec_digest: context.effective_configuration.eval_spec_digest.clone(),
                case_manifest_digest: context.case_manifest.digest.clone(),
                corpus_manifest_digest: set.corpus.manifest_digest.clone(),
            },
        ),
        (
            &set.scorer,
            ArtifactSubject::Scorer {
                scorer_id: context.effective_configuration.scorer_id.clone(),
                scorer_version: context.effective_configuration.scorer_version.clone(),
                semantics_identity_digest: context
                    .effective_configuration
                    .scorer_artifact_digest
                    .clone(),
                metric_semantics_version: context.policy.metric_semantics_version.clone(),
                executable_digest: context.reproducibility.executable_digest.clone(),
            },
        ),
        (
            &set.verifier,
            ArtifactSubject::Verifier {
                verifier_name: context.policy.oracle.verifier_name.clone(),
                verifier_version: context.policy.oracle.verifier_version.clone(),
                semantics_identity_digest: context.policy.oracle.verifier_artifact_digest.clone(),
            },
        ),
    ];
    for (binding, subject) in expected {
        subject.validate()?;
        if binding.subject_digest != canonical_digest(&subject)? {
            return Err(validation(
                "promotion context artifact binding does not match its exact usage subject",
            ));
        }
    }
    Ok(())
}

fn validate_payload(
    payload: &ArtifactAttestationPayload,
    tenant: &str,
    incarnation: &str,
    now: u64,
) -> Result<(), CogniGraphError> {
    payload.manifest.validate()?;
    payload.subject.validate()?;
    if payload.manifest.artifact_kind != payload.subject.kind() {
        return Err(validation(
            "artifact manifest kind and subject kind do not match",
        ));
    }
    if payload.manifest_digest != canonical_digest(&payload.manifest)? {
        return Err(validation("artifact manifest digest mismatch"));
    }
    validate_record_id(
        "attestor_registration_id",
        &payload.attestor_registration_id,
    )?;
    validate_identifier("attestor_principal_id", &payload.attestor_principal_id)?;
    if payload.locations.is_empty() || payload.locations.len() > MAX_ARTIFACT_LOCATIONS {
        return Err(validation(format!(
            "artifact attestation requires 1..={MAX_ARTIFACT_LOCATIONS} signed locations"
        )));
    }
    for location in &payload.locations {
        location.validate()?;
    }
    if payload.hash_completed_at_ms == 0
        || payload.signed_at_ms < payload.hash_completed_at_ms
        || payload.signed_at_ms > now.saturating_add(MAX_CLOCK_SKEW_MS)
        || payload
            .locations
            .iter()
            .any(|location| location.observed_at_ms > payload.hash_completed_at_ms)
    {
        return Err(validation(
            "artifact attestation timestamp ordering is invalid",
        ));
    }
    let subject_digest = canonical_digest(&payload.subject)?;
    let expected_id = artifact_attestation_id(
        tenant,
        incarnation,
        payload.manifest.artifact_kind,
        &payload.manifest_digest,
        &subject_digest,
        &payload.attestor_registration_id,
    )?;
    if payload.attestation_id != expected_id {
        return Err(validation(
            "artifact attestation id does not match its natural identity",
        ));
    }
    Ok(())
}

fn validate_statement_scope<T>(
    statement: &GovernanceStatement<T>,
    tenant: &str,
    incarnation: &str,
) -> Result<(), CogniGraphError> {
    if statement.schema_version != cognigraph_governance::GOVERNANCE_SCHEMA_VERSION
        || statement.domain != ARTIFACT_ATTESTATION_DOMAIN
        || statement.tenant != tenant
        || statement.tenant_incarnation != incarnation
    {
        return Err(validation(
            "signed artifact statement domain or tenant scope mismatch",
        ));
    }
    Ok(())
}

fn validate_revision_subject(
    revision: &RevisionAttestation,
    expected_kind: &str,
) -> Result<(), CogniGraphError> {
    if revision.kind != expected_kind || !revision.immutable || revision.issued_at_ms == 0 {
        return Err(validation(
            "artifact subject revision kind or immutable status mismatch",
        ));
    }
    for (label, value) in [
        ("revision_id", &revision.revision_id),
        ("source_lineage", &revision.source_lineage),
        ("immutability_method", &revision.immutability_method),
        ("issuer", &revision.issuer),
    ] {
        validate_identifier(&format!("subject.revision.{label}"), value)?;
    }
    validate_location_uri(&revision.manifest_uri)?;
    validate_location_uri(&revision.verification_uri)?;
    validate_digest(
        "subject.revision.manifest_digest",
        &revision.manifest_digest,
    )?;
    validate_digest(
        "subject.revision.verification_digest",
        &revision.verification_digest,
    )?;
    if let Some(digest) = &revision.candidate_digest {
        validate_digest("subject.revision.candidate_digest", digest)?;
    }
    if let Some(digest) = &revision.configuration_digest {
        validate_digest("subject.revision.configuration_digest", digest)?;
    }
    Ok(())
}

fn require_attestor(
    key: &GovernanceKeyRecord,
    actor: &GovernanceActor,
    principal_id: &str,
) -> Result<(), CogniGraphError> {
    if key.verification_key.purpose != KeyPurpose::ArtifactAttestor
        || key.subject_user_key != actor.user_key
        || key.principal_id != principal_id
    {
        return Err(CogniGraphError::Forbidden(
            "authenticated Artifact Attestor does not own the signing principal".into(),
        ));
    }
    Ok(())
}

fn validate_logical_path(path: &str) -> Result<(), CogniGraphError> {
    if path.is_empty()
        || path.len() > MAX_PATH_BYTES
        || !is_nfc(path)
        || path.starts_with('/')
        || path.contains('\\')
        || path.chars().any(char::is_control)
        || path
            .split('/')
            .any(|segment| segment.is_empty() || matches!(segment, "." | ".."))
    {
        return Err(validation(
            "artifact logical_path must be relative, NFC, slash-separated, traversal-free, and portable",
        ));
    }
    Ok(())
}

fn validate_location_uri(uri: &str) -> Result<(), CogniGraphError> {
    validate_text("artifact location uri", uri, MAX_URI_BYTES)?;
    let (scheme, rest) = uri
        .split_once(':')
        .ok_or_else(|| validation("artifact location uri must include an explicit scheme"))?;
    let authority_has_userinfo = rest
        .strip_prefix("//")
        .and_then(|rest| rest.split('/').next())
        .is_some_and(|authority| authority.contains('@'));
    if !scheme
        .as_bytes()
        .first()
        .is_some_and(u8::is_ascii_alphabetic)
        || !scheme
            .bytes()
            .skip(1)
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.'))
        || rest.is_empty()
        || uri.contains(['?', '#'])
        || authority_has_userinfo
        || uri.chars().any(char::is_whitespace)
    {
        return Err(validation(
            "artifact location uri must be absolute and contain no userinfo, query, fragment, whitespace, or credentials",
        ));
    }
    Ok(())
}

fn validate_identifier(label: &str, value: &str) -> Result<(), CogniGraphError> {
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

fn validate_text(label: &str, value: &str, max: usize) -> Result<(), CogniGraphError> {
    if value.trim().is_empty() || value.len() > max || value.chars().any(char::is_control) {
        return Err(validation(format!(
            "{label} must be non-empty, control-free, and at most {max} bytes"
        )));
    }
    Ok(())
}

fn validate_digest(label: &str, value: &str) -> Result<(), CogniGraphError> {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return Err(validation(format!(
            "{label} must use an algorithm-prefixed sha256 digest"
        )));
    };
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(validation(format!(
            "{label} must be sha256:<64 lowercase hex characters>"
        )));
    }
    Ok(())
}

fn validate_record_id(label: &str, value: &str) -> Result<(), CogniGraphError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(validation(format!(
            "{label} must be 64 lowercase hexadecimal characters"
        )));
    }
    Ok(())
}

fn signature_error(error: cognigraph_governance::GovernanceError) -> CogniGraphError {
    CogniGraphError::Forbidden(format!("artifact attestation signature rejected: {error}"))
}

fn stored_signature_error(error: cognigraph_governance::GovernanceError) -> CogniGraphError {
    conflict(format!(
        "stored artifact attestation signature is invalid: {error}"
    ))
}

fn validation(message: impl Into<String>) -> CogniGraphError {
    CogniGraphError::ValidationError(message.into())
}

fn conflict(message: impl Into<String>) -> CogniGraphError {
    CogniGraphError::DocumentConflict(message.into())
}

fn not_found(collection: &str, id: &str) -> CogniGraphError {
    CogniGraphError::DocumentNotFound {
        collection: collection.into(),
        key: id.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(label: &str) -> String {
        digest_bytes(label.as_bytes())
    }

    fn manifest(kind: ArtifactKind) -> ArtifactManifest {
        ArtifactManifest {
            schema_version: 1,
            artifact_kind: kind,
            artifact_format: "cognigraph-test-v1".into(),
            entries: vec![ArtifactManifestEntry {
                logical_path: "data/input.jsonl".into(),
                media_type: "application/x-ndjson".into(),
                byte_length: 4,
                blob_digest: digest("blob"),
                executable: false,
            }],
            entry_count: 1,
            total_bytes: 4,
        }
    }

    #[test]
    fn manifest_is_content_addressed_and_path_safe() {
        let valid = manifest(ArtifactKind::Corpus);
        valid.validate().unwrap();
        let first = canonical_digest(&valid).unwrap();
        let mut changed = valid.clone();
        changed.entries[0].blob_digest = digest("other");
        assert_ne!(first, canonical_digest(&changed).unwrap());

        for path in ["/absolute", "../escape", "a/../b", "a\\b", "a//b"] {
            let mut invalid = valid.clone();
            invalid.entries[0].logical_path = path.into();
            assert!(invalid.validate().is_err(), "accepted unsafe path {path}");
        }
    }

    #[test]
    fn manifest_rejects_case_aliases_and_mismatched_totals() {
        let mut invalid = manifest(ArtifactKind::Graph);
        invalid.entries = vec![
            ArtifactManifestEntry {
                logical_path: "B".into(),
                media_type: "application/octet-stream".into(),
                byte_length: 1,
                blob_digest: digest("b"),
                executable: false,
            },
            ArtifactManifestEntry {
                logical_path: "b".into(),
                media_type: "application/octet-stream".into(),
                byte_length: 1,
                blob_digest: digest("b2"),
                executable: false,
            },
        ];
        invalid.entry_count = 2;
        invalid.total_bytes = 2;
        assert!(invalid.validate().is_err());

        let mut bad_total = manifest(ArtifactKind::Oracle);
        bad_total.total_bytes = 5;
        assert!(bad_total.validate().is_err());
    }

    #[test]
    fn location_is_a_signed_hint_not_a_query_bearing_secret_channel() {
        for uri in [
            "https://user@example.test/a",
            "https://example.test/a?token=secret",
            "https://example.test/a#fragment",
            "1https://example.test/a",
            "+https://example.test/a",
            "relative/path",
        ] {
            assert!(validate_location_uri(uri).is_err(), "accepted {uri}");
        }
        validate_location_uri("s3://bucket/immutable/object").unwrap();
        validate_location_uri("file:///staged/artifact.tar").unwrap();
    }
}
