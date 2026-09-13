//! Manifest receipt.

use super::*;

pub(super) fn validate_receipt_manifest<'a>(
    manifest: &'a ArtifactManifest,
    receipt: &ConsumedArtifactReceipt,
    artifact_kind: ArtifactKind,
    artifact_format: &str,
    expected_paths: &[&str],
) -> Result<Vec<&'a crate::artifact_attestations::ArtifactManifestEntry>, CogniGraphError> {
    if manifest.schema_version != ARTIFACT_MANIFEST_SCHEMA_VERSION
        || manifest.artifact_kind != artifact_kind
        || manifest.artifact_format != artifact_format
        || manifest.entry_count != manifest.entries.len() as u64
        || manifest.entry_count != receipt.entry_count
        || manifest.total_bytes != receipt.total_bytes
        || canonical_digest(manifest)? != receipt.manifest_digest
        || canonical_digest(&manifest.entries)? != receipt.verified_blob_set_digest
        || manifest.entries.len() != expected_paths.len()
    {
        return Err(validation(
            "derivation receipt manifest projection does not match its signed artifact binding",
        ));
    }
    let total_bytes = manifest
        .entries
        .iter()
        .try_fold(0_u64, |total, entry| total.checked_add(entry.byte_length))
        .ok_or_else(|| validation("derivation receipt manifest byte accounting overflowed"))?;
    if total_bytes == 0 || total_bytes != manifest.total_bytes {
        return Err(validation(
            "derivation receipt manifest projection has invalid byte accounting",
        ));
    }
    let mut entries = Vec::with_capacity(expected_paths.len());
    for (entry, expected_path) in manifest.entries.iter().zip(expected_paths) {
        validate_digest(
            "derivation receipt manifest entry blob_digest",
            &entry.blob_digest,
        )?;
        if entry.logical_path != *expected_path
            || entry.media_type != "application/json"
            || entry.executable
            || entry.byte_length == 0
        {
            return Err(validation(
                "derivation receipt manifest projection has an invalid entry contract",
            ));
        }
        entries.push(entry);
    }
    Ok(entries)
}
