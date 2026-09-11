use std::time::Duration;

use cognigraph_auth::{Role, User};
use cognigraph_core::CogniGraphError;
use cognigraph_governance::{GovernanceStatement, KeyPurpose, SigningKeyMaterial};
use cognigraph_native::NativeBackend;
use serde_json::Value;

use crate::artifact_attestations::{
    ARTIFACT_ATTESTATION_DOMAIN, ArtifactAttestationPayload, ArtifactKind, ArtifactManifest,
    ArtifactManifestEntry, ArtifactSubject, CreateArtifactAttestationRequest,
    ResolveArtifactBindingsRequest, artifact_attestation_id,
};
use crate::governance::{
    ApprovePolicyRevisionRequest, CreatePolicyRevisionRequest, GovernanceActor,
    GovernanceKeyRecord, KEY_REGISTRATION_DOMAIN, KEY_REVOCATION_DOMAIN, KeyRegistrationPayload,
    KeyRevocationPayload, M20_KEY_REGISTRATION_DOMAIN, POLICY_APPROVAL_DOMAIN,
    POLICY_REVISION_DOMAIN, PROMOTION_INTENT_DOMAIN, PolicyApprovalPayload, PolicyRevisionPayload,
    PromotionIntentPayload, RegisterGovernanceKeyRequest, RevokeGovernanceKeyRequest,
    SignedPromotionIntent, approval_id, key_registration_id, key_revocation_id, policy_revision_id,
};
use crate::promotions::{
    CandidateDifference, DIGEST_ALGORITHM, ExclusionPolicy, M18_EVALUATOR_ID,
    M18_METRIC_SEMANTICS_VERSION, M18_ORACLE_VERIFIER_NAME, M18_ORACLE_VERIFIER_VERSION,
    M19_PROMOTION_DECISION_SCHEMA_VERSION, OracleAttestationStatus, OraclePolicy, OracleStageRule,
    PROMOTION_POLICY_SCHEMA_VERSION, PromotionAction, PromotionActor, PromotionDecision,
    PromotionTarget, RecallPolicy, ResolvedPromotionPolicy, RestraintPolicy, RevisionAttestation,
    canonical_digest, digest_bytes, now_millis,
};
use crate::state::AppState;

const TENANT: &str = "default";
const INCARNATION: &str = "default";

struct RegisteredPrincipal {
    signing_key: SigningKeyMaterial,
    actor: GovernanceActor,
    record: GovernanceKeyRecord,
}

fn artifact_revision(kind: &str) -> RevisionAttestation {
    RevisionAttestation {
        kind: kind.into(),
        revision_id: format!("{kind}-revision-v1"),
        manifest_uri: format!("urn:cognigraph:test:{kind}:manifest"),
        manifest_digest: digest_bytes(format!("legacy-{kind}-manifest").as_bytes()),
        source_lineage: format!("{kind}-lineage-v1"),
        immutability_method: "content-addressed-test-fixture".into(),
        issuer: "m20-test-issuer".into(),
        issued_at_ms: now_millis(),
        verification_uri: format!("urn:cognigraph:test:{kind}:verification"),
        verification_digest: digest_bytes(format!("legacy-{kind}-verification").as_bytes()),
        candidate_digest: (kind == "graph").then(|| digest_bytes(b"candidate")),
        configuration_digest: (kind == "graph").then(|| digest_bytes(b"construction-config")),
        immutable: true,
    }
}

fn artifact_subject(kind: ArtifactKind) -> ArtifactSubject {
    match kind {
        ArtifactKind::Corpus => ArtifactSubject::Corpus {
            revision: artifact_revision("corpus"),
            preprocessing_digest: digest_bytes(b"preprocessing"),
        },
        ArtifactKind::Graph => ArtifactSubject::Graph {
            revision: artifact_revision("graph"),
            candidate_digest: digest_bytes(b"candidate"),
            construction_config_digest: digest_bytes(b"construction-config"),
            corpus_manifest_digest: digest_bytes(b"corpus-content-manifest"),
            preprocessing_digest: digest_bytes(b"preprocessing"),
        },
        ArtifactKind::Oracle => ArtifactSubject::Oracle {
            revision: artifact_revision("oracle"),
            eval_spec_digest: digest_bytes(b"eval-spec"),
            case_manifest_digest: digest_bytes(b"case-manifest"),
            corpus_manifest_digest: digest_bytes(b"corpus-content-manifest"),
        },
        ArtifactKind::Scorer => ArtifactSubject::Scorer {
            scorer_id: "cognigraph-construct.evaluate".into(),
            scorer_version: "1".into(),
            semantics_identity_digest: digest_bytes(
                b"cognigraph-construct.evaluate/distinct-fact-set/v1",
            ),
            metric_semantics_version: "cognigraph.distinct-fact-set.v1".into(),
            executable_digest: digest_bytes(b"release-server-binary"),
        },
        ArtifactKind::Verifier => ArtifactSubject::Verifier {
            verifier_name: "cognigraph.oracle-separation-manifest".into(),
            verifier_version: "1".into(),
            semantics_identity_digest: digest_bytes(b"cognigraph.oracle-separation-manifest/v1"),
        },
    }
}

fn signed_artifact_request(
    attestor: &RegisteredPrincipal,
    kind: ArtifactKind,
) -> CreateArtifactAttestationRequest {
    let subject = artifact_subject(kind);
    let manifest = ArtifactManifest {
        schema_version: 1,
        artifact_kind: kind,
        artifact_format: "cognigraph-test-single-file-v1".into(),
        entries: vec![ArtifactManifestEntry {
            logical_path: format!("inputs/{kind:?}.bin").to_lowercase(),
            media_type: "application/octet-stream".into(),
            byte_length: 4,
            blob_digest: digest_bytes(format!("raw-{kind:?}-bytes").as_bytes()),
            executable: matches!(kind, ArtifactKind::Scorer | ArtifactKind::Verifier),
        }],
        entry_count: 1,
        total_bytes: 4,
    };
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
            locations: vec![crate::artifact_attestations::ArtifactLocationObservation {
                uri: format!("urn:cognigraph:test:artifact:{kind:?}").to_lowercase(),
                observed_at_ms: now,
                object_version: Some("immutable-v1".into()),
                etag: None,
                last_modified: None,
                declared_length: Some(4),
                content_encoding: None,
            }],
            attestor_registration_id: attestor.record.registration_id.clone(),
            attestor_principal_id: attestor.record.principal_id.clone(),
            hash_completed_at_ms: now,
            signed_at_ms: now,
        },
    );
    CreateArtifactAttestationRequest {
        attestor_signature: attestor.signing_key.sign(&statement).unwrap(),
        statement,
    }
}

#[tokio::test]
async fn artifact_attestations_are_signed_idempotent_and_resolve_exact_five_slots() {
    let state = AppState::new(NativeBackend::new());
    let root = SigningKeyMaterial::generate(KeyPurpose::TrustRoot).unwrap();
    state
        .promotions
        .configure_governance_root(Some(&root.verification_key().unwrap().public_key))
        .unwrap();
    let attestor = register_principal(
        &state,
        &root,
        KeyPurpose::ArtifactAttestor,
        Role::ArtifactAttestor,
        "artifact-attestor-principal",
        "artifact-attestor-user",
    )
    .await;
    let crossing_key = SigningKeyMaterial::generate(KeyPurpose::PolicyAuthor).unwrap();
    let crossing_request = registration_request(
        &root,
        &crossing_key,
        KeyPurpose::PolicyAuthor,
        "artifact-attestor-principal",
        "crossing-policy-author-user",
        KEY_REGISTRATION_DOMAIN,
        TENANT,
    );
    let crossing_error = state
        .promotions
        .register_governance_key(
            TENANT,
            INCARNATION,
            governance_actor(Role::Admin, "admin-user"),
            user(Role::PolicyAuthor, "crossing-policy-author-user"),
            "register-crossing-policy-author",
            crossing_request,
        )
        .await
        .unwrap_err();
    assert!(matches!(
        crossing_error,
        CogniGraphError::DocumentConflict(_)
    ));

    let mut backdated = signed_artifact_request(&attestor, ArtifactKind::Corpus);
    let backdated_at = attestor.record.not_before_ms.saturating_sub(1);
    backdated.statement.payload.locations[0].observed_at_ms = backdated_at;
    backdated.statement.payload.hash_completed_at_ms = backdated_at;
    backdated.statement.payload.signed_at_ms = backdated_at;
    backdated.attestor_signature = attestor.signing_key.sign(&backdated.statement).unwrap();
    let backdated_error = state
        .promotions
        .create_artifact_attestation(
            TENANT,
            INCARNATION,
            attestor.actor.clone(),
            "backdated-artifact-attestation",
            backdated,
        )
        .await
        .unwrap_err();
    assert!(matches!(
        backdated_error,
        CogniGraphError::DocumentConflict(_)
    ));
    assert!(
        state
            .promotions
            .list_artifact_attestations(TENANT, INCARNATION, 10, None)
            .await
            .unwrap()
            .records
            .is_empty()
    );

    let mut ids = Vec::new();
    let mut corpus_request = None;
    for kind in [
        ArtifactKind::Corpus,
        ArtifactKind::Graph,
        ArtifactKind::Oracle,
        ArtifactKind::Scorer,
        ArtifactKind::Verifier,
    ] {
        let request = signed_artifact_request(&attestor, kind);
        if kind == ArtifactKind::Corpus {
            corpus_request = Some(request.clone());
        }
        let idempotency_key = format!("attest-{kind:?}").to_lowercase();
        let created = state
            .promotions
            .create_artifact_attestation(
                TENANT,
                INCARNATION,
                attestor.actor.clone(),
                &idempotency_key,
                request.clone(),
            )
            .await
            .unwrap();
        assert!(!created.replayed);
        assert_eq!(created.record.artifact_kind, kind);
        assert!(created.record.public_value().unwrap().get("_key").is_none());
        let replay = state
            .promotions
            .create_artifact_attestation(
                TENANT,
                INCARNATION,
                attestor.actor.clone(),
                &idempotency_key,
                request,
            )
            .await
            .unwrap();
        assert!(replay.replayed);
        ids.push(created.record.attestation_id);
    }

    let set = state
        .promotions
        .resolve_artifact_bindings(
            TENANT,
            INCARNATION,
            &ResolveArtifactBindingsRequest {
                corpus_attestation_id: ids[0].clone(),
                graph_attestation_id: ids[1].clone(),
                oracle_attestation_id: ids[2].clone(),
                scorer_attestation_id: ids[3].clone(),
                verifier_attestation_id: ids[4].clone(),
            },
        )
        .await
        .unwrap();
    set.validate().unwrap();

    let wrong_slot = state
        .promotions
        .resolve_artifact_bindings(
            TENANT,
            INCARNATION,
            &ResolveArtifactBindingsRequest {
                corpus_attestation_id: ids[1].clone(),
                graph_attestation_id: ids[0].clone(),
                oracle_attestation_id: ids[2].clone(),
                scorer_attestation_id: ids[3].clone(),
                verifier_attestation_id: ids[4].clone(),
            },
        )
        .await;
    assert!(matches!(
        wrong_slot,
        Err(CogniGraphError::ValidationError(_))
    ));

    let page = state
        .promotions
        .list_artifact_attestations(TENANT, INCARNATION, 10, None)
        .await
        .unwrap();
    assert_eq!(page.records.len(), 5);
    assert!(page.records.iter().all(|record| record.entry_count == 1));
    let first_page = state
        .promotions
        .list_artifact_attestations(TENANT, INCARNATION, 2, None)
        .await
        .unwrap();
    assert_eq!(first_page.records.len(), 2);
    let cursor = first_page.next_cursor.as_deref().unwrap();
    assert!(cursor.contains(":aa:"));
    let second_page = state
        .promotions
        .list_artifact_attestations(TENANT, INCARNATION, 2, Some(cursor))
        .await
        .unwrap();
    assert_eq!(second_page.records.len(), 2);
    assert!(matches!(
        state
            .promotions
            .list_artifact_attestations(TENANT, INCARNATION, 51, None)
            .await,
        Err(CogniGraphError::ValidationError(_))
    ));
    let status = state
        .promotions
        .operator_status(TENANT, INCARNATION)
        .await
        .unwrap();
    assert_eq!(status["healthy"], true);
    assert_eq!(status["artifact_attestations"], 5);
    assert_eq!(status["artifact_attestations_by_kind"]["corpus"], 1);

    tokio::time::sleep(Duration::from_millis(2)).await;
    let now = now_millis();
    let revocation_statement = GovernanceStatement::new(
        KEY_REVOCATION_DOMAIN,
        TENANT,
        INCARNATION,
        KeyRevocationPayload {
            revocation_id: key_revocation_id(TENANT, INCARNATION, &attestor.record.registration_id),
            registration_id: attestor.record.registration_id.clone(),
            key_id: attestor.record.verification_key.key_id.clone(),
            public_key_digest: attestor.record.public_key_digest.clone(),
            reason: "rotate artifact attestor credentials".into(),
            signed_at_ms: now,
            effective_at_ms: now,
        },
    );
    state
        .promotions
        .revoke_governance_key(
            TENANT,
            INCARNATION,
            governance_actor(Role::Admin, "admin-user"),
            "revoke-artifact-attestor",
            RevokeGovernanceKeyRequest {
                root_signature: root.sign(&revocation_statement).unwrap(),
                statement: revocation_statement,
            },
        )
        .await
        .unwrap();

    let historical_replay = state
        .promotions
        .create_artifact_attestation(
            TENANT,
            INCARNATION,
            attestor.actor.clone(),
            "attest-corpus",
            corpus_request.unwrap(),
        )
        .await
        .unwrap();
    assert!(historical_replay.replayed);
    state
        .promotions
        .recover_tenant(TENANT, INCARNATION)
        .await
        .unwrap();
    let revoked_resolution = state
        .promotions
        .resolve_artifact_bindings(
            TENANT,
            INCARNATION,
            &ResolveArtifactBindingsRequest {
                corpus_attestation_id: ids[0].clone(),
                graph_attestation_id: ids[1].clone(),
                oracle_attestation_id: ids[2].clone(),
                scorer_attestation_id: ids[3].clone(),
                verifier_attestation_id: ids[4].clone(),
            },
        )
        .await;
    assert!(matches!(
        revoked_resolution,
        Err(CogniGraphError::Forbidden(_))
    ));
}

fn user(role: Role, key: &str) -> User {
    User {
        key: key.into(),
        username: format!("{key}@example.test"),
        role,
        tenant: TENANT.into(),
    }
}

fn governance_actor(role: Role, key: &str) -> GovernanceActor {
    GovernanceActor::from_user(user(role, key))
}

fn registration_request(
    root: &SigningKeyMaterial,
    signing_key: &SigningKeyMaterial,
    purpose: KeyPurpose,
    principal_id: &str,
    subject_user_key: &str,
    domain: &str,
    tenant: &str,
) -> RegisterGovernanceKeyRequest {
    let now = now_millis();
    let verification_key = signing_key.verification_key().unwrap();
    assert_eq!(verification_key.purpose, purpose);
    let registration_id = key_registration_id(TENANT, INCARNATION, &verification_key.key_id);
    let statement = GovernanceStatement::new(
        domain,
        tenant,
        INCARNATION,
        KeyRegistrationPayload {
            registration_id,
            principal_id: principal_id.into(),
            subject_user_key: subject_user_key.into(),
            verification_key,
            not_before_ms: now.saturating_sub(1),
            not_after_ms: None,
            signed_at_ms: now,
        },
    );
    RegisterGovernanceKeyRequest {
        root_signature: root.sign(&statement).unwrap(),
        possession_signature: signing_key.sign(&statement).unwrap(),
        statement,
    }
}

async fn register_principal(
    state: &AppState,
    root: &SigningKeyMaterial,
    purpose: KeyPurpose,
    role: Role,
    principal_id: &str,
    user_key: &str,
) -> RegisteredPrincipal {
    let signing_key = SigningKeyMaterial::generate(purpose).unwrap();
    let actor = governance_actor(role, user_key);
    let request = registration_request(
        root,
        &signing_key,
        purpose,
        principal_id,
        user_key,
        if purpose == KeyPurpose::ArtifactAttestor {
            M20_KEY_REGISTRATION_DOMAIN
        } else {
            KEY_REGISTRATION_DOMAIN
        },
        TENANT,
    );
    let mutation = state
        .promotions
        .register_governance_key(
            TENANT,
            INCARNATION,
            governance_actor(Role::Admin, "admin-user"),
            user(role, user_key),
            &format!("register-{principal_id}"),
            request,
        )
        .await
        .unwrap();
    assert!(!mutation.replayed);
    RegisteredPrincipal {
        signing_key,
        actor,
        record: mutation.record,
    }
}

fn resolved_policy(revision: &str) -> ResolvedPromotionPolicy {
    ResolvedPromotionPolicy {
        schema_version: PROMOTION_POLICY_SCHEMA_VERSION,
        policy_id: "governed-strict-v1".into(),
        policy_revision: revision.into(),
        source_digest: digest_bytes(format!("policy-source:{revision}").as_bytes()),
        allowed_candidate_kinds: vec!["semantic-neuron-bundle".into()],
        allowed_candidate_differences: vec![
            CandidateDifference::CandidateIdentity,
            CandidateDifference::ConstructionConfiguration,
            CandidateDifference::GraphRevision,
            CandidateDifference::ResolvedConfiguration,
        ],
        evaluator_id: M18_EVALUATOR_ID.into(),
        metric_semantics_version: M18_METRIC_SEMANTICS_VERSION.into(),
        recall: RecallPolicy {
            min_expected_distinct: 1,
            min_ratio_numerator: 1,
            min_ratio_denominator: 1,
            max_missing: 0,
            max_additional_missing_vs_baseline: 0,
        },
        restraint: RestraintPolicy {
            min_forbidden_distinct: 1,
            min_ratio_numerator: 1,
            min_ratio_denominator: 1,
            max_violations: 0,
            max_additional_violations_vs_baseline: 0,
        },
        exclusions: ExclusionPolicy {
            allowed_reason_codes: Vec::new(),
            max_count: 0,
            require_same_manifest_as_baseline: true,
        },
        required_runs: 2,
        require_exact_replay: true,
        oracle: OraclePolicy {
            required_status: OracleAttestationStatus::Verified,
            verifier_name: M18_ORACLE_VERIFIER_NAME.into(),
            verifier_version: M18_ORACLE_VERIFIER_VERSION.into(),
            verifier_artifact_digest: digest_bytes(b"cognigraph.oracle-separation-manifest/v1"),
            required_stages: vec![
                OracleStageRule {
                    stage: "candidate_build".into(),
                    allow_oracle_reads: false,
                },
                OracleStageRule {
                    stage: "candidate_tuning".into(),
                    allow_oracle_reads: false,
                },
                OracleStageRule {
                    stage: "construction".into(),
                    allow_oracle_reads: false,
                },
                OracleStageRule {
                    stage: "evaluation".into(),
                    allow_oracle_reads: true,
                },
            ],
        },
    }
}

fn target() -> PromotionTarget {
    PromotionTarget {
        space_type: "semantic-neurons".into(),
        channel: "stable".into(),
    }
}

fn policy_request(
    author: &RegisteredPrincipal,
    policy: ResolvedPromotionPolicy,
) -> CreatePolicyRevisionRequest {
    let target = target();
    let statement = GovernanceStatement::new(
        POLICY_REVISION_DOMAIN,
        TENANT,
        INCARNATION,
        PolicyRevisionPayload {
            policy_revision_id: policy_revision_id(TENANT, INCARNATION, &target, &policy).unwrap(),
            target,
            resolved_policy_digest: canonical_digest(&policy).unwrap(),
            resolved_policy: policy,
            author_registration_id: author.record.registration_id.clone(),
            author_principal_id: author.record.principal_id.clone(),
            signed_at_ms: now_millis(),
        },
    );
    CreatePolicyRevisionRequest {
        author_signature: author.signing_key.sign(&statement).unwrap(),
        statement,
    }
}

fn corrupt_signature(signature: &mut String) {
    let replacement = if signature.starts_with('A') { "B" } else { "A" };
    signature.replace_range(..1, replacement);
}

fn assert_storage_fields_absent(value: &Value) {
    match value {
        Value::Object(object) => {
            assert!(
                !object.contains_key("_key"),
                "public JSON leaked `_key`: {value}"
            );
            for child in object.values() {
                assert_storage_fields_absent(child);
            }
        }
        Value::Array(array) => {
            for child in array {
                assert_storage_fields_absent(child);
            }
        }
        _ => {}
    }
}

#[tokio::test]
async fn key_registration_rejects_tampered_proofs_and_foreign_scope() {
    let root = SigningKeyMaterial::generate(KeyPurpose::TrustRoot).unwrap();
    let state = AppState::new(NativeBackend::new());
    state
        .promotions
        .configure_governance_root(Some(&root.public_key))
        .unwrap();

    let root_tamper_key = SigningKeyMaterial::generate(KeyPurpose::PolicyAuthor).unwrap();
    let mut root_tamper = registration_request(
        &root,
        &root_tamper_key,
        KeyPurpose::PolicyAuthor,
        "root-tamper-principal",
        "root-tamper-user",
        KEY_REGISTRATION_DOMAIN,
        TENANT,
    );
    let mut omitted_null = serde_json::to_value(&root_tamper).unwrap();
    omitted_null
        .pointer_mut("/statement/payload")
        .and_then(Value::as_object_mut)
        .unwrap()
        .remove("not_after_ms");
    assert!(serde_json::from_value::<RegisterGovernanceKeyRequest>(omitted_null).is_err());
    corrupt_signature(&mut root_tamper.root_signature.signature);
    let error = state
        .promotions
        .register_governance_key(
            TENANT,
            INCARNATION,
            governance_actor(Role::Admin, "admin-user"),
            user(Role::PolicyAuthor, "root-tamper-user"),
            "root-tamper",
            root_tamper,
        )
        .await
        .unwrap_err();
    assert!(matches!(error, CogniGraphError::Forbidden(_)));

    let future_validity_key = SigningKeyMaterial::generate(KeyPurpose::PolicyAuthor).unwrap();
    let mut future_validity = registration_request(
        &root,
        &future_validity_key,
        KeyPurpose::PolicyAuthor,
        "future-validity-principal",
        "future-validity-user",
        KEY_REGISTRATION_DOMAIN,
        TENANT,
    );
    future_validity.statement.payload.not_before_ms =
        future_validity.statement.payload.signed_at_ms + 1;
    future_validity.root_signature = root.sign(&future_validity.statement).unwrap();
    future_validity.possession_signature = future_validity_key
        .sign(&future_validity.statement)
        .unwrap();
    let error = state
        .promotions
        .register_governance_key(
            TENANT,
            INCARNATION,
            governance_actor(Role::Admin, "admin-user"),
            user(Role::PolicyAuthor, "future-validity-user"),
            "future-validity",
            future_validity,
        )
        .await
        .unwrap_err();
    assert!(matches!(error, CogniGraphError::ValidationError(_)));

    let mut root_alias_key = root.verification_key().unwrap();
    root_alias_key.purpose = KeyPurpose::PolicyAuthor;
    let now = now_millis();
    let root_alias_statement = GovernanceStatement::new(
        KEY_REGISTRATION_DOMAIN,
        TENANT,
        INCARNATION,
        KeyRegistrationPayload {
            registration_id: key_registration_id(TENANT, INCARNATION, &root_alias_key.key_id),
            principal_id: "root-alias-principal".into(),
            subject_user_key: "root-alias-user".into(),
            verification_key: root_alias_key,
            not_before_ms: now.saturating_sub(1),
            not_after_ms: None,
            signed_at_ms: now,
        },
    );
    let root_alias_error = state
        .promotions
        .register_governance_key(
            TENANT,
            INCARNATION,
            governance_actor(Role::Admin, "admin-user"),
            user(Role::PolicyAuthor, "root-alias-user"),
            "root-alias",
            RegisterGovernanceKeyRequest {
                root_signature: root.sign(&root_alias_statement).unwrap(),
                possession_signature: root.sign(&root_alias_statement).unwrap(),
                statement: root_alias_statement,
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(
        root_alias_error,
        CogniGraphError::ValidationError(_)
    ));

    let possession_tamper_key = SigningKeyMaterial::generate(KeyPurpose::PolicyAuthor).unwrap();
    let mut possession_tamper = registration_request(
        &root,
        &possession_tamper_key,
        KeyPurpose::PolicyAuthor,
        "possession-tamper-principal",
        "possession-tamper-user",
        KEY_REGISTRATION_DOMAIN,
        TENANT,
    );
    corrupt_signature(&mut possession_tamper.possession_signature.signature);
    let error = state
        .promotions
        .register_governance_key(
            TENANT,
            INCARNATION,
            governance_actor(Role::Admin, "admin-user"),
            user(Role::PolicyAuthor, "possession-tamper-user"),
            "possession-tamper",
            possession_tamper,
        )
        .await
        .unwrap_err();
    assert!(matches!(error, CogniGraphError::Forbidden(_)));

    for (label, domain, tenant) in [
        ("wrong-domain", "cognigraph.not-key-registration.v1", TENANT),
        ("wrong-tenant", KEY_REGISTRATION_DOMAIN, "foreign-tenant"),
    ] {
        let signing_key = SigningKeyMaterial::generate(KeyPurpose::PolicyAuthor).unwrap();
        let request = registration_request(
            &root,
            &signing_key,
            KeyPurpose::PolicyAuthor,
            &format!("{label}-principal"),
            &format!("{label}-user"),
            domain,
            tenant,
        );
        let error = state
            .promotions
            .register_governance_key(
                TENANT,
                INCARNATION,
                governance_actor(Role::Admin, "admin-user"),
                user(Role::PolicyAuthor, &format!("{label}-user")),
                label,
                request,
            )
            .await
            .unwrap_err();
        assert!(matches!(error, CogniGraphError::ValidationError(_)));
    }

    // Canonical signing normalizes Unicode, so identity-bearing fields must
    // already be NFC or visually identical principals could cross duties.
    let decomposed_key = SigningKeyMaterial::generate(KeyPurpose::PolicyAuthor).unwrap();
    let decomposed = registration_request(
        &root,
        &decomposed_key,
        KeyPurpose::PolicyAuthor,
        "e\u{301}",
        "decomposed-user",
        KEY_REGISTRATION_DOMAIN,
        TENANT,
    );
    let error = state
        .promotions
        .register_governance_key(
            TENANT,
            INCARNATION,
            governance_actor(Role::Admin, "admin-user"),
            user(Role::PolicyAuthor, "decomposed-user"),
            "decomposed-principal",
            decomposed,
        )
        .await
        .unwrap_err();
    assert!(matches!(error, CogniGraphError::ValidationError(_)));

    assert!(
        state
            .promotions
            .list_governance_keys(TENANT, INCARNATION, 20, None)
            .await
            .unwrap()
            .records
            .is_empty()
    );
    state.jobs.shutdown().await;
}

#[tokio::test]
async fn governance_idempotency_replays_natural_identity_and_rejects_cross_request_reuse() {
    let root = SigningKeyMaterial::generate(KeyPurpose::TrustRoot).unwrap();
    let state = AppState::new(NativeBackend::new());
    state
        .promotions
        .configure_governance_root(Some(&root.public_key))
        .unwrap();
    let first_key = SigningKeyMaterial::generate(KeyPurpose::PolicyAuthor).unwrap();
    let first_request = registration_request(
        &root,
        &first_key,
        KeyPurpose::PolicyAuthor,
        "idempotent-author",
        "idempotent-author-user",
        KEY_REGISTRATION_DOMAIN,
        TENANT,
    );
    let first = state
        .promotions
        .register_governance_key(
            TENANT,
            INCARNATION,
            governance_actor(Role::Admin, "admin-user"),
            user(Role::PolicyAuthor, "idempotent-author-user"),
            "shared-idempotency-key",
            first_request.clone(),
        )
        .await
        .unwrap();
    assert!(!first.replayed);

    let replay = state
        .promotions
        .register_governance_key(
            TENANT,
            INCARNATION,
            governance_actor(Role::Admin, "admin-user"),
            user(Role::PolicyAuthor, "idempotent-author-user"),
            "shared-idempotency-key",
            first_request.clone(),
        )
        .await
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.record, first.record);

    let alternate_key_error = state
        .promotions
        .register_governance_key(
            TENANT,
            INCARNATION,
            governance_actor(Role::Admin, "admin-user"),
            user(Role::PolicyAuthor, "idempotent-author-user"),
            "different-retry-key",
            first_request,
        )
        .await
        .unwrap_err();
    assert!(matches!(
        alternate_key_error,
        CogniGraphError::DocumentConflict(_)
    ));

    let second_key = SigningKeyMaterial::generate(KeyPurpose::PolicyAuthor).unwrap();
    let second_request = registration_request(
        &root,
        &second_key,
        KeyPurpose::PolicyAuthor,
        "other-author",
        "other-author-user",
        KEY_REGISTRATION_DOMAIN,
        TENANT,
    );
    let error = state
        .promotions
        .register_governance_key(
            TENANT,
            INCARNATION,
            governance_actor(Role::Admin, "admin-user"),
            user(Role::PolicyAuthor, "other-author-user"),
            "shared-idempotency-key",
            second_request,
        )
        .await
        .unwrap_err();
    assert!(matches!(error, CogniGraphError::DocumentConflict(_)));
    state.jobs.shutdown().await;
}

#[tokio::test]
async fn independent_policy_authority_survives_later_revocation_but_cannot_be_reused() {
    let root = SigningKeyMaterial::generate(KeyPurpose::TrustRoot).unwrap();
    let state = AppState::new(NativeBackend::new());
    state
        .promotions
        .configure_governance_root(Some(&root.public_key))
        .unwrap();

    let author = register_principal(
        &state,
        &root,
        KeyPurpose::PolicyAuthor,
        Role::PolicyAuthor,
        "author-principal",
        "author-user",
    )
    .await;

    let crossing_key = SigningKeyMaterial::generate(KeyPurpose::PolicyApprover).unwrap();
    let crossing_request = registration_request(
        &root,
        &crossing_key,
        KeyPurpose::PolicyApprover,
        "author-principal",
        "crossing-approver-user",
        KEY_REGISTRATION_DOMAIN,
        TENANT,
    );
    let crossing_error = state
        .promotions
        .register_governance_key(
            TENANT,
            INCARNATION,
            governance_actor(Role::Admin, "admin-user"),
            user(Role::PolicyApprover, "crossing-approver-user"),
            "register-crossing-approver",
            crossing_request,
        )
        .await
        .unwrap_err();
    assert!(matches!(
        crossing_error,
        CogniGraphError::DocumentConflict(_)
    ));

    let approver = register_principal(
        &state,
        &root,
        KeyPurpose::PolicyApprover,
        Role::PolicyApprover,
        "approver-principal",
        "approver-user",
    )
    .await;
    let promoter = register_principal(
        &state,
        &root,
        KeyPurpose::Promoter,
        Role::Promoter,
        "promoter-principal",
        "promoter-user",
    )
    .await;

    let policy_submission = policy_request(&author, resolved_policy("1"));
    let policy_mutation = state
        .promotions
        .create_policy_revision(
            TENANT,
            INCARNATION,
            author.actor.clone(),
            "create-policy-v1",
            policy_submission.clone(),
        )
        .await
        .unwrap();
    let policy = policy_mutation.record;
    let pending_policy = state
        .promotions
        .create_policy_revision(
            TENANT,
            INCARNATION,
            author.actor.clone(),
            "create-pending-policy",
            policy_request(&author, resolved_policy("pending")),
        )
        .await
        .unwrap()
        .record;
    let approval_statement = GovernanceStatement::new(
        POLICY_APPROVAL_DOMAIN,
        TENANT,
        INCARNATION,
        PolicyApprovalPayload {
            approval_id: approval_id(TENANT, INCARNATION, &policy.policy_revision_id),
            policy_revision_id: policy.policy_revision_id.clone(),
            policy_revision_digest: policy.policy_revision_digest.clone(),
            target: policy.target.clone(),
            resolved_policy_digest: policy.resolved_policy_digest.clone(),
            author_principal_id: policy.author_principal_id.clone(),
            approver_registration_id: approver.record.registration_id.clone(),
            approver_principal_id: approver.record.principal_id.clone(),
            decision: "approve".into(),
            reason: "independent policy review passed".into(),
            signed_at_ms: now_millis(),
        },
    );
    let approval_submission = ApprovePolicyRevisionRequest {
        approval_signature: approver.signing_key.sign(&approval_statement).unwrap(),
        statement: approval_statement,
    };
    let approval = state
        .promotions
        .approve_policy_revision(
            TENANT,
            INCARNATION,
            approver.actor.clone(),
            "approve-policy-v1",
            approval_submission.clone(),
        )
        .await
        .unwrap()
        .record;
    let binding = state
        .promotions
        .policy_binding(TENANT, INCARNATION, &approval.approval_id)
        .await
        .unwrap();
    assert_eq!(binding.author_principal_id, "author-principal");
    assert_eq!(binding.approver_principal_id, "approver-principal");
    assert_ne!(binding.author_principal_id, binding.approver_principal_id);
    state
        .promotions
        .validate_policy_binding_active(
            TENANT,
            INCARNATION,
            &policy.target,
            &policy.resolved_policy,
            &binding,
        )
        .await
        .unwrap();

    for public in [
        author.record.public_value().unwrap(),
        approver.record.public_value().unwrap(),
        promoter.record.public_value().unwrap(),
        policy.public_value().unwrap(),
        approval.public_value().unwrap(),
    ] {
        assert_storage_fields_absent(&public);
        assert!(public.get("idempotency_key_hash").is_none());
    }

    let intent_payload = PromotionIntentPayload {
        requested_action: "promote".into(),
        target: policy.target.clone(),
        evidence_id: "evidence-v2".into(),
        evidence_digest: digest_bytes(b"evidence-v2"),
        policy_revision_id: Some(policy.policy_revision_id.clone()),
        policy_revision_digest: Some(policy.policy_revision_digest.clone()),
        approval_id: Some(approval.approval_id.clone()),
        approval_digest: Some(approval.approval_digest.clone()),
        artifact_authority_digest: None,
        consumption_authority_digest: None,
        derivation_authority_digest: None,
        preparation_authority_digest: None,
        gate_assessment_digest: digest_bytes(b"gates"),
        expected_head_decision_id: None,
        rollback_target_evidence_id: None,
        reason: "all governed gates passed".into(),
        idempotency_key_hash: digest_bytes(b"promote-idempotency"),
        promoter_registration_id: promoter.record.registration_id.clone(),
        promoter_principal_id: promoter.record.principal_id.clone(),
        signed_at_ms: now_millis(),
    };
    let intent_statement =
        GovernanceStatement::new(PROMOTION_INTENT_DOMAIN, TENANT, INCARNATION, intent_payload);
    let mut omitted_intent_null = serde_json::to_value(&intent_statement).unwrap();
    omitted_intent_null
        .pointer_mut("/payload")
        .and_then(Value::as_object_mut)
        .unwrap()
        .remove("expected_head_decision_id");
    assert!(
        serde_json::from_value::<GovernanceStatement<PromotionIntentPayload>>(omitted_intent_null)
            .is_err()
    );
    let decision = PromotionDecision {
        key: "private-storage-key".into(),
        schema_version: M19_PROMOTION_DECISION_SCHEMA_VERSION,
        digest_algorithm: DIGEST_ALGORITHM.into(),
        id: "decision-v2".into(),
        tenant: TENANT.into(),
        tenant_incarnation: INCARNATION.into(),
        target: policy.target.clone(),
        action: PromotionAction::Promote,
        actor: PromotionActor {
            user_key: promoter.actor.user_key.clone(),
            username: promoter.actor.username.clone(),
            role: "promoter".into(),
        },
        reason: "all governed gates passed".into(),
        created_at_ms: now_millis(),
        idempotency_key_hash: digest_bytes(b"private-decision-idempotency"),
        request_digest: digest_bytes(b"request"),
        decision_digest: digest_bytes(b"decision"),
        evidence_id: "evidence-v2".into(),
        evidence_digest: digest_bytes(b"evidence-v2"),
        policy_digest: policy.resolved_policy_digest.clone(),
        gate_assessment_digest: digest_bytes(b"gates"),
        expected_head_decision_id: None,
        predecessor_decision_id: None,
        resulting_selection: None,
        governance: Some(SignedPromotionIntent {
            promoter_signature: promoter.signing_key.sign(&intent_statement).unwrap(),
            statement: intent_statement,
            promoter_registration: promoter.record.clone(),
        }),
    };
    let public_decision = decision.public_value().unwrap();
    assert!(public_decision.get("_key").is_none());
    assert!(public_decision.get("idempotency_key_hash").is_none());
    let public_registration = public_decision
        .pointer("/governance/promoter_registration")
        .and_then(Value::as_object)
        .unwrap();
    assert!(!public_registration.contains_key("_key"));
    assert!(!public_registration.contains_key("idempotency_key_hash"));

    // Ensure revocation is temporally after the accepted policy and approval,
    // even on a millisecond-resolution clock.
    while now_millis() <= approval.approved_at_ms {
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    let revocation_statement = GovernanceStatement::new(
        KEY_REVOCATION_DOMAIN,
        TENANT,
        INCARNATION,
        KeyRevocationPayload {
            revocation_id: key_revocation_id(TENANT, INCARNATION, &author.record.registration_id),
            registration_id: author.record.registration_id.clone(),
            key_id: author.record.verification_key.key_id.clone(),
            public_key_digest: author.record.public_key_digest.clone(),
            reason: "rotate author credentials".into(),
            signed_at_ms: now_millis(),
            effective_at_ms: now_millis(),
        },
    );
    let revocation = state
        .promotions
        .revoke_governance_key(
            TENANT,
            INCARNATION,
            governance_actor(Role::Admin, "admin-user"),
            "revoke-author-v1",
            RevokeGovernanceKeyRequest {
                root_signature: root.sign(&revocation_statement).unwrap(),
                statement: revocation_statement,
            },
        )
        .await
        .unwrap()
        .record;
    assert_storage_fields_absent(&revocation.public_value().unwrap());

    // Prospective revocation does not rewrite policy/approval history.
    state
        .promotions
        .recover_tenant(TENANT, INCARNATION)
        .await
        .unwrap();

    // It does block any new use of the author authority and any active use of
    // a binding whose author is no longer trusted.
    let binding_error = state
        .promotions
        .validate_policy_binding_active(
            TENANT,
            INCARNATION,
            &policy.target,
            &policy.resolved_policy,
            &binding,
        )
        .await
        .unwrap_err();
    assert!(matches!(binding_error, CogniGraphError::Forbidden(_)));
    assert!(matches!(
        state
            .promotions
            .policy_binding(TENANT, INCARNATION, &approval.approval_id)
            .await,
        Err(CogniGraphError::Forbidden(_))
    ));
    let historical_policy_replay = state
        .promotions
        .create_policy_revision(
            TENANT,
            INCARNATION,
            author.actor.clone(),
            "create-policy-v1",
            policy_submission,
        )
        .await
        .unwrap();
    assert!(historical_policy_replay.replayed);
    assert_eq!(historical_policy_replay.record, policy);
    let future_policy_error = state
        .promotions
        .create_policy_revision(
            TENANT,
            INCARNATION,
            author.actor.clone(),
            "create-policy-v2-after-revocation",
            policy_request(&author, resolved_policy("2")),
        )
        .await
        .unwrap_err();
    assert!(matches!(future_policy_error, CogniGraphError::Forbidden(_)));

    let late_approval_statement = GovernanceStatement::new(
        POLICY_APPROVAL_DOMAIN,
        TENANT,
        INCARNATION,
        PolicyApprovalPayload {
            approval_id: approval_id(TENANT, INCARNATION, &pending_policy.policy_revision_id),
            policy_revision_id: pending_policy.policy_revision_id.clone(),
            policy_revision_digest: pending_policy.policy_revision_digest.clone(),
            target: pending_policy.target.clone(),
            resolved_policy_digest: pending_policy.resolved_policy_digest.clone(),
            author_principal_id: pending_policy.author_principal_id.clone(),
            approver_registration_id: approver.record.registration_id.clone(),
            approver_principal_id: approver.record.principal_id.clone(),
            decision: "approve".into(),
            reason: "approval must not outlive author authority".into(),
            signed_at_ms: now_millis(),
        },
    );
    let late_approval_error = state
        .promotions
        .approve_policy_revision(
            TENANT,
            INCARNATION,
            approver.actor.clone(),
            "late-approval-after-author-revocation",
            ApprovePolicyRevisionRequest {
                approval_signature: approver.signing_key.sign(&late_approval_statement).unwrap(),
                statement: late_approval_statement,
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(late_approval_error, CogniGraphError::Forbidden(_)));

    while now_millis() <= revocation.recorded_at_ms {
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    let approver_revocation_statement = GovernanceStatement::new(
        KEY_REVOCATION_DOMAIN,
        TENANT,
        INCARNATION,
        KeyRevocationPayload {
            revocation_id: key_revocation_id(TENANT, INCARNATION, &approver.record.registration_id),
            registration_id: approver.record.registration_id.clone(),
            key_id: approver.record.verification_key.key_id.clone(),
            public_key_digest: approver.record.public_key_digest.clone(),
            reason: "rotate approver credentials".into(),
            signed_at_ms: now_millis(),
            effective_at_ms: now_millis(),
        },
    );
    state
        .promotions
        .revoke_governance_key(
            TENANT,
            INCARNATION,
            governance_actor(Role::Admin, "admin-user"),
            "revoke-approver-v1",
            RevokeGovernanceKeyRequest {
                root_signature: root.sign(&approver_revocation_statement).unwrap(),
                statement: approver_revocation_statement,
            },
        )
        .await
        .unwrap();
    let historical_approval_replay = state
        .promotions
        .approve_policy_revision(
            TENANT,
            INCARNATION,
            approver.actor,
            "approve-policy-v1",
            approval_submission,
        )
        .await
        .unwrap();
    assert!(historical_approval_replay.replayed);
    assert_eq!(historical_approval_replay.record, approval);

    state.jobs.shutdown().await;
}
