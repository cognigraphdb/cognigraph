//! Deterministic offline artifact backup, verification, and restoration.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::ffi::{OsStr, OsString};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

use cognigraph_core::CogniGraphError;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use unicode_normalization::is_nfc;
use uuid::Uuid;

use crate::cas::{
    HASH_BUFFER_BYTES, LocalArtifactCas, canonicalize, checked_seen, parse_sha256_digest,
    require_direct_child, require_exact_stream, require_normal_directory, same_file_identity,
    storage_conflict, storage_error, tenant_scope_digest, tenant_scope_hex, validation,
};

pub const ARTIFACT_RECOVERY_PLAN_DOMAIN: &str = "cognigraph.artifact-recovery-plan.v1";
pub const ARTIFACT_BACKUP_RECEIPT_DOMAIN: &str = "cognigraph.artifact-backup-receipt.v1";
pub const ARTIFACT_RESTORE_RECEIPT_DOMAIN: &str = "cognigraph.artifact-restore-receipt.v1";
/// Default upper bound for one evidence-scoped custody operation: two disjoint
/// 1 GiB evaluation inventories.
pub const DEFAULT_MAX_CUSTODY_BYTES: u64 = 2 * 1024 * 1024 * 1024;

const RECOVERY_SCHEMA_VERSION: u32 = 1;
const DIGEST_ALGORITHM: &str = "sha256";
const CUSTODY_FILE: &str = "custody.json";
const BACKUP_RECEIPT_FILE: &str = "backup-receipt.json";
const BLOBS_DIRECTORY: &str = "blobs";
const SHA256_DIRECTORY: &str = "sha256";
const MAX_RECOVERY_ATTESTATIONS: usize = 10;
const MIN_RECOVERY_ATTESTATIONS: usize = 5;
const MAX_RECOVERY_BLOBS: usize = 8_192;
const MAX_RECOVERY_MANIFEST_ENTRIES: u64 = 20_000;
const MAX_RECOVERY_PLAN_BYTES: u64 = 4 * 1024 * 1024;
const MAX_RECOVERY_RECEIPT_BYTES: u64 = 64 * 1024;
const MAX_IDENTIFIER_BYTES: usize = 1_024;
const MIN_RECOVERABLE_EVIDENCE_SCHEMA_VERSION: u32 = 4;
const MAX_RECOVERABLE_EVIDENCE_SCHEMA_VERSION: u32 = 6;

/// One historically validated signed-attestation projection selected by a
/// recovery plan. It contains no signed location or manifest logical path.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactRecoveryAttestation {
    pub kind: String,
    pub attestation_id: String,
    pub attestation_digest: String,
    pub manifest_digest: String,
}

/// One unique content address and its exact byte length.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactRecoveryBlob {
    pub blob_digest: String,
    pub byte_length: u64,
}

/// Closed, content-addressed plan for preserving all external artifact bytes
/// selected by one immutable promotion-evidence record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactRecoveryPlanV1 {
    pub schema_version: u32,
    pub domain: String,
    pub digest_algorithm: String,
    pub tenant: String,
    pub tenant_incarnation: String,
    pub tenant_scope_digest: String,
    pub evidence_id: String,
    pub evidence_digest: String,
    pub evidence_schema_version: u32,
    pub artifact_authority_digest: String,
    pub candidate_artifact_set_digest: String,
    pub baseline_artifact_set_digest: String,
    pub attestations: Vec<ArtifactRecoveryAttestation>,
    pub blobs: Vec<ArtifactRecoveryBlob>,
    pub manifest_entry_count: u64,
    pub blob_count: u64,
    pub total_bytes: u64,
    pub plan_digest: String,
}

impl ArtifactRecoveryPlanV1 {
    /// Construct, canonicalize, and self-digest a recovery plan.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        tenant: impl Into<String>,
        tenant_incarnation: impl Into<String>,
        evidence_id: impl Into<String>,
        evidence_digest: impl Into<String>,
        evidence_schema_version: u32,
        artifact_authority_digest: impl Into<String>,
        candidate_artifact_set_digest: impl Into<String>,
        baseline_artifact_set_digest: impl Into<String>,
        mut attestations: Vec<ArtifactRecoveryAttestation>,
        mut blobs: Vec<ArtifactRecoveryBlob>,
        manifest_entry_count: u64,
    ) -> Result<Self, CogniGraphError> {
        attestations.sort();
        reject_conflicting_attestations(&attestations)?;
        attestations.dedup();

        blobs.sort();
        reject_conflicting_blob_lengths(&blobs)?;
        blobs.dedup();

        let tenant = tenant.into();
        let tenant_incarnation = tenant_incarnation.into();
        let total_bytes = checked_blob_total(&blobs)?;
        let blob_count = u64::try_from(blobs.len())
            .map_err(|_| capacity("artifact recovery blob count does not fit in u64"))?;
        let mut plan = Self {
            schema_version: RECOVERY_SCHEMA_VERSION,
            domain: ARTIFACT_RECOVERY_PLAN_DOMAIN.into(),
            digest_algorithm: DIGEST_ALGORITHM.into(),
            tenant_scope_digest: tenant_scope_digest(&tenant, &tenant_incarnation)?,
            tenant,
            tenant_incarnation,
            evidence_id: evidence_id.into(),
            evidence_digest: evidence_digest.into(),
            evidence_schema_version,
            artifact_authority_digest: artifact_authority_digest.into(),
            candidate_artifact_set_digest: candidate_artifact_set_digest.into(),
            baseline_artifact_set_digest: baseline_artifact_set_digest.into(),
            attestations,
            blobs,
            manifest_entry_count,
            blob_count,
            total_bytes,
            plan_digest: String::new(),
        };
        plan.plan_digest = recovery_plan_digest(&plan)?;
        plan.validate()?;
        Ok(plan)
    }

    /// Validate the closed wire contract and every derived field.
    pub fn validate(&self) -> Result<(), CogniGraphError> {
        if self.schema_version != RECOVERY_SCHEMA_VERSION
            || self.domain != ARTIFACT_RECOVERY_PLAN_DOMAIN
            || self.digest_algorithm != DIGEST_ALGORITHM
        {
            return Err(validation(
                "artifact recovery plan schema, domain, or digest algorithm is unsupported",
            ));
        }
        validate_identifier("artifact recovery tenant", &self.tenant)?;
        validate_identifier(
            "artifact recovery tenant incarnation",
            &self.tenant_incarnation,
        )?;
        validate_record_id("artifact recovery evidence id", &self.evidence_id)?;
        for (label, digest) in [
            ("tenant_scope_digest", &self.tenant_scope_digest),
            ("evidence_digest", &self.evidence_digest),
            ("artifact_authority_digest", &self.artifact_authority_digest),
            (
                "candidate_artifact_set_digest",
                &self.candidate_artifact_set_digest,
            ),
            (
                "baseline_artifact_set_digest",
                &self.baseline_artifact_set_digest,
            ),
            ("plan_digest", &self.plan_digest),
        ] {
            validate_digest(label, digest)?;
        }
        if self.tenant_scope_digest != tenant_scope_digest(&self.tenant, &self.tenant_incarnation)?
        {
            return Err(validation(
                "artifact recovery tenant scope digest does not match tenant and incarnation",
            ));
        }
        if !(MIN_RECOVERABLE_EVIDENCE_SCHEMA_VERSION..=MAX_RECOVERABLE_EVIDENCE_SCHEMA_VERSION)
            .contains(&self.evidence_schema_version)
        {
            return Err(validation(
                "artifact recovery requires M21-M23 promotion evidence schema version 4, 5, or 6",
            ));
        }
        validate_attestations(&self.attestations)?;
        validate_blobs(&self.blobs)?;
        if self.manifest_entry_count < self.attestations.len() as u64
            || self.manifest_entry_count > MAX_RECOVERY_MANIFEST_ENTRIES
            || self.blob_count == 0
            || self.blob_count > self.manifest_entry_count
            || self.blob_count != self.blobs.len() as u64
            || self.total_bytes == 0
            || self.total_bytes != checked_blob_total(&self.blobs)?
        {
            return Err(validation(
                "artifact recovery counts or checked total bytes are inconsistent or out of bounds",
            ));
        }
        if self.plan_digest != recovery_plan_digest(self)? {
            return Err(validation("artifact recovery plan digest mismatch"));
        }
        if self.canonical_bytes()?.len() as u64 > MAX_RECOVERY_PLAN_BYTES {
            return Err(capacity(
                "artifact recovery plan exceeds its canonical byte limit",
            ));
        }
        Ok(())
    }

    /// Canonical NFC JSON bytes used for the on-disk `custody.json` file.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, CogniGraphError> {
        canonical_bytes(self)
    }

    /// Parse only an exact canonical wire representation.
    pub fn from_canonical_bytes(bytes: &[u8]) -> Result<Self, CogniGraphError> {
        if bytes.len() as u64 > MAX_RECOVERY_PLAN_BYTES {
            return Err(capacity("artifact recovery plan exceeds its byte limit"));
        }
        let plan: Self = serde_json::from_slice(bytes)?;
        plan.validate()?;
        if plan.canonical_bytes()? != bytes {
            return Err(validation(
                "artifact recovery plan must use exact canonical JSON encoding",
            ));
        }
        Ok(plan)
    }

    fn validate_operation_limit(&self, max_total_bytes: u64) -> Result<(), CogniGraphError> {
        validate_operation_limit(max_total_bytes)?;
        if self.total_bytes > max_total_bytes {
            return Err(capacity(format!(
                "artifact recovery requires {} bytes, exceeding the {max_total_bytes} byte operation limit",
                self.total_bytes
            )));
        }
        Ok(())
    }
}

/// Deterministic operation receipt emitted only after every bundle blob was
/// copied and reread successfully. It has no trusted timestamp and is not a
/// signature or availability claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactBackupReceiptV1 {
    pub schema_version: u32,
    pub domain: String,
    pub digest_algorithm: String,
    pub tenant: String,
    pub tenant_incarnation: String,
    pub tenant_scope_digest: String,
    pub evidence_id: String,
    pub evidence_digest: String,
    pub plan_digest: String,
    pub blob_count: u64,
    pub total_bytes: u64,
    pub receipt_digest: String,
}

impl ArtifactBackupReceiptV1 {
    fn new(plan: &ArtifactRecoveryPlanV1) -> Result<Self, CogniGraphError> {
        let mut receipt = Self {
            schema_version: RECOVERY_SCHEMA_VERSION,
            domain: ARTIFACT_BACKUP_RECEIPT_DOMAIN.into(),
            digest_algorithm: DIGEST_ALGORITHM.into(),
            tenant: plan.tenant.clone(),
            tenant_incarnation: plan.tenant_incarnation.clone(),
            tenant_scope_digest: plan.tenant_scope_digest.clone(),
            evidence_id: plan.evidence_id.clone(),
            evidence_digest: plan.evidence_digest.clone(),
            plan_digest: plan.plan_digest.clone(),
            blob_count: plan.blob_count,
            total_bytes: plan.total_bytes,
            receipt_digest: String::new(),
        };
        receipt.receipt_digest = backup_receipt_digest(&receipt)?;
        receipt.validate_against(plan)?;
        Ok(receipt)
    }

    pub fn validate_against(&self, plan: &ArtifactRecoveryPlanV1) -> Result<(), CogniGraphError> {
        plan.validate()?;
        if self.schema_version != RECOVERY_SCHEMA_VERSION
            || self.domain != ARTIFACT_BACKUP_RECEIPT_DOMAIN
            || self.digest_algorithm != DIGEST_ALGORITHM
            || self.tenant != plan.tenant
            || self.tenant_incarnation != plan.tenant_incarnation
            || self.tenant_scope_digest != plan.tenant_scope_digest
            || self.evidence_id != plan.evidence_id
            || self.evidence_digest != plan.evidence_digest
            || self.plan_digest != plan.plan_digest
            || self.blob_count != plan.blob_count
            || self.total_bytes != plan.total_bytes
        {
            return Err(validation(
                "artifact backup receipt does not match its recovery plan",
            ));
        }
        validate_digest("backup receipt digest", &self.receipt_digest)?;
        if self.receipt_digest != backup_receipt_digest(self)? {
            return Err(validation("artifact backup receipt digest mismatch"));
        }
        Ok(())
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, CogniGraphError> {
        canonical_bytes(self)
    }
}

/// Deterministic operation receipt returned only after the final restored CAS
/// scope was published and reread successfully. It has no trusted timestamp.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactRestoreReceiptV1 {
    pub schema_version: u32,
    pub domain: String,
    pub digest_algorithm: String,
    pub tenant: String,
    pub tenant_incarnation: String,
    pub tenant_scope_digest: String,
    pub evidence_id: String,
    pub evidence_digest: String,
    pub plan_digest: String,
    pub backup_receipt_digest: String,
    pub blob_count: u64,
    pub total_bytes: u64,
    pub receipt_digest: String,
}

impl ArtifactRestoreReceiptV1 {
    fn new(
        plan: &ArtifactRecoveryPlanV1,
        backup: &ArtifactBackupReceiptV1,
    ) -> Result<Self, CogniGraphError> {
        backup.validate_against(plan)?;
        let mut receipt = Self {
            schema_version: RECOVERY_SCHEMA_VERSION,
            domain: ARTIFACT_RESTORE_RECEIPT_DOMAIN.into(),
            digest_algorithm: DIGEST_ALGORITHM.into(),
            tenant: plan.tenant.clone(),
            tenant_incarnation: plan.tenant_incarnation.clone(),
            tenant_scope_digest: plan.tenant_scope_digest.clone(),
            evidence_id: plan.evidence_id.clone(),
            evidence_digest: plan.evidence_digest.clone(),
            plan_digest: plan.plan_digest.clone(),
            backup_receipt_digest: backup.receipt_digest.clone(),
            blob_count: plan.blob_count,
            total_bytes: plan.total_bytes,
            receipt_digest: String::new(),
        };
        receipt.receipt_digest = restore_receipt_digest(&receipt)?;
        receipt.validate_against(plan, backup)?;
        Ok(receipt)
    }

    pub fn validate_against(
        &self,
        plan: &ArtifactRecoveryPlanV1,
        backup: &ArtifactBackupReceiptV1,
    ) -> Result<(), CogniGraphError> {
        backup.validate_against(plan)?;
        if self.schema_version != RECOVERY_SCHEMA_VERSION
            || self.domain != ARTIFACT_RESTORE_RECEIPT_DOMAIN
            || self.digest_algorithm != DIGEST_ALGORITHM
            || self.tenant != plan.tenant
            || self.tenant_incarnation != plan.tenant_incarnation
            || self.tenant_scope_digest != plan.tenant_scope_digest
            || self.evidence_id != plan.evidence_id
            || self.evidence_digest != plan.evidence_digest
            || self.plan_digest != plan.plan_digest
            || self.backup_receipt_digest != backup.receipt_digest
            || self.blob_count != plan.blob_count
            || self.total_bytes != plan.total_bytes
        {
            return Err(validation(
                "artifact restore receipt does not match its recovery plan and backup receipt",
            ));
        }
        validate_digest("restore receipt digest", &self.receipt_digest)?;
        if self.receipt_digest != restore_receipt_digest(self)? {
            return Err(validation("artifact restore receipt digest mismatch"));
        }
        Ok(())
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, CogniGraphError> {
        canonical_bytes(self)
    }
}

/// Create and atomically publish a closed artifact recovery bundle.
///
/// `destination` must be absolute and absent. Its parent must be an existing
/// normal directory outside the source CAS tree.
pub fn create_bundle(
    source: &LocalArtifactCas,
    plan: &ArtifactRecoveryPlanV1,
    destination: impl AsRef<Path>,
    max_total_bytes: u64,
) -> Result<ArtifactBackupReceiptV1, CogniGraphError> {
    plan.validate()?;
    plan.validate_operation_limit(max_total_bytes)?;
    let destination = absent_destination(destination.as_ref(), "artifact recovery bundle")?;
    reject_nested_paths(
        source.root(),
        &destination.path,
        "source CAS",
        "recovery bundle",
    )?;

    let stage = create_stage(&destination.parent, ".cognigraph-backup")?;
    let mut published = false;
    let result = (|| {
        let blobs_root = stage.join(BLOBS_DIRECTORY);
        let algorithm_root = blobs_root.join(SHA256_DIRECTORY);
        create_private_directory(&blobs_root)?;
        create_private_directory(&algorithm_root)?;

        copy_plan_from_cas(source, plan, &algorithm_root)?;
        write_canonical_file(&stage.join(CUSTODY_FILE), &plan.canonical_bytes()?)?;
        verify_blob_tree(&algorithm_root, &plan.blobs)?;

        let receipt = ArtifactBackupReceiptV1::new(plan)?;
        write_canonical_file(
            &stage.join(BACKUP_RECEIPT_FILE),
            &receipt.canonical_bytes()?,
        )?;
        sync_bundle_tree(&stage, &plan.blobs)?;

        let expected =
            RecoveryExpectation::new(&plan.plan_digest, &plan.tenant, &plan.tenant_incarnation)?;
        let staged_receipt = verify_bundle_inner(&stage, &expected, max_total_bytes)?;
        if staged_receipt != receipt {
            return Err(storage_conflict(
                "staged artifact backup receipt changed before publication",
            ));
        }

        atomic_publish_no_replace(&stage, &destination.path)?;
        published = true;
        sync_directory_after_publication(&destination.parent, "artifact recovery bundle")?;

        let final_receipt = verify_bundle_inner(&destination.path, &expected, max_total_bytes)
            .map_err(|error| {
                published_but_unverified("artifact recovery bundle", &destination.path, error)
            })?;
        if final_receipt != receipt {
            return Err(storage_conflict(
                "published artifact backup receipt changed during final reread",
            ));
        }
        Ok(receipt)
    })();
    if !published {
        cleanup_stage(&stage);
    }
    result
}

/// Verify a closed bundle against an independently pinned plan digest and
/// tenant-incarnation identity.
pub fn verify_bundle(
    bundle: impl AsRef<Path>,
    expected_plan_digest: &str,
    expected_tenant: &str,
    expected_tenant_incarnation: &str,
    max_total_bytes: u64,
) -> Result<ArtifactBackupReceiptV1, CogniGraphError> {
    let expected = RecoveryExpectation::new(
        expected_plan_digest,
        expected_tenant,
        expected_tenant_incarnation,
    )?;
    verify_bundle_inner(bundle.as_ref(), &expected, max_total_bytes)
}

/// Restore a verified bundle into one absent tenant scope and atomically
/// publish the complete scope. Existing scopes are never merged or replaced.
pub fn restore_bundle(
    bundle: impl AsRef<Path>,
    destination_cas_root: impl AsRef<Path>,
    expected_plan_digest: &str,
    expected_tenant: &str,
    expected_tenant_incarnation: &str,
    max_total_bytes: u64,
) -> Result<ArtifactRestoreReceiptV1, CogniGraphError> {
    let expected = RecoveryExpectation::new(
        expected_plan_digest,
        expected_tenant,
        expected_tenant_incarnation,
    )?;
    let canonical_bundle = checked_existing_root(bundle.as_ref(), "artifact recovery bundle")?;
    let backup = verify_bundle_inner(&canonical_bundle, &expected, max_total_bytes)?;
    let plan = read_plan(&canonical_bundle.join(CUSTODY_FILE))?;
    expected.require_plan(&plan)?;
    let restore_receipt = ArtifactRestoreReceiptV1::new(&plan, &backup)?;

    let destination_cas = LocalArtifactCas::open(destination_cas_root, max_total_bytes)?;
    reject_nested_paths(
        &canonical_bundle,
        destination_cas.root(),
        "recovery bundle",
        "destination CAS",
    )?;
    let scope_hex = tenant_scope_hex(&plan.tenant, &plan.tenant_incarnation)?;
    let final_scope = destination_cas.tenants_root().join(&scope_hex);
    require_absent(&final_scope, "destination artifact tenant scope")?;

    let stage = create_stage(
        destination_cas.tenants_root(),
        &format!(".cognigraph-restore-{scope_hex}"),
    )?;
    let mut published = false;
    let result = (|| {
        let stage_algorithm = stage.join(SHA256_DIRECTORY);
        create_private_directory(&stage_algorithm)?;
        let bundle_algorithm = canonical_bundle
            .join(BLOBS_DIRECTORY)
            .join(SHA256_DIRECTORY);
        copy_plan_from_bundle(&bundle_algorithm, &plan.blobs, &stage_algorithm)?;
        verify_scope_tree(&stage, &plan.blobs)?;
        sync_scope_tree(&stage, &plan.blobs)?;

        atomic_publish_no_replace(&stage, &final_scope)?;
        published = true;
        sync_directory_after_publication(
            destination_cas.tenants_root(),
            "restored artifact tenant scope",
        )?;

        let final_cas = LocalArtifactCas::open(destination_cas.root(), max_total_bytes)?;
        verify_plan_in_cas(&final_cas, &plan).map_err(|error| {
            published_but_unverified("restored artifact tenant scope", &final_scope, error)
        })?;
        Ok(restore_receipt)
    })();
    if !published {
        cleanup_stage(&stage);
    }
    result
}

#[derive(Debug)]
struct RecoveryExpectation<'a> {
    plan_digest: &'a str,
    tenant: &'a str,
    tenant_incarnation: &'a str,
}

impl<'a> RecoveryExpectation<'a> {
    fn new(
        plan_digest: &'a str,
        tenant: &'a str,
        tenant_incarnation: &'a str,
    ) -> Result<Self, CogniGraphError> {
        validate_digest("expected artifact recovery plan digest", plan_digest)?;
        validate_identifier("expected artifact recovery tenant", tenant)?;
        validate_identifier(
            "expected artifact recovery tenant incarnation",
            tenant_incarnation,
        )?;
        Ok(Self {
            plan_digest,
            tenant,
            tenant_incarnation,
        })
    }

    fn require_plan(&self, plan: &ArtifactRecoveryPlanV1) -> Result<(), CogniGraphError> {
        if plan.plan_digest != self.plan_digest
            || plan.tenant != self.tenant
            || plan.tenant_incarnation != self.tenant_incarnation
        {
            return Err(storage_conflict(
                "artifact recovery bundle does not match the independently pinned plan and tenant scope",
            ));
        }
        Ok(())
    }
}

#[derive(Debug)]
struct AbsentDestination {
    parent: PathBuf,
    path: PathBuf,
}

fn absent_destination(path: &Path, label: &str) -> Result<AbsentDestination, CogniGraphError> {
    if !path.is_absolute() {
        return Err(validation(format!("{label} path must be absolute")));
    }
    let requested_parent = path
        .parent()
        .ok_or_else(|| validation(format!("{label} must have a parent directory")))?;
    require_normal_directory(requested_parent, &format!("{label} parent"))?;
    let parent = canonicalize(requested_parent, &format!("{label} parent"))?;
    let name = path
        .file_name()
        .ok_or_else(|| validation(format!("{label} must have a normal final component")))?;
    require_normal_component(name, label)?;
    let path = parent.join(name);
    require_absent(&path, label)?;
    Ok(AbsentDestination { parent, path })
}

fn checked_existing_root(path: &Path, label: &str) -> Result<PathBuf, CogniGraphError> {
    if !path.is_absolute() {
        return Err(validation(format!("{label} path must be absolute")));
    }
    require_normal_directory(path, label)?;
    canonicalize(path, label)
}

fn require_normal_component(component: &OsStr, label: &str) -> Result<(), CogniGraphError> {
    let mut components = Path::new(component).components();
    if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() {
        return Err(validation(format!("{label} has an unsafe final component")));
    }
    Ok(())
}

fn require_absent(path: &Path, label: &str) -> Result<(), CogniGraphError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Err(storage_conflict(format!("{label} already exists"))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(storage_error(format!("cannot inspect {label}"), error)),
    }
}

fn reject_nested_paths(
    left: &Path,
    right: &Path,
    left_label: &str,
    right_label: &str,
) -> Result<(), CogniGraphError> {
    if left.starts_with(right) || right.starts_with(left) {
        return Err(validation(format!(
            "{left_label} and {right_label} cannot contain one another"
        )));
    }
    Ok(())
}

fn create_stage(parent: &Path, prefix: &str) -> Result<PathBuf, CogniGraphError> {
    require_normal_directory(parent, "artifact recovery staging parent")?;
    for _ in 0..16 {
        let path = parent.join(format!("{prefix}-{}.partial", Uuid::new_v4()));
        match create_private_directory(&path) {
            Ok(()) => return Ok(path),
            Err(CogniGraphError::BackendError(message))
                if message.contains("File exists") || message.contains("already exists") => {}
            Err(error) => return Err(error),
        }
    }
    Err(storage_conflict(
        "could not allocate a unique artifact recovery staging directory",
    ))
}

fn create_private_directory(path: &Path) -> Result<(), CogniGraphError> {
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt as _;
        builder.mode(0o700);
    }
    builder
        .create(path)
        .map_err(|error| storage_error("cannot create artifact recovery directory", error))
}

fn create_private_file(path: &Path) -> Result<File, CogniGraphError> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    options
        .open(path)
        .map_err(|error| storage_error("cannot create artifact recovery file", error))
}

fn copy_plan_from_cas(
    source: &LocalArtifactCas,
    plan: &ArtifactRecoveryPlanV1,
    destination_algorithm: &Path,
) -> Result<(), CogniGraphError> {
    let mut created_shards = BTreeSet::new();
    for blob in &plan.blobs {
        let digest_hex = parse_sha256_digest(&blob.blob_digest)?;
        let shard =
            ensure_private_shard(destination_algorithm, &digest_hex[..2], &mut created_shards)?;
        let destination_path = shard.join(&digest_hex[2..]);
        let mut destination = create_private_file(&destination_path)?;
        let mut source_file = source.open_checked_blob(
            &plan.tenant,
            &plan.tenant_incarnation,
            &blob.blob_digest,
            blob.byte_length,
        )?;
        copy_and_verify(
            &mut source_file,
            &mut destination,
            &blob.blob_digest,
            blob.byte_length,
        )?;
        destination
            .sync_all()
            .map_err(|error| storage_error("cannot sync artifact backup blob", error))?;
        source.recheck_parent_chain(
            tenant_scope_hex(&plan.tenant, &plan.tenant_incarnation)?.as_str(),
            digest_hex,
        )?;
    }
    Ok(())
}

fn copy_plan_from_bundle(
    source_algorithm: &Path,
    blobs: &[ArtifactRecoveryBlob],
    destination_algorithm: &Path,
) -> Result<(), CogniGraphError> {
    let mut created_shards = BTreeSet::new();
    for blob in blobs {
        let digest_hex = parse_sha256_digest(&blob.blob_digest)?;
        let source_shard = checked_named_directory(
            source_algorithm,
            &digest_hex[..2],
            "artifact recovery bundle shard",
        )?;
        let source_path = source_shard.join(&digest_hex[2..]);
        let mut source = open_checked_file(
            &source_path,
            &source_shard,
            blob.byte_length,
            "artifact recovery bundle blob",
        )?;
        let destination_shard =
            ensure_private_shard(destination_algorithm, &digest_hex[..2], &mut created_shards)?;
        let mut destination = create_private_file(&destination_shard.join(&digest_hex[2..]))?;
        copy_and_verify(
            &mut source,
            &mut destination,
            &blob.blob_digest,
            blob.byte_length,
        )?;
        destination
            .sync_all()
            .map_err(|error| storage_error("cannot sync restored artifact blob", error))?;
    }
    Ok(())
}

fn ensure_private_shard(
    algorithm_root: &Path,
    shard: &str,
    created: &mut BTreeSet<String>,
) -> Result<PathBuf, CogniGraphError> {
    let path = algorithm_root.join(shard);
    if created.insert(shard.to_string()) {
        create_private_directory(&path)?;
    }
    Ok(path)
}

fn copy_and_verify(
    source: &mut File,
    destination: &mut File,
    expected_digest: &str,
    expected_length: u64,
) -> Result<(), CogniGraphError> {
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; HASH_BUFFER_BYTES];
    let mut seen = 0_u64;
    loop {
        let read = source
            .read(&mut buffer)
            .map_err(|error| storage_error("cannot read artifact recovery source blob", error))?;
        if read == 0 {
            break;
        }
        seen = checked_seen(seen, read, expected_length)?;
        hasher.update(&buffer[..read]);
        destination.write_all(&buffer[..read]).map_err(|error| {
            storage_error("cannot write artifact recovery destination blob", error)
        })?;
    }
    require_exact_stream(expected_digest, expected_length, seen, hasher.finalize())
}

fn verify_bundle_inner(
    bundle: &Path,
    expected: &RecoveryExpectation<'_>,
    max_total_bytes: u64,
) -> Result<ArtifactBackupReceiptV1, CogniGraphError> {
    validate_operation_limit(max_total_bytes)?;
    let bundle = checked_existing_root(bundle, "artifact recovery bundle")?;
    let plan = read_plan(&bundle.join(CUSTODY_FILE))?;
    plan.validate_operation_limit(max_total_bytes)?;
    expected.require_plan(&plan)?;
    let receipt = read_backup_receipt(&bundle.join(BACKUP_RECEIPT_FILE))?;
    receipt.validate_against(&plan)?;
    verify_closed_bundle_tree(&bundle, &plan.blobs)?;
    verify_blob_tree(
        &bundle.join(BLOBS_DIRECTORY).join(SHA256_DIRECTORY),
        &plan.blobs,
    )?;
    Ok(receipt)
}

fn read_plan(path: &Path) -> Result<ArtifactRecoveryPlanV1, CogniGraphError> {
    let bytes = read_bounded_normal_file(path, MAX_RECOVERY_PLAN_BYTES, "artifact recovery plan")?;
    ArtifactRecoveryPlanV1::from_canonical_bytes(&bytes)
}

fn read_backup_receipt(path: &Path) -> Result<ArtifactBackupReceiptV1, CogniGraphError> {
    let bytes =
        read_bounded_normal_file(path, MAX_RECOVERY_RECEIPT_BYTES, "artifact backup receipt")?;
    let receipt: ArtifactBackupReceiptV1 = serde_json::from_slice(&bytes)?;
    if receipt.canonical_bytes()? != bytes {
        return Err(validation(
            "artifact backup receipt must use exact canonical JSON encoding",
        ));
    }
    Ok(receipt)
}

fn read_bounded_normal_file(
    path: &Path,
    max_bytes: u64,
    label: &str,
) -> Result<Vec<u8>, CogniGraphError> {
    let parent = path
        .parent()
        .ok_or_else(|| validation(format!("{label} has no parent")))?;
    let before = fs::symlink_metadata(path)
        .map_err(|error| storage_error(format!("cannot inspect {label}"), error))?;
    if before.file_type().is_symlink() || !before.is_file() {
        return Err(storage_conflict(format!(
            "{label} must be a regular non-symlink file"
        )));
    }
    if before.len() > max_bytes {
        return Err(capacity(format!(
            "{label} exceeds its {max_bytes} byte limit"
        )));
    }
    let mut file = open_checked_file(path, parent, before.len(), label)?;
    let capacity = usize::try_from(before.len())
        .map_err(|_| capacity(format!("{label} is too large for this platform")))?;
    let mut bytes = Vec::with_capacity(capacity);
    file.read_to_end(&mut bytes)
        .map_err(|error| storage_error(format!("cannot read {label}"), error))?;
    if bytes.len() as u64 != before.len() {
        return Err(storage_conflict(format!(
            "{label} length changed while it was read"
        )));
    }
    Ok(bytes)
}

fn open_checked_file(
    path: &Path,
    expected_parent: &Path,
    expected_length: u64,
    label: &str,
) -> Result<File, CogniGraphError> {
    let before = fs::symlink_metadata(path)
        .map_err(|error| storage_error(format!("cannot inspect {label}"), error))?;
    if before.file_type().is_symlink() || !before.is_file() || before.len() != expected_length {
        return Err(storage_conflict(format!(
            "{label} type or length does not match its recovery plan"
        )));
    }
    let canonical = canonicalize(path, label)?;
    require_direct_child(expected_parent, &canonical, label)?;
    let file = File::open(&canonical)
        .map_err(|error| storage_error(format!("cannot open {label}"), error))?;
    let opened = file
        .metadata()
        .map_err(|error| storage_error(format!("cannot inspect opened {label}"), error))?;
    if !opened.is_file() || opened.len() != expected_length || !same_file_identity(&before, &opened)
    {
        return Err(storage_conflict(format!(
            "{label} changed while it was being opened"
        )));
    }
    Ok(file)
}

fn verify_blob_tree(
    algorithm_root: &Path,
    blobs: &[ArtifactRecoveryBlob],
) -> Result<(), CogniGraphError> {
    for blob in blobs {
        let digest_hex = parse_sha256_digest(&blob.blob_digest)?;
        let shard = checked_named_directory(
            algorithm_root,
            &digest_hex[..2],
            "artifact recovery SHA-256 shard",
        )?;
        let path = shard.join(&digest_hex[2..]);
        let mut file =
            open_checked_file(&path, &shard, blob.byte_length, "artifact recovery blob")?;
        hash_reader(&mut file, &blob.blob_digest, blob.byte_length)?;
    }
    Ok(())
}

fn verify_plan_in_cas(
    cas: &LocalArtifactCas,
    plan: &ArtifactRecoveryPlanV1,
) -> Result<(), CogniGraphError> {
    for blob in &plan.blobs {
        let mut file = cas.open_checked_blob(
            &plan.tenant,
            &plan.tenant_incarnation,
            &blob.blob_digest,
            blob.byte_length,
        )?;
        hash_reader(&mut file, &blob.blob_digest, blob.byte_length)?;
    }
    Ok(())
}

fn hash_reader(
    file: &mut File,
    expected_digest: &str,
    expected_length: u64,
) -> Result<(), CogniGraphError> {
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; HASH_BUFFER_BYTES];
    let mut seen = 0_u64;
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| storage_error("cannot read artifact recovery blob", error))?;
        if read == 0 {
            break;
        }
        seen = checked_seen(seen, read, expected_length)?;
        hasher.update(&buffer[..read]);
    }
    require_exact_stream(expected_digest, expected_length, seen, hasher.finalize())
}

fn checked_named_directory(
    parent: &Path,
    name: &str,
    label: &str,
) -> Result<PathBuf, CogniGraphError> {
    let path = parent.join(name);
    require_normal_directory(&path, label)?;
    let canonical = canonicalize(&path, label)?;
    require_direct_child(parent, &canonical, label)?;
    Ok(canonical)
}

fn verify_closed_bundle_tree(
    bundle: &Path,
    blobs: &[ArtifactRecoveryBlob],
) -> Result<(), CogniGraphError> {
    require_exact_entries(
        bundle,
        &[
            (OsString::from(CUSTODY_FILE), ExpectedEntry::File),
            (OsString::from(BACKUP_RECEIPT_FILE), ExpectedEntry::File),
            (OsString::from(BLOBS_DIRECTORY), ExpectedEntry::Directory),
        ],
        "artifact recovery bundle",
    )?;
    let blobs_root = checked_named_directory(bundle, BLOBS_DIRECTORY, "artifact blobs directory")?;
    require_exact_entries(
        &blobs_root,
        &[(OsString::from(SHA256_DIRECTORY), ExpectedEntry::Directory)],
        "artifact blobs directory",
    )?;
    let algorithm =
        checked_named_directory(&blobs_root, SHA256_DIRECTORY, "artifact SHA-256 directory")?;
    verify_closed_algorithm_tree(&algorithm, blobs)
}

fn verify_scope_tree(scope: &Path, blobs: &[ArtifactRecoveryBlob]) -> Result<(), CogniGraphError> {
    require_exact_entries(
        scope,
        &[(OsString::from(SHA256_DIRECTORY), ExpectedEntry::Directory)],
        "restored artifact tenant scope",
    )?;
    let algorithm = checked_named_directory(scope, SHA256_DIRECTORY, "restored SHA-256 directory")?;
    verify_closed_algorithm_tree(&algorithm, blobs)?;
    verify_blob_tree(&algorithm, blobs)
}

fn verify_closed_algorithm_tree(
    algorithm: &Path,
    blobs: &[ArtifactRecoveryBlob],
) -> Result<(), CogniGraphError> {
    let mut expected_by_shard = BTreeMap::<String, BTreeSet<String>>::new();
    for blob in blobs {
        let hex = parse_sha256_digest(&blob.blob_digest)?;
        expected_by_shard
            .entry(hex[..2].to_string())
            .or_default()
            .insert(hex[2..].to_string());
    }
    let expected_shards = expected_by_shard
        .keys()
        .map(|name| (OsString::from(name), ExpectedEntry::Directory))
        .collect::<Vec<_>>();
    require_exact_entries(
        algorithm,
        &expected_shards,
        "artifact recovery SHA-256 directory",
    )?;
    for (shard, names) in expected_by_shard {
        let shard_path =
            checked_named_directory(algorithm, &shard, "artifact recovery SHA-256 shard")?;
        let expected_files = names
            .into_iter()
            .map(|name| (OsString::from(name), ExpectedEntry::File))
            .collect::<Vec<_>>();
        require_exact_entries(
            &shard_path,
            &expected_files,
            "artifact recovery SHA-256 shard",
        )?;
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExpectedEntry {
    File,
    Directory,
}

fn require_exact_entries(
    directory: &Path,
    expected: &[(OsString, ExpectedEntry)],
    label: &str,
) -> Result<(), CogniGraphError> {
    let expected = expected.iter().cloned().collect::<BTreeMap<_, _>>();
    let mut observed = BTreeMap::new();
    let entries = fs::read_dir(directory)
        .map_err(|error| storage_error(format!("cannot enumerate {label}"), error))?;
    for entry in entries {
        let entry = entry
            .map_err(|error| storage_error(format!("cannot inspect an entry in {label}"), error))?;
        let name = entry.file_name();
        if observed.contains_key(&name) {
            return Err(storage_conflict(format!(
                "{label} contains a duplicate entry"
            )));
        }
        let metadata = fs::symlink_metadata(entry.path())
            .map_err(|error| storage_error(format!("cannot inspect an entry in {label}"), error))?;
        let kind = if metadata.file_type().is_symlink() {
            return Err(storage_conflict(format!(
                "{label} contains a forbidden symlink"
            )));
        } else if metadata.is_file() {
            ExpectedEntry::File
        } else if metadata.is_dir() {
            ExpectedEntry::Directory
        } else {
            return Err(storage_conflict(format!(
                "{label} contains a non-file, non-directory entry"
            )));
        };
        observed.insert(name, kind);
    }
    if observed != expected {
        return Err(storage_conflict(format!(
            "{label} is not a closed tree with exactly the expected entries"
        )));
    }
    Ok(())
}

fn write_canonical_file(path: &Path, bytes: &[u8]) -> Result<(), CogniGraphError> {
    let mut file = create_private_file(path)?;
    file.write_all(bytes)
        .map_err(|error| storage_error("cannot write artifact recovery metadata", error))?;
    file.sync_all()
        .map_err(|error| storage_error("cannot sync artifact recovery metadata", error))
}

fn sync_bundle_tree(bundle: &Path, blobs: &[ArtifactRecoveryBlob]) -> Result<(), CogniGraphError> {
    let algorithm = bundle.join(BLOBS_DIRECTORY).join(SHA256_DIRECTORY);
    sync_algorithm_tree(&algorithm, blobs)?;
    sync_directory(&bundle.join(BLOBS_DIRECTORY))?;
    sync_directory(bundle)
}

fn sync_scope_tree(scope: &Path, blobs: &[ArtifactRecoveryBlob]) -> Result<(), CogniGraphError> {
    sync_algorithm_tree(&scope.join(SHA256_DIRECTORY), blobs)?;
    sync_directory(scope)
}

fn sync_algorithm_tree(
    algorithm: &Path,
    blobs: &[ArtifactRecoveryBlob],
) -> Result<(), CogniGraphError> {
    let shards = blobs
        .iter()
        .map(|blob| parse_sha256_digest(&blob.blob_digest).map(|hex| hex[..2].to_string()))
        .collect::<Result<BTreeSet<_>, _>>()?;
    for shard in shards {
        sync_directory(&algorithm.join(shard))?;
    }
    sync_directory(algorithm)
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> Result<(), CogniGraphError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| storage_error("cannot sync artifact recovery directory", error))
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> Result<(), CogniGraphError> {
    Ok(())
}

fn sync_directory_after_publication(path: &Path, label: &str) -> Result<(), CogniGraphError> {
    sync_directory(path).map_err(|error| {
        CogniGraphError::BackendError(format!(
            "{label} was published, but parent-directory durability is uncertain: {error}"
        ))
    })
}

fn published_but_unverified(label: &str, path: &Path, error: CogniGraphError) -> CogniGraphError {
    CogniGraphError::BackendError(format!(
        "{label} was published at {}, but its final verification failed: {error}; inspect or verify that exact path and do not retry by overwriting it",
        path.display()
    ))
}

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "ios"
))]
fn atomic_publish_no_replace(stage: &Path, destination: &Path) -> Result<(), CogniGraphError> {
    use rustix::fs::{RenameFlags, renameat_with};

    let parent = stage
        .parent()
        .ok_or_else(|| validation("artifact recovery stage has no parent"))?;
    if destination.parent() != Some(parent) {
        return Err(validation(
            "artifact recovery publication must rename within one parent directory",
        ));
    }
    let parent_file = File::open(parent)
        .map_err(|error| storage_error("cannot open artifact publication parent", error))?;
    let stage_name = stage
        .file_name()
        .ok_or_else(|| validation("artifact recovery stage has no file name"))?;
    let destination_name = destination
        .file_name()
        .ok_or_else(|| validation("artifact recovery destination has no file name"))?;
    renameat_with(
        &parent_file,
        stage_name,
        &parent_file,
        destination_name,
        RenameFlags::NOREPLACE,
    )
    .map_err(|error| {
        storage_error(
            "cannot atomically publish artifact recovery directory without replacement",
            std::io::Error::from_raw_os_error(error.raw_os_error()),
        )
    })
}

#[cfg(windows)]
fn atomic_publish_no_replace(_stage: &Path, _destination: &Path) -> Result<(), CogniGraphError> {
    Err(CogniGraphError::BackendError(
        "atomic no-replace directory publication is not implemented safely on Windows".into(),
    ))
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "ios",
    windows
)))]
fn atomic_publish_no_replace(_stage: &Path, _destination: &Path) -> Result<(), CogniGraphError> {
    Err(CogniGraphError::BackendError(
        "atomic no-replace directory publication is unsupported on this platform".into(),
    ))
}

fn cleanup_stage(stage: &Path) {
    let _ = fs::remove_dir_all(stage);
}

fn recovery_plan_digest(plan: &ArtifactRecoveryPlanV1) -> Result<String, CogniGraphError> {
    typed_record_digest(plan, "plan_digest")
}

fn backup_receipt_digest(receipt: &ArtifactBackupReceiptV1) -> Result<String, CogniGraphError> {
    typed_record_digest(receipt, "receipt_digest")
}

fn restore_receipt_digest(receipt: &ArtifactRestoreReceiptV1) -> Result<String, CogniGraphError> {
    typed_record_digest(receipt, "receipt_digest")
}

fn typed_record_digest<T: Serialize>(
    value: &T,
    digest_field: &str,
) -> Result<String, CogniGraphError> {
    let mut value = serde_json::to_value(value)?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| validation("artifact recovery record must serialize as an object"))?;
    if object.remove(digest_field).is_none() {
        return Err(validation(
            "artifact recovery record is missing its digest field",
        ));
    }
    cognigraph_governance::canonical_digest(&value)
        .map_err(|error| validation(format!("cannot digest artifact recovery record: {error}")))
}

fn canonical_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, CogniGraphError> {
    cognigraph_governance::canonical_json_bytes(value).map_err(|error| {
        validation(format!(
            "cannot serialize canonical artifact recovery JSON: {error}"
        ))
    })
}

fn validate_attestations(
    attestations: &[ArtifactRecoveryAttestation],
) -> Result<(), CogniGraphError> {
    if !(MIN_RECOVERY_ATTESTATIONS..=MAX_RECOVERY_ATTESTATIONS).contains(&attestations.len()) {
        return Err(validation(
            "artifact recovery plan must contain 5..=10 distinct attestation projections",
        ));
    }
    let mut previous: Option<&ArtifactRecoveryAttestation> = None;
    let mut ids = HashSet::new();
    let mut kind_counts = HashMap::<&str, usize>::new();
    for attestation in attestations {
        validate_kind(&attestation.kind)?;
        validate_record_id(
            "artifact recovery attestation id",
            &attestation.attestation_id,
        )?;
        validate_digest(
            "artifact recovery attestation digest",
            &attestation.attestation_digest,
        )?;
        validate_digest(
            "artifact recovery manifest digest",
            &attestation.manifest_digest,
        )?;
        if previous.is_some_and(|previous| previous >= attestation) {
            return Err(validation(
                "artifact recovery attestations must be strictly sorted and distinct",
            ));
        }
        if !ids.insert(attestation.attestation_id.as_str()) {
            return Err(validation(
                "artifact recovery attestation ids must be globally distinct",
            ));
        }
        *kind_counts.entry(&attestation.kind).or_default() += 1;
        previous = Some(attestation);
    }
    for kind in ["corpus", "graph", "oracle", "scorer", "verifier"] {
        if !matches!(kind_counts.get(kind), Some(1 | 2)) {
            return Err(validation(
                "artifact recovery attestations must name each of the five artifact kinds once or twice",
            ));
        }
    }
    Ok(())
}

fn reject_conflicting_attestations(
    attestations: &[ArtifactRecoveryAttestation],
) -> Result<(), CogniGraphError> {
    let mut by_id = HashMap::<&str, &ArtifactRecoveryAttestation>::new();
    for attestation in attestations {
        if let Some(previous) = by_id.insert(&attestation.attestation_id, attestation)
            && previous != attestation
        {
            return Err(validation(
                "artifact recovery attestation id has conflicting projections",
            ));
        }
    }
    Ok(())
}

fn validate_blobs(blobs: &[ArtifactRecoveryBlob]) -> Result<(), CogniGraphError> {
    if blobs.is_empty() || blobs.len() > MAX_RECOVERY_BLOBS {
        return Err(validation(format!(
            "artifact recovery plan must contain 1..={MAX_RECOVERY_BLOBS} unique blobs"
        )));
    }
    let mut previous: Option<&ArtifactRecoveryBlob> = None;
    for blob in blobs {
        validate_digest("artifact recovery blob digest", &blob.blob_digest)?;
        if previous.is_some_and(|previous| previous >= blob) {
            return Err(validation(
                "artifact recovery blobs must be strictly sorted and unique",
            ));
        }
        previous = Some(blob);
    }
    reject_conflicting_blob_lengths(blobs)
}

fn reject_conflicting_blob_lengths(blobs: &[ArtifactRecoveryBlob]) -> Result<(), CogniGraphError> {
    let mut lengths = HashMap::<&str, u64>::new();
    for blob in blobs {
        if let Some(previous) = lengths.insert(&blob.blob_digest, blob.byte_length)
            && previous != blob.byte_length
        {
            return Err(validation(
                "artifact recovery blob digest has conflicting declared lengths",
            ));
        }
    }
    Ok(())
}

fn checked_blob_total(blobs: &[ArtifactRecoveryBlob]) -> Result<u64, CogniGraphError> {
    blobs.iter().try_fold(0_u64, |total, blob| {
        total
            .checked_add(blob.byte_length)
            .ok_or_else(|| capacity("artifact recovery total byte count overflowed"))
    })
}

fn validate_identifier(label: &str, value: &str) -> Result<(), CogniGraphError> {
    if value.is_empty()
        || value.len() > MAX_IDENTIFIER_BYTES
        || !is_nfc(value)
        || value.chars().any(char::is_control)
    {
        return Err(validation(format!(
            "{label} must be non-empty, NFC, control-free, and at most {MAX_IDENTIFIER_BYTES} bytes"
        )));
    }
    Ok(())
}

fn validate_kind(kind: &str) -> Result<(), CogniGraphError> {
    if !matches!(kind, "corpus" | "graph" | "oracle" | "scorer" | "verifier") {
        return Err(validation(
            "artifact recovery attestation kind must be corpus, graph, oracle, scorer, or verifier",
        ));
    }
    Ok(())
}

fn validate_record_id(label: &str, value: &str) -> Result<(), CogniGraphError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(validation(format!(
            "{label} must contain exactly 64 lowercase hexadecimal characters"
        )));
    }
    Ok(())
}

fn validate_digest(label: &str, digest: &str) -> Result<(), CogniGraphError> {
    parse_sha256_digest(digest)
        .map(|_| ())
        .map_err(|_| validation(format!("{label} must use sha256:<64 lowercase hex>")))
}

fn validate_operation_limit(max_total_bytes: u64) -> Result<(), CogniGraphError> {
    if max_total_bytes == 0 {
        return Err(validation(
            "artifact recovery operation byte limit must be positive",
        ));
    }
    Ok(())
}

fn capacity(message: impl Into<String>) -> CogniGraphError {
    CogniGraphError::CapacityExceeded(message.into())
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Barrier};
    use std::thread;

    use serde_json::Value;
    use tempfile::TempDir;

    use super::*;
    use crate::cas::hex_encode;

    const TENANT: &str = "tenant-a";
    const INCARNATION: &str = "incarnation-1";

    fn digest(bytes: impl AsRef<[u8]>) -> String {
        format!("sha256:{}", hex_encode(Sha256::digest(bytes.as_ref())))
    }

    fn cas_root() -> TempDir {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("tenants")).unwrap();
        root
    }

    fn stage_blob(
        root: &Path,
        tenant: &str,
        incarnation: &str,
        bytes: &[u8],
    ) -> ArtifactRecoveryBlob {
        let blob_digest = digest(bytes);
        let hex = parse_sha256_digest(&blob_digest).unwrap();
        let scope = tenant_scope_hex(tenant, incarnation).unwrap();
        let shard = root
            .join("tenants")
            .join(scope)
            .join(SHA256_DIRECTORY)
            .join(&hex[..2]);
        fs::create_dir_all(&shard).unwrap();
        fs::write(shard.join(&hex[2..]), bytes).unwrap();
        ArtifactRecoveryBlob {
            blob_digest,
            byte_length: bytes.len() as u64,
        }
    }

    fn attestations() -> Vec<ArtifactRecoveryAttestation> {
        ["corpus", "graph", "oracle", "scorer", "verifier"]
            .into_iter()
            .map(|kind| ArtifactRecoveryAttestation {
                kind: kind.into(),
                attestation_id: digest(format!("id:{kind}"))
                    .trim_start_matches("sha256:")
                    .into(),
                attestation_digest: digest(format!("attestation:{kind}")),
                manifest_digest: digest(format!("manifest:{kind}")),
            })
            .collect()
    }

    fn plan(blobs: Vec<ArtifactRecoveryBlob>) -> ArtifactRecoveryPlanV1 {
        ArtifactRecoveryPlanV1::new(
            TENANT,
            INCARNATION,
            digest("evidence-id").trim_start_matches("sha256:"),
            digest("evidence"),
            6,
            digest("authority"),
            digest("candidate-set"),
            digest("baseline-set"),
            attestations(),
            blobs,
            5,
        )
        .unwrap()
    }

    #[test]
    fn public_m24_fixture_records_are_digest_valid_and_mutually_bound() {
        let plan: ArtifactRecoveryPlanV1 =
            serde_json::from_str(include_str!("../../../fixtures/m24/recovery-plan.json")).unwrap();
        let backup: ArtifactBackupReceiptV1 =
            serde_json::from_str(include_str!("../../../fixtures/m24/backup-receipt.json"))
                .unwrap();
        let restore: ArtifactRestoreReceiptV1 =
            serde_json::from_str(include_str!("../../../fixtures/m24/restore-receipt.json"))
                .unwrap();

        plan.validate().unwrap();
        backup.validate_against(&plan).unwrap();
        restore.validate_against(&plan, &backup).unwrap();
        assert!(
            plan.attestations
                .iter()
                .all(|item| item.attestation_id.len() == 64)
        );
        assert_eq!(
            plan.attestations
                .iter()
                .filter(|item| item.kind == "graph")
                .count(),
            2
        );
        assert_ne!(
            plan.candidate_artifact_set_digest,
            plan.baseline_artifact_set_digest
        );
        ArtifactRecoveryPlanV1::from_canonical_bytes(&plan.canonical_bytes().unwrap()).unwrap();
    }

    fn blob_path(root: &Path, blob: &ArtifactRecoveryBlob) -> PathBuf {
        let hex = parse_sha256_digest(&blob.blob_digest).unwrap();
        root.join(BLOBS_DIRECTORY)
            .join(SHA256_DIRECTORY)
            .join(&hex[..2])
            .join(&hex[2..])
    }

    #[test]
    fn plan_is_deterministic_sorted_deduplicated_and_strict() {
        let first = ArtifactRecoveryBlob {
            blob_digest: digest("first"),
            byte_length: 5,
        };
        let second = ArtifactRecoveryBlob {
            blob_digest: digest("second"),
            byte_length: 6,
        };
        let mut reversed_attestations = attestations();
        reversed_attestations.reverse();
        let one = ArtifactRecoveryPlanV1::new(
            TENANT,
            INCARNATION,
            digest("evidence-id").trim_start_matches("sha256:"),
            digest("evidence"),
            6,
            digest("authority"),
            digest("candidate-set"),
            digest("baseline-set"),
            reversed_attestations,
            vec![second.clone(), first.clone(), first.clone()],
            5,
        )
        .unwrap();
        let two = plan(vec![first, second]);
        assert_eq!(one, two);
        assert_eq!(one.blob_count, 2);
        assert_eq!(one.total_bytes, 11);
        assert_eq!(
            ArtifactRecoveryPlanV1::from_canonical_bytes(&one.canonical_bytes().unwrap()).unwrap(),
            one
        );

        let mut unknown: Value = serde_json::to_value(&one).unwrap();
        unknown["unexpected"] = Value::Bool(true);
        assert!(serde_json::from_value::<ArtifactRecoveryPlanV1>(unknown).is_err());
        let mut explicit_null: Value = serde_json::to_value(&one).unwrap();
        explicit_null["plan_digest"] = Value::Null;
        assert!(serde_json::from_value::<ArtifactRecoveryPlanV1>(explicit_null).is_err());

        let pretty = serde_json::to_vec_pretty(&one).unwrap();
        assert!(ArtifactRecoveryPlanV1::from_canonical_bytes(&pretty).is_err());
    }

    #[test]
    fn plan_rejects_conflicts_scope_tamper_and_non_nfc_identity() {
        let blob_digest = digest("same-address");
        let conflicting = ArtifactRecoveryPlanV1::new(
            TENANT,
            INCARNATION,
            digest("evidence-id").trim_start_matches("sha256:"),
            digest("evidence"),
            6,
            digest("authority"),
            digest("candidate-set"),
            digest("baseline-set"),
            attestations(),
            vec![
                ArtifactRecoveryBlob {
                    blob_digest: blob_digest.clone(),
                    byte_length: 1,
                },
                ArtifactRecoveryBlob {
                    blob_digest,
                    byte_length: 2,
                },
            ],
            5,
        );
        assert!(conflicting.is_err());

        let blob = ArtifactRecoveryBlob {
            blob_digest: digest("blob"),
            byte_length: 4,
        };
        let mut plan = plan(vec![blob.clone()]);
        plan.tenant_scope_digest = digest("foreign-scope");
        plan.plan_digest = recovery_plan_digest(&plan).unwrap();
        assert!(plan.validate().is_err());

        assert!(
            ArtifactRecoveryPlanV1::new(
                "e\u{301}",
                INCARNATION,
                digest("evidence-id").trim_start_matches("sha256:"),
                digest("evidence"),
                6,
                digest("authority"),
                digest("candidate-set"),
                digest("baseline-set"),
                attestations(),
                vec![blob],
                5,
            )
            .is_err()
        );
    }

    #[tokio::test]
    async fn create_verify_restore_and_final_cas_reread_succeed() {
        let source_root = cas_root();
        let first = stage_blob(source_root.path(), TENANT, INCARNATION, b"first blob");
        let second = stage_blob(source_root.path(), TENANT, INCARNATION, b"second blob");
        let plan = plan(vec![second.clone(), first.clone()]);
        let source = LocalArtifactCas::open(source_root.path(), 1024).unwrap();
        let bundle_parent = tempfile::tempdir().unwrap();
        let bundle = bundle_parent.path().join("bundle");

        let backup = create_bundle(&source, &plan, &bundle, 1024).unwrap();
        backup.validate_against(&plan).unwrap();
        assert_eq!(
            verify_bundle(&bundle, &plan.plan_digest, TENANT, INCARNATION, 1024,).unwrap(),
            backup
        );

        let target_root = cas_root();
        let restore = restore_bundle(
            &bundle,
            target_root.path(),
            &plan.plan_digest,
            TENANT,
            INCARNATION,
            1024,
        )
        .unwrap();
        restore.validate_against(&plan, &backup).unwrap();

        let restored = LocalArtifactCas::open(target_root.path(), 1024).unwrap();
        let mut budget = restored.begin_evaluation(TENANT, INCARNATION).unwrap();
        for blob in [&first, &second] {
            restored
                .verify_blob(&mut budget, &blob.blob_digest, blob.byte_length, None)
                .await
                .unwrap();
        }
    }

    #[test]
    fn verify_rejects_wrong_expectation_tampered_blob_and_extra_entry() {
        let source_root = cas_root();
        let blob = stage_blob(source_root.path(), TENANT, INCARNATION, b"protected bytes");
        let plan = plan(vec![blob.clone()]);
        let source = LocalArtifactCas::open(source_root.path(), 1024).unwrap();

        let first_parent = tempfile::tempdir().unwrap();
        let first_bundle = first_parent.path().join("bundle");
        create_bundle(&source, &plan, &first_bundle, 1024).unwrap();
        assert!(
            verify_bundle(
                &first_bundle,
                &plan.plan_digest,
                "tenant-b",
                INCARNATION,
                1024,
            )
            .is_err()
        );
        fs::write(blob_path(&first_bundle, &blob), b"tampered bytes!").unwrap();
        assert!(
            verify_bundle(&first_bundle, &plan.plan_digest, TENANT, INCARNATION, 1024,).is_err()
        );

        let second_parent = tempfile::tempdir().unwrap();
        let second_bundle = second_parent.path().join("bundle");
        create_bundle(&source, &plan, &second_bundle, 1024).unwrap();
        fs::write(second_bundle.join("unexpected"), b"extra").unwrap();
        assert!(
            verify_bundle(&second_bundle, &plan.plan_digest, TENANT, INCARNATION, 1024,).is_err()
        );
    }

    #[test]
    fn canonical_plan_and_receipt_tamper_fail_closed() {
        let source_root = cas_root();
        let blob = stage_blob(source_root.path(), TENANT, INCARNATION, b"canonical");
        let plan = plan(vec![blob]);
        let source = LocalArtifactCas::open(source_root.path(), 1024).unwrap();

        let plan_parent = tempfile::tempdir().unwrap();
        let plan_bundle = plan_parent.path().join("bundle");
        create_bundle(&source, &plan, &plan_bundle, 1024).unwrap();
        fs::write(
            plan_bundle.join(CUSTODY_FILE),
            serde_json::to_vec_pretty(&plan).unwrap(),
        )
        .unwrap();
        assert!(
            verify_bundle(&plan_bundle, &plan.plan_digest, TENANT, INCARNATION, 1024,).is_err()
        );

        let receipt_parent = tempfile::tempdir().unwrap();
        let receipt_bundle = receipt_parent.path().join("bundle");
        create_bundle(&source, &plan, &receipt_bundle, 1024).unwrap();
        let receipt_path = receipt_bundle.join(BACKUP_RECEIPT_FILE);
        let mut receipt: ArtifactBackupReceiptV1 =
            serde_json::from_slice(&fs::read(&receipt_path).unwrap()).unwrap();
        receipt.total_bytes += 1;
        receipt.receipt_digest = backup_receipt_digest(&receipt).unwrap();
        fs::write(&receipt_path, receipt.canonical_bytes().unwrap()).unwrap();
        assert!(
            verify_bundle(
                &receipt_bundle,
                &plan.plan_digest,
                TENANT,
                INCARNATION,
                1024,
            )
            .is_err()
        );
    }

    #[cfg(unix)]
    #[test]
    fn bundle_symlinks_are_rejected() {
        use std::os::unix::fs::symlink;

        let source_root = cas_root();
        let blob = stage_blob(
            source_root.path(),
            TENANT,
            INCARNATION,
            b"symlink protected",
        );
        let plan = plan(vec![blob.clone()]);
        let source = LocalArtifactCas::open(source_root.path(), 1024).unwrap();
        let bundle_parent = tempfile::tempdir().unwrap();
        let bundle = bundle_parent.path().join("bundle");
        create_bundle(&source, &plan, &bundle, 1024).unwrap();

        let original = blob_path(&bundle, &blob);
        let external = bundle_parent.path().join("external");
        fs::write(&external, b"symlink protected").unwrap();
        fs::remove_file(&original).unwrap();
        symlink(&external, &original).unwrap();
        assert!(verify_bundle(&bundle, &plan.plan_digest, TENANT, INCARNATION, 1024,).is_err());
    }

    #[test]
    fn failures_never_overwrite_bundle_or_existing_restore_scope() {
        let source_root = cas_root();
        let blob = stage_blob(source_root.path(), TENANT, INCARNATION, b"no overwrite");
        let plan = plan(vec![blob]);
        let source = LocalArtifactCas::open(source_root.path(), 1024).unwrap();
        let bundle_parent = tempfile::tempdir().unwrap();
        let bundle = bundle_parent.path().join("bundle");
        fs::create_dir(&bundle).unwrap();
        fs::write(bundle.join("sentinel"), b"keep").unwrap();
        assert!(create_bundle(&source, &plan, &bundle, 1024).is_err());
        assert_eq!(fs::read(bundle.join("sentinel")).unwrap(), b"keep");

        fs::remove_dir_all(&bundle).unwrap();
        create_bundle(&source, &plan, &bundle, 1024).unwrap();
        let target_root = cas_root();
        let scope = target_root
            .path()
            .join("tenants")
            .join(tenant_scope_hex(TENANT, INCARNATION).unwrap());
        fs::create_dir(&scope).unwrap();
        fs::write(scope.join("sentinel"), b"keep").unwrap();
        assert!(
            restore_bundle(
                &bundle,
                target_root.path(),
                &plan.plan_digest,
                TENANT,
                INCARNATION,
                1024,
            )
            .is_err()
        );
        assert_eq!(fs::read(scope.join("sentinel")).unwrap(), b"keep");
    }

    #[test]
    fn tampered_restore_leaves_final_scope_absent() {
        let source_root = cas_root();
        let blob = stage_blob(source_root.path(), TENANT, INCARNATION, b"restore guard");
        let plan = plan(vec![blob.clone()]);
        let source = LocalArtifactCas::open(source_root.path(), 1024).unwrap();
        let bundle_parent = tempfile::tempdir().unwrap();
        let bundle = bundle_parent.path().join("bundle");
        create_bundle(&source, &plan, &bundle, 1024).unwrap();
        fs::write(blob_path(&bundle, &blob), b"bad restore!!").unwrap();

        let target_root = cas_root();
        assert!(
            restore_bundle(
                &bundle,
                target_root.path(),
                &plan.plan_digest,
                TENANT,
                INCARNATION,
                1024,
            )
            .is_err()
        );
        assert!(
            !target_root
                .path()
                .join("tenants")
                .join(tenant_scope_hex(TENANT, INCARNATION).unwrap())
                .exists()
        );
    }

    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios",
        windows
    ))]
    #[test]
    fn concurrent_bundle_publishers_have_exactly_one_winner() {
        let source_root = cas_root();
        let blob = stage_blob(source_root.path(), TENANT, INCARNATION, b"one publisher");
        let plan = plan(vec![blob]);
        let source = LocalArtifactCas::open(source_root.path(), 1024).unwrap();
        let bundle_parent = tempfile::tempdir().unwrap();
        let bundle = bundle_parent.path().join("bundle");
        let barrier = Arc::new(Barrier::new(3));

        let handles = (0..2)
            .map(|_| {
                let source = source.clone();
                let plan = plan.clone();
                let bundle = bundle.clone();
                let barrier = barrier.clone();
                thread::spawn(move || {
                    barrier.wait();
                    create_bundle(&source, &plan, &bundle, 1024)
                })
            })
            .collect::<Vec<_>>();
        barrier.wait();
        let results = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        verify_bundle(&bundle, &plan.plan_digest, TENANT, INCARNATION, 1024).unwrap();
    }
}
