//! Manifest.

use super::*;

pub(super) async fn consume_manifest(
    cas: &LocalArtifactCas,
    budget: &mut ArtifactEvaluationBudget,
    verified_blobs: &mut HashMap<(String, u64), Option<Vec<u8>>>,
    record: &ArtifactAttestationRecord,
    slot: &ArtifactConsumptionSlotPlan,
    purpose: ArtifactConsumptionPurpose,
    retain_caps: &[(&str, u64)],
) -> Result<ConsumedManifest, CogniGraphError> {
    if record.artifact_kind != slot.artifact_kind
        || record.manifest.artifact_kind != slot.artifact_kind
        || record.manifest.artifact_format != slot.artifact_format
    {
        return Err(validation(
            "active artifact manifest does not match the pinned M21 slot contract",
        ));
    }

    let mut retained_bytes = HashMap::new();
    for (index, entry) in record.manifest.entries.iter().enumerate() {
        let retain = retain_caps
            .iter()
            .find_map(|(logical_path, cap)| (entry.logical_path == *logical_path).then_some(*cap));
        let cache_key = (entry.blob_digest.clone(), entry.byte_length);
        let cached = verified_blobs.get(&cache_key);
        let needs_retained_bytes = retain.is_some() && cached.is_some_and(Option::is_none);
        let verified_bytes = if cached.is_none() || needs_retained_bytes {
            if cached.is_none() && verified_blobs.len() >= MAX_EVALUATION_UNIQUE_BLOBS {
                return Err(CogniGraphError::CapacityExceeded(format!(
                    "M21 evaluation exceeds the {MAX_EVALUATION_UNIQUE_BLOBS} unique blob limit"
                )));
            }
            let verified = cas
                .verify_blob(budget, &entry.blob_digest, entry.byte_length, retain)
                .await?;
            let bytes = verified.retained_bytes;
            verified_blobs.insert(cache_key, bytes.clone());
            bytes
        } else {
            cached.cloned().flatten()
        };
        if let Some(bytes) = verified_bytes
            && retained_bytes
                .insert(entry.logical_path.clone(), bytes)
                .is_some()
        {
            return Err(validation(
                "artifact manifest repeats a retained entrypoint",
            ));
        }
        if index % 128 == 127 {
            tokio::task::yield_now().await;
        }
    }

    Ok(ConsumedManifest {
        receipt: ConsumedArtifactReceipt {
            artifact_kind: record.artifact_kind,
            purpose,
            attestation_id: record.attestation_id.clone(),
            attestation_digest: record.attestation_digest.clone(),
            manifest_digest: record.manifest_digest.clone(),
            entry_count: record.manifest.entry_count,
            total_bytes: record.manifest.total_bytes,
            verified_blob_set_digest: canonical_digest(&record.manifest.entries)?,
            semantic_digest: record.manifest_digest.clone(),
        },
        retained_bytes,
    })
}
pub(super) fn validate_manifest_contracts(
    records: &ActiveArtifactAttestations,
    plan: &ArtifactConsumptionPlan,
) -> Result<(), CogniGraphError> {
    validate_manifest_slot(&records.corpus, &plan.corpus)?;
    validate_manifest_slot(&records.graph, &plan.graph)?;
    validate_manifest_slot(&records.oracle, &plan.oracle)?;
    validate_manifest_slot(&records.scorer, &plan.scorer)?;
    validate_manifest_slot(&records.verifier, &plan.verifier)
}
pub(super) fn validate_manifest_slot(
    record: &ArtifactAttestationRecord,
    slot: &ArtifactConsumptionSlotPlan,
) -> Result<(), CogniGraphError> {
    if record.artifact_kind != slot.artifact_kind
        || record.manifest.artifact_kind != slot.artifact_kind
        || record.manifest.artifact_format != slot.artifact_format
    {
        return Err(validation(
            "artifact manifest kind or format does not match its M21 consumption slot",
        ));
    }
    match slot.mode {
        ArtifactConsumptionMode::CompleteManifest => {
            if slot.entrypoint.is_some() || !slot.auxiliary_entrypoints.is_empty() {
                return Err(validation(
                    "complete-manifest consumption cannot name an entrypoint",
                ));
            }
        }
        ArtifactConsumptionMode::PreparedChunkCorpusJson
        | ArtifactConsumptionMode::EvaluationGraphJson
        | ArtifactConsumptionMode::PromotionOracleJson => {
            let entry = require_single_entrypoint(record, slot)?;
            if entry.executable || entry.media_type != "application/json" {
                return Err(validation(
                    "corpus, graph, and oracle entrypoints must be non-executable application/json",
                ));
            }
        }
        ArtifactConsumptionMode::ReproduciblePreparedChunkCorpusPackage => {
            let primary = slot
                .entrypoint
                .as_deref()
                .ok_or_else(|| validation("reproducible corpus package is missing corpus.json"))?;
            let mut expected = slot
                .auxiliary_entrypoints
                .iter()
                .map(String::as_str)
                .chain(std::iter::once(primary))
                .collect::<Vec<_>>();
            expected.sort_unstable();
            let actual = record
                .manifest
                .entries
                .iter()
                .map(|entry| entry.logical_path.as_str())
                .collect::<Vec<_>>();
            if actual != expected
                || record
                    .manifest
                    .entries
                    .iter()
                    .any(|entry| entry.executable || entry.media_type != "application/json")
            {
                return Err(validation(
                    "M23 corpus package requires exactly canonical non-executable application/json corpus.json and documents.json entries",
                ));
            }
        }
        ArtifactConsumptionMode::ReproducibleEvaluationGraphPackage => {
            let primary = slot
                .entrypoint
                .as_deref()
                .ok_or_else(|| validation("reproducible graph package is missing graph.json"))?;
            let mut expected = slot
                .auxiliary_entrypoints
                .iter()
                .map(String::as_str)
                .chain(std::iter::once(primary))
                .collect::<Vec<_>>();
            expected.sort_unstable();
            let actual = record
                .manifest
                .entries
                .iter()
                .map(|entry| entry.logical_path.as_str())
                .collect::<Vec<_>>();
            if actual != expected
                || record
                    .manifest
                    .entries
                    .iter()
                    .any(|entry| entry.executable || entry.media_type != "application/json")
            {
                return Err(validation(
                    "M22 graph package requires exactly canonical non-executable application/json candidate.json and graph.json entries",
                ));
            }
        }
        ArtifactConsumptionMode::CurrentExecutable => {
            let entry = require_single_entrypoint(record, slot)?;
            if !entry.executable || entry.media_type != "application/octet-stream" {
                return Err(validation(
                    "scorer and verifier entrypoints must be executable application/octet-stream",
                ));
            }
        }
    }
    Ok(())
}
pub(super) fn require_single_entrypoint<'a>(
    record: &'a ArtifactAttestationRecord,
    slot: &ArtifactConsumptionSlotPlan,
) -> Result<&'a crate::artifact_attestations::ArtifactManifestEntry, CogniGraphError> {
    if !slot.auxiliary_entrypoints.is_empty() {
        return Err(validation(
            "single-entry artifact consumption cannot name auxiliary entrypoints",
        ));
    }
    let entrypoint = slot
        .entrypoint
        .as_deref()
        .ok_or_else(|| validation("artifact consumption slot is missing its entrypoint"))?;
    if record.manifest.entries.len() != 1 || record.manifest.entries[0].logical_path != entrypoint {
        return Err(validation(
            "M21 graph, oracle, scorer, and verifier manifests require exactly their pinned entrypoint",
        ));
    }
    Ok(&record.manifest.entries[0])
}
pub(super) fn require_manifest_entry<'a>(
    record: &'a ArtifactAttestationRecord,
    logical_path: &str,
) -> Result<&'a crate::artifact_attestations::ArtifactManifestEntry, CogniGraphError> {
    record
        .manifest
        .entries
        .iter()
        .find(|entry| entry.logical_path == logical_path)
        .ok_or_else(|| {
            validation(format!(
                "verified artifact manifest is missing `{logical_path}`"
            ))
        })
}
pub(super) fn ensure_same_active_bindings(
    before: &ActiveArtifactAttestations,
    after: &ActiveArtifactAttestations,
) -> Result<(), CogniGraphError> {
    let before = [
        before.corpus.binding(),
        before.graph.binding(),
        before.oracle.binding(),
        before.scorer.binding(),
        before.verifier.binding(),
    ];
    let after = [
        after.corpus.binding(),
        after.graph.binding(),
        after.oracle.binding(),
        after.scorer.binding(),
        after.verifier.binding(),
    ];
    if before != after {
        return Err(validation(
            "active artifact authority changed during verified consumption",
        ));
    }
    Ok(())
}
