//! Content-addressed artifact verification and offline recovery primitives.
//!
//! This crate deliberately does not interpret artifact manifest logical paths
//! or signed location hints. Filesystem paths are derived only from validated
//! SHA-256 digests and tenant-incarnation scope digests.

mod cas;
mod recovery;

pub use cas::{
    ArtifactEvaluationBudget, LocalArtifactCas, VerifiedArtifactBlob, tenant_scope_digest,
    tenant_scope_hex,
};
pub use recovery::{
    ARTIFACT_BACKUP_RECEIPT_DOMAIN, ARTIFACT_RECOVERY_PLAN_DOMAIN, ARTIFACT_RESTORE_RECEIPT_DOMAIN,
    ArtifactBackupReceiptV1, ArtifactRecoveryAttestation, ArtifactRecoveryBlob,
    ArtifactRecoveryPlanV1, ArtifactRestoreReceiptV1, DEFAULT_MAX_CUSTODY_BYTES, create_bundle,
    restore_bundle, verify_bundle,
};
