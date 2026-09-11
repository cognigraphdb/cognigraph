//! Deterministic M24 recovery-plan projection.
//!
//! A plan is derived exclusively from immutable M21-M23 promotion evidence
//! and historically valid signed M20 attestation records. This module never
//! opens the local CAS and deliberately ignores signed location observations:
//! the resulting inventory is not proof that any blob is currently available
//! or held by a backup custodian.

use std::collections::BTreeMap;

use cognigraph_artifacts::{
    ArtifactRecoveryAttestation, ArtifactRecoveryBlob, ArtifactRecoveryPlanV1,
};
use cognigraph_core::CogniGraphError;

use crate::artifact_attestations::{ArtifactAttestationRecord, ArtifactKind};
use crate::promotions::{
    EvidenceRunRole, M21_PROMOTION_EVIDENCE_SCHEMA_VERSION, M22_PROMOTION_EVIDENCE_SCHEMA_VERSION,
    M23_PROMOTION_EVIDENCE_SCHEMA_VERSION, PromotionDecision, PromotionEvidence, PromotionManager,
};

pub(crate) const MAX_ARTIFACT_RECOVERY_BLOBS: usize = 8_192;
pub(crate) const MAX_ARTIFACT_RECOVERY_MANIFEST_ENTRIES: u64 = 20_000;

/// Derive one closed, deterministic byte inventory without touching artifact
/// storage. Later key expiry or revocation does not invalidate authority that
/// was valid when the immutable evidence was created.
pub(crate) async fn derive_recovery_plan(
    promotions: &PromotionManager,
    tenant: &str,
    incarnation: &str,
    evidence_id: &str,
    max_total_bytes: u64,
) -> Result<ArtifactRecoveryPlanV1, CogniGraphError> {
    if max_total_bytes == 0 {
        return Err(validation(
            "artifact recovery plan byte limit must be positive",
        ));
    }
    let _guard = promotions.transition_lock.lock().await;
    promotions.ensure_tenant_active(tenant)?;

    let evidence = promotions
        .get_evidence(tenant, incarnation, evidence_id)
        .await?;
    if !matches!(
        evidence.schema_version,
        M21_PROMOTION_EVIDENCE_SCHEMA_VERSION
            | M22_PROMOTION_EVIDENCE_SCHEMA_VERSION
            | M23_PROMOTION_EVIDENCE_SCHEMA_VERSION
    ) {
        return Err(validation(
            "artifact recovery plans require M21-M23 promotion evidence",
        ));
    }

    // Revalidate the signed policy and artifact authority at the immutable
    // evidence timestamp. This intentionally differs from an active-key
    // lookup: later revocation must not erase historical validity.
    promotions
        .validate_historical_promotion_authority_without_semantic_repairs_locked(
            tenant,
            incarnation,
            &BTreeMap::from([(evidence.id.clone(), evidence.clone())]),
            &BTreeMap::<String, PromotionDecision>::new(),
        )
        .await?;

    let authority = evidence
        .artifact_attestations
        .as_ref()
        .ok_or_else(|| conflict("M21-M23 promotion evidence is missing artifact authority"))?;
    authority.candidate.validate()?;
    authority.baseline.validate()?;

    let candidate_context = run_context(&evidence, EvidenceRunRole::CandidateOriginal)?;
    let baseline_context = run_context(&evidence, EvidenceRunRole::BaselineOriginal)?;
    let candidate_set = candidate_context
        .artifact_attestations
        .as_ref()
        .ok_or_else(|| conflict("candidate evidence run is missing artifact bindings"))?;
    let baseline_set = baseline_context
        .artifact_attestations
        .as_ref()
        .ok_or_else(|| conflict("baseline evidence run is missing artifact bindings"))?;
    if candidate_set != &authority.candidate || baseline_set != &authority.baseline {
        return Err(conflict(
            "promotion evidence artifact authority does not exactly match its candidate and baseline runs",
        ));
    }
    crate::artifact_attestations::validate_context_subject_bindings(
        candidate_context,
        candidate_set,
    )?;
    crate::artifact_attestations::validate_context_subject_bindings(
        baseline_context,
        baseline_set,
    )?;

    // A candidate and baseline often share corpus/oracle/scorer/verifier
    // attestations. Validate every slot before deduplicating by immutable id.
    let mut records = BTreeMap::<String, ArtifactAttestationRecord>::new();
    for binding in authority
        .candidate
        .bindings()
        .into_iter()
        .chain(authority.baseline.bindings())
    {
        let record = if let Some(record) = records.get(&binding.attestation_id) {
            record.clone()
        } else {
            promotions
                .get_artifact_attestation(tenant, incarnation, &binding.attestation_id)
                .await?
        };
        if record.binding() != *binding || record.accepted_at_ms > evidence.created_at_ms {
            return Err(conflict(
                "promotion evidence artifact binding does not match historical signed authority",
            ));
        }
        if let Some(existing) = records.insert(record.attestation_id.clone(), record.clone())
            && existing != record
        {
            return Err(conflict(
                "one artifact attestation id resolves to divergent records",
            ));
        }
    }

    let mut attestations = Vec::with_capacity(records.len());
    let mut blobs = BTreeMap::<String, u64>::new();
    let mut manifest_entry_count = 0_u64;
    let mut total_bytes = 0_u64;
    for record in records.values() {
        reserve_manifest_entries(&mut manifest_entry_count, record.manifest.entry_count)?;
        attestations.push(ArtifactRecoveryAttestation {
            kind: artifact_kind_name(record.artifact_kind).into(),
            attestation_id: record.attestation_id.clone(),
            attestation_digest: record.attestation_digest.clone(),
            manifest_digest: record.manifest_digest.clone(),
        });
        for entry in &record.manifest.entries {
            insert_blob(
                &mut blobs,
                &entry.blob_digest,
                entry.byte_length,
                &mut total_bytes,
                max_total_bytes,
            )?;
        }
    }
    let blobs = blobs
        .into_iter()
        .map(|(blob_digest, byte_length)| ArtifactRecoveryBlob {
            blob_digest,
            byte_length,
        })
        .collect::<Vec<_>>();

    let evidence_id = evidence.id.clone();
    let evidence_digest = evidence.evidence_digest.clone();
    let evidence_schema_version = evidence.schema_version;
    let artifact_authority_digest = authority.authority_digest.clone();
    let candidate_artifact_set_digest = authority.candidate.set_digest.clone();
    let baseline_artifact_set_digest = authority.baseline.set_digest.clone();
    let plan = ArtifactRecoveryPlanV1::new(
        tenant,
        incarnation,
        evidence_id,
        evidence_digest,
        evidence_schema_version,
        artifact_authority_digest,
        candidate_artifact_set_digest,
        baseline_artifact_set_digest,
        attestations,
        blobs,
        manifest_entry_count,
    )?;
    if plan.total_bytes != total_bytes {
        return Err(conflict(
            "artifact recovery plan total does not match server byte accounting",
        ));
    }
    Ok(plan)
}

fn reserve_manifest_entries(total: &mut u64, entries: u64) -> Result<(), CogniGraphError> {
    let next = total
        .checked_add(entries)
        .ok_or_else(|| capacity("artifact recovery manifest entry count overflowed"))?;
    if next > MAX_ARTIFACT_RECOVERY_MANIFEST_ENTRIES {
        return Err(capacity(format!(
            "artifact recovery plan exceeds {MAX_ARTIFACT_RECOVERY_MANIFEST_ENTRIES} manifest entries"
        )));
    }
    *total = next;
    Ok(())
}

fn run_context(
    evidence: &PromotionEvidence,
    role: EvidenceRunRole,
) -> Result<&crate::promotions::PromotionContext, CogniGraphError> {
    evidence
        .runs
        .iter()
        .find(|run| run.role == role)
        .map(|run| &run.source.context)
        .ok_or_else(|| conflict("promotion evidence is missing a required evaluation run"))
}

fn insert_blob(
    blobs: &mut BTreeMap<String, u64>,
    digest: &str,
    byte_length: u64,
    total_bytes: &mut u64,
    max_total_bytes: u64,
) -> Result<(), CogniGraphError> {
    if let Some(previous_length) = blobs.get(digest) {
        if *previous_length != byte_length {
            return Err(conflict(format!(
                "artifact blob digest `{digest}` has conflicting declared lengths"
            )));
        }
        return Ok(());
    }
    if blobs.len() >= MAX_ARTIFACT_RECOVERY_BLOBS {
        return Err(capacity(format!(
            "artifact recovery plan exceeds {MAX_ARTIFACT_RECOVERY_BLOBS} distinct blobs"
        )));
    }
    let next_total = total_bytes
        .checked_add(byte_length)
        .ok_or_else(|| capacity("artifact recovery byte accounting overflowed"))?;
    if next_total > max_total_bytes {
        return Err(capacity(format!(
            "artifact recovery plan requires {next_total} distinct blob bytes, exceeding its {max_total_bytes} byte limit"
        )));
    }
    blobs.insert(digest.into(), byte_length);
    *total_bytes = next_total;
    Ok(())
}

fn artifact_kind_name(kind: ArtifactKind) -> &'static str {
    match kind {
        ArtifactKind::Corpus => "corpus",
        ArtifactKind::Graph => "graph",
        ArtifactKind::Oracle => "oracle",
        ArtifactKind::Scorer => "scorer",
        ArtifactKind::Verifier => "verifier",
    }
}

fn validation(message: impl Into<String>) -> CogniGraphError {
    CogniGraphError::ValidationError(message.into())
}

fn conflict(message: impl Into<String>) -> CogniGraphError {
    CogniGraphError::DocumentConflict(message.into())
}

fn capacity(message: impl Into<String>) -> CogniGraphError {
    CogniGraphError::CapacityExceeded(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(byte: u8) -> String {
        format!("sha256:{}", char::from(byte).to_string().repeat(64))
    }

    #[test]
    fn blob_inventory_deduplicates_exact_pairs_and_rejects_length_conflicts() {
        let mut blobs = BTreeMap::new();
        let mut total = 0;
        insert_blob(&mut blobs, &digest(b'a'), 7, &mut total, 10).unwrap();
        insert_blob(&mut blobs, &digest(b'a'), 7, &mut total, 10).unwrap();
        assert_eq!(blobs.len(), 1);
        assert_eq!(total, 7);

        let error = insert_blob(&mut blobs, &digest(b'a'), 8, &mut total, 10).unwrap_err();
        assert!(matches!(error, CogniGraphError::DocumentConflict(_)));
        assert_eq!(blobs.get(&digest(b'a')), Some(&7));
        assert_eq!(total, 7);
    }

    #[test]
    fn blob_inventory_enforces_checked_configured_byte_accounting() {
        let mut blobs = BTreeMap::new();
        let mut total = 0;
        insert_blob(&mut blobs, &digest(b'a'), 7, &mut total, 10).unwrap();
        let error = insert_blob(&mut blobs, &digest(b'b'), 4, &mut total, 10).unwrap_err();
        assert!(matches!(error, CogniGraphError::CapacityExceeded(_)));
        assert_eq!(blobs.len(), 1);
        assert_eq!(total, 7);

        let mut overflowed = BTreeMap::new();
        let mut total = u64::MAX;
        let error =
            insert_blob(&mut overflowed, &digest(b'c'), 1, &mut total, u64::MAX).unwrap_err();
        assert!(matches!(error, CogniGraphError::CapacityExceeded(_)));
    }

    #[test]
    fn blob_inventory_caps_distinct_entries_without_mutating_on_failure() {
        let mut blobs = (0..MAX_ARTIFACT_RECOVERY_BLOBS)
            .map(|index| (format!("sha256:{index:064x}"), 0))
            .collect::<BTreeMap<_, _>>();
        let mut total = 0;
        let error = insert_blob(
            &mut blobs,
            &format!("sha256:{:064x}", MAX_ARTIFACT_RECOVERY_BLOBS),
            0,
            &mut total,
            1,
        )
        .unwrap_err();
        assert!(matches!(error, CogniGraphError::CapacityExceeded(_)));
        assert_eq!(blobs.len(), MAX_ARTIFACT_RECOVERY_BLOBS);
        assert_eq!(total, 0);
    }

    #[test]
    fn manifest_entry_accounting_is_checked_and_capped() {
        let mut total = MAX_ARTIFACT_RECOVERY_MANIFEST_ENTRIES - 1;
        reserve_manifest_entries(&mut total, 1).unwrap();
        assert_eq!(total, MAX_ARTIFACT_RECOVERY_MANIFEST_ENTRIES);

        let error = reserve_manifest_entries(&mut total, 1).unwrap_err();
        assert!(matches!(error, CogniGraphError::CapacityExceeded(_)));
        assert_eq!(total, MAX_ARTIFACT_RECOVERY_MANIFEST_ENTRIES);

        let mut overflowed = u64::MAX;
        let error = reserve_manifest_entries(&mut overflowed, 1).unwrap_err();
        assert!(matches!(error, CogniGraphError::CapacityExceeded(_)));
        assert_eq!(overflowed, u64::MAX);
    }
}
