//! Cas fixtures.

use super::*;

pub(super) fn copy_test_tree(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let target = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_test_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}
pub(super) fn staged_blob_path(root: &Path, digest: &str) -> PathBuf {
    let hex = digest.strip_prefix("sha256:").expect("SHA-256 digest");
    root.join("tenants")
        .join(tenant_scope_hex(TENANT, INCARNATION).unwrap())
        .join("sha256")
        .join(&hex[..2])
        .join(&hex[2..])
}
pub(super) fn stage_bytes(root: &Path, bytes: &[u8]) -> (String, u64) {
    let digest = digest_bytes(bytes);
    let path = staged_blob_path(root, &digest);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
    (digest, bytes.len() as u64)
}
pub(super) fn stage_current_executable(root: &Path, digest: &str) -> u64 {
    let source = std::env::current_exe().unwrap();
    let length = fs::metadata(&source).unwrap().len();
    let destination = staged_blob_path(root, digest);
    fs::create_dir_all(destination.parent().unwrap()).unwrap();
    if !destination.exists() {
        fs::copy(source, &destination).unwrap();
    }
    length
}
pub(super) fn staged_manifest(
    root: &Path,
    kind: ArtifactKind,
    format: &str,
    logical_path: &str,
    media_type: &str,
    executable: bool,
    bytes: &[u8],
) -> ArtifactManifest {
    let (blob_digest, byte_length) = stage_bytes(root, bytes);
    ArtifactManifest {
        schema_version: 1,
        artifact_kind: kind,
        artifact_format: format.into(),
        entries: vec![ArtifactManifestEntry {
            logical_path: logical_path.into(),
            media_type: media_type.into(),
            byte_length,
            blob_digest,
            executable,
        }],
        entry_count: 1,
        total_bytes: byte_length,
    }
}
pub(super) fn staged_m22_graph_manifest(
    root: &Path,
    candidate_bytes: &[u8],
    graph_bytes: &[u8],
) -> ArtifactManifest {
    let (candidate_digest, candidate_length) = stage_bytes(root, candidate_bytes);
    let (graph_digest, graph_length) = stage_bytes(root, graph_bytes);
    ArtifactManifest {
        schema_version: 1,
        artifact_kind: ArtifactKind::Graph,
        artifact_format: M22_GRAPH_ARTIFACT_FORMAT.into(),
        entries: vec![
            ArtifactManifestEntry {
                logical_path: CANDIDATE_ENTRYPOINT.into(),
                media_type: "application/json".into(),
                byte_length: candidate_length,
                blob_digest: candidate_digest,
                executable: false,
            },
            ArtifactManifestEntry {
                logical_path: GRAPH_ENTRYPOINT.into(),
                media_type: "application/json".into(),
                byte_length: graph_length,
                blob_digest: graph_digest,
                executable: false,
            },
        ],
        entry_count: 2,
        total_bytes: candidate_length + graph_length,
    }
}
pub(super) fn staged_m23_corpus_manifest(
    root: &Path,
    corpus_bytes: &[u8],
    documents_bytes: &[u8],
) -> ArtifactManifest {
    let (corpus_digest, corpus_length) = stage_bytes(root, corpus_bytes);
    let (documents_digest, documents_length) = stage_bytes(root, documents_bytes);
    ArtifactManifest {
        schema_version: 1,
        artifact_kind: ArtifactKind::Corpus,
        artifact_format: M23_CORPUS_ARTIFACT_FORMAT.into(),
        entries: vec![
            ArtifactManifestEntry {
                logical_path: CORPUS_ENTRYPOINT.into(),
                media_type: "application/json".into(),
                byte_length: corpus_length,
                blob_digest: corpus_digest,
                executable: false,
            },
            ArtifactManifestEntry {
                logical_path: DOCUMENTS_ENTRYPOINT.into(),
                media_type: "application/json".into(),
                byte_length: documents_length,
                blob_digest: documents_digest,
                executable: false,
            },
        ],
        entry_count: 2,
        total_bytes: corpus_length + documents_length,
    }
}
pub(super) fn staged_executable_manifest(
    root: &Path,
    kind: ArtifactKind,
    executable_digest: &str,
) -> ArtifactManifest {
    let byte_length = stage_current_executable(root, executable_digest);
    ArtifactManifest {
        schema_version: 1,
        artifact_kind: kind,
        artifact_format: EXECUTABLE_ARTIFACT_FORMAT.into(),
        entries: vec![ArtifactManifestEntry {
            logical_path: EXECUTABLE_ENTRYPOINT.into(),
            media_type: "application/octet-stream".into(),
            byte_length,
            blob_digest: executable_digest.into(),
            executable: true,
        }],
        entry_count: 1,
        total_bytes: byte_length,
    }
}
pub(super) async fn store_signed_manifest_attestation(
    state: &AppState,
    attestor: &TestGovernancePrincipal,
    label: &str,
    manifest: ArtifactManifest,
    subject: ArtifactSubject,
) -> ArtifactAttestationRecord {
    assert_eq!(manifest.artifact_kind, subject.kind());
    let manifest_digest = canonical_digest(&manifest).unwrap();
    let subject_digest = canonical_digest(&subject).unwrap();
    let attestation_id = artifact_attestation_id(
        TENANT,
        INCARNATION,
        manifest.artifact_kind,
        &manifest_digest,
        &subject_digest,
        &attestor.record.registration_id,
    )
    .unwrap();
    match state
        .promotions
        .get_artifact_attestation(TENANT, INCARNATION, &attestation_id)
        .await
    {
        Ok(existing) => return existing,
        Err(CogniGraphError::DocumentNotFound { .. }) => {}
        Err(error) => panic!("cannot inspect deterministic test attestation: {error}"),
    }
    let now = now_millis();
    let statement = GovernanceStatement::new(
        ARTIFACT_ATTESTATION_DOMAIN,
        TENANT,
        INCARNATION,
        ArtifactAttestationPayload {
            attestation_id,
            manifest: manifest.clone(),
            manifest_digest,
            subject,
            locations: vec![ArtifactLocationObservation {
                uri: format!("urn:cognigraph:test:m21:{label}"),
                observed_at_ms: now,
                object_version: Some("immutable-v1".into()),
                etag: None,
                last_modified: None,
                declared_length: Some(manifest.total_bytes),
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
            &format!("m21-attest-{label}"),
            CreateArtifactAttestationRequest {
                attestor_signature: attestor.signing_key.sign(&statement).unwrap(),
                statement,
            },
        )
        .await
        .unwrap()
        .record
}
