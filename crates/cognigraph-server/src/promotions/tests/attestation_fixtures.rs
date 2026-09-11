//! Attestation fixtures.

use super::*;

pub(super) fn attested_context(
    candidate: &str,
    binding: &PolicyGovernanceBinding,
) -> PromotionContext {
    let mut context = governed_context(candidate, binding);
    context.schema_version = M20_PROMOTION_CONTEXT_SCHEMA_VERSION;
    let corpus_manifest_digest = digest("m20-corpus-manifest");
    let subjects = [
        (
            ArtifactKind::Corpus,
            ArtifactSubject::Corpus {
                revision: context.revisions.corpus.clone(),
                preprocessing_digest: context.effective_configuration.preprocessing_digest.clone(),
            },
            corpus_manifest_digest.clone(),
        ),
        (
            ArtifactKind::Graph,
            ArtifactSubject::Graph {
                revision: context.revisions.graph.clone(),
                candidate_digest: context.candidate.candidate_digest.clone(),
                construction_config_digest: context
                    .effective_configuration
                    .construction_config_digest
                    .clone(),
                corpus_manifest_digest: corpus_manifest_digest.clone(),
                preprocessing_digest: context.effective_configuration.preprocessing_digest.clone(),
            },
            digest(&format!("m20-graph-manifest-{candidate}")),
        ),
        (
            ArtifactKind::Oracle,
            ArtifactSubject::Oracle {
                revision: context.revisions.oracle.clone(),
                eval_spec_digest: context.effective_configuration.eval_spec_digest.clone(),
                case_manifest_digest: context.case_manifest.digest.clone(),
                corpus_manifest_digest: corpus_manifest_digest.clone(),
            },
            digest("m20-oracle-manifest"),
        ),
        (
            ArtifactKind::Scorer,
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
            digest("m20-scorer-manifest"),
        ),
        (
            ArtifactKind::Verifier,
            ArtifactSubject::Verifier {
                verifier_name: context.policy.oracle.verifier_name.clone(),
                verifier_version: context.policy.oracle.verifier_version.clone(),
                semantics_identity_digest: context.policy.oracle.verifier_artifact_digest.clone(),
            },
            digest("m20-verifier-manifest"),
        ),
    ];
    let attestor_registration_id = digest("m20-attestor-registration")
        .trim_start_matches("sha256:")
        .to_string();
    let mut bindings =
        subjects.into_iter().map(
            |(kind, subject, manifest_digest)| ArtifactAttestationBinding {
                kind,
                attestation_id: digest(&format!("m20-attestation-{kind:?}-{candidate}"))
                    .trim_start_matches("sha256:")
                    .to_string(),
                attestation_digest: digest(&format!("m20-record-{kind:?}-{candidate}")),
                manifest_digest,
                subject_digest: canonical_digest(&subject).unwrap(),
                entry_count: 1,
                total_bytes: 10,
                attestor_registration_id: attestor_registration_id.clone(),
                attestor_registration_digest: digest("m20-attestor-registration-record"),
                attestor_principal_id: "m20-artifact-attestor".into(),
                root_key_id: "ed25519:m20-test-root".into(),
            },
        );
    let mut set = ArtifactAttestationSet {
        corpus: bindings.next().unwrap(),
        graph: bindings.next().unwrap(),
        oracle: bindings.next().unwrap(),
        scorer: bindings.next().unwrap(),
        verifier: bindings.next().unwrap(),
        set_digest: String::new(),
    };
    set.set_digest = record_digest(&set, "set_digest").unwrap();
    context.artifact_attestations = Some(set);
    context
}
pub(super) fn test_artifact_manifest(kind: ArtifactKind, label: &str) -> ArtifactManifest {
    let byte_length = 16;
    ArtifactManifest {
        schema_version: 1,
        artifact_kind: kind,
        artifact_format: "cognigraph-test-single-file-v1".into(),
        entries: vec![ArtifactManifestEntry {
            logical_path: format!("inputs/{label}.bin"),
            media_type: "application/octet-stream".into(),
            byte_length,
            blob_digest: digest(&format!("m20-blob:{label}")),
            executable: matches!(kind, ArtifactKind::Scorer | ArtifactKind::Verifier),
        }],
        entry_count: 1,
        total_bytes: byte_length,
    }
}
pub(super) async fn store_signed_artifact_attestation(
    state: &AppState,
    attestor: &TestGovernancePrincipal,
    kind: ArtifactKind,
    label: &str,
    subject: ArtifactSubject,
) -> ArtifactAttestationRecord {
    assert_eq!(subject.kind(), kind);
    let manifest = test_artifact_manifest(kind, label);
    let manifest_digest = canonical_digest(&manifest).unwrap();
    let subject_digest = canonical_digest(&subject).unwrap();
    let attestation_id = artifact_attestation_id(
        TENANT,
        INCARNATION,
        kind,
        &manifest_digest,
        &subject_digest,
        &attestor.record.registration_id,
    )
    .unwrap();
    let now = now_millis();
    let statement = GovernanceStatement::new(
        ARTIFACT_ATTESTATION_DOMAIN,
        TENANT,
        INCARNATION,
        ArtifactAttestationPayload {
            attestation_id,
            manifest,
            manifest_digest,
            subject,
            locations: vec![ArtifactLocationObservation {
                uri: format!("urn:cognigraph:test:m20:{label}"),
                observed_at_ms: now,
                object_version: Some("immutable-v1".into()),
                etag: None,
                last_modified: None,
                declared_length: Some(16),
                content_encoding: None,
            }],
            attestor_registration_id: attestor.record.registration_id.clone(),
            attestor_principal_id: attestor.record.principal_id.clone(),
            hash_completed_at_ms: now,
            signed_at_ms: now,
        },
    );
    state
        .promotions
        .create_artifact_attestation(
            TENANT,
            INCARNATION,
            attestor.actor.clone(),
            &format!("attest-{label}"),
            CreateArtifactAttestationRequest {
                attestor_signature: attestor.signing_key.sign(&statement).unwrap(),
                statement,
            },
        )
        .await
        .unwrap()
        .record
}
pub(super) struct TestArtifactCasRoot(pub(super) PathBuf);
impl TestArtifactCasRoot {
    pub(super) fn new() -> Self {
        static NEXT_ROOT: AtomicU64 = AtomicU64::new(0);
        loop {
            let nonce = NEXT_ROOT.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!(
                "cognigraph-m21-cas-{}-{}-{nonce}",
                std::process::id(),
                now_millis()
            ));
            match fs::create_dir(&root) {
                Ok(()) => {
                    fs::create_dir(root.join("tenants"))
                        .expect("create M21 CAS fixture tenant root");
                    return Self(root);
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("create unique M21 CAS fixture: {error}"),
            }
        }
    }
}
impl Drop for TestArtifactCasRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
