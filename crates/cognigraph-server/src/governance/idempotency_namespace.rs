//! One Idempotency-Key namespace for all signed authority of a tenant.
//!
//! Stored-authority validation refuses a tenant whose signed records share an
//! idempotency-key hash, whatever their kind. Admission has to refuse the same
//! reuse before a record is stored: the records are append-only, so one
//! accepted request would leave authority that never validates again
//! (CG-112). Both sides iterate [`AuthorityRecordKind::ALL`], so a new signed
//! record kind joins the namespace in one place.

use super::*;

/// The signed record kinds that share one idempotency-key namespace.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AuthorityRecordKind {
    GovernanceKey,
    KeyRevocation,
    PolicyRevision,
    PolicyApproval,
    SemanticRepairRevision,
    SemanticRepairReview,
    ArtifactAttestation,
}

impl AuthorityRecordKind {
    pub(crate) const ALL: [Self; 7] = [
        Self::GovernanceKey,
        Self::KeyRevocation,
        Self::PolicyRevision,
        Self::PolicyApproval,
        Self::SemanticRepairRevision,
        Self::SemanticRepairReview,
        Self::ArtifactAttestation,
    ];

    pub(crate) fn collection(self) -> &'static str {
        match self {
            Self::GovernanceKey => GOVERNANCE_KEYS_COLLECTION,
            Self::KeyRevocation => GOVERNANCE_KEY_REVOCATIONS_COLLECTION,
            Self::PolicyRevision => POLICY_REVISIONS_COLLECTION,
            Self::PolicyApproval => POLICY_APPROVALS_COLLECTION,
            Self::SemanticRepairRevision => SEMANTIC_REPAIR_REVISIONS_COLLECTION,
            Self::SemanticRepairReview => SEMANTIC_REPAIR_REVIEWS_COLLECTION,
            Self::ArtifactAttestation => ARTIFACT_ATTESTATIONS_COLLECTION,
        }
    }

    /// The record-kind segment of the scoped storage key.
    pub(crate) fn key_segment(self) -> &'static str {
        match self {
            Self::GovernanceKey => "gk",
            Self::KeyRevocation => "gkr",
            Self::PolicyRevision => "pr",
            Self::PolicyApproval => "pa",
            Self::SemanticRepairRevision => "srr",
            Self::SemanticRepairReview => "srrv",
            Self::ArtifactAttestation => "aa",
        }
    }
}

impl GovernanceAuthority {
    fn idempotency_key_hashes(&self, kind: AuthorityRecordKind) -> Vec<&str> {
        fn hashes<'a, T: 'a>(
            records: impl Iterator<Item = &'a T>,
            hash: impl Fn(&'a T) -> &'a String,
        ) -> Vec<&'a str> {
            records.map(|record| hash(record).as_str()).collect()
        }
        match kind {
            AuthorityRecordKind::GovernanceKey => {
                hashes(self.keys.values(), |record| &record.idempotency_key_hash)
            }
            AuthorityRecordKind::KeyRevocation => hashes(self.revocations.values(), |record| {
                &record.idempotency_key_hash
            }),
            AuthorityRecordKind::PolicyRevision => hashes(self.policies.values(), |record| {
                &record.idempotency_key_hash
            }),
            AuthorityRecordKind::PolicyApproval => hashes(self.approvals.values(), |record| {
                &record.idempotency_key_hash
            }),
            AuthorityRecordKind::SemanticRepairRevision => {
                hashes(self.semantic_repair_revisions.values(), |record| {
                    &record.idempotency_key_hash
                })
            }
            AuthorityRecordKind::SemanticRepairReview => {
                hashes(self.semantic_repair_reviews.values(), |record| {
                    &record.idempotency_key_hash
                })
            }
            AuthorityRecordKind::ArtifactAttestation => hashes(self.artifacts.values(), |record| {
                &record.idempotency_key_hash
            }),
        }
    }

    /// Every stored signed record of a tenant has its own idempotency-key hash.
    pub(super) fn ensure_idempotency_hashes_unique(&self) -> Result<(), CogniGraphError> {
        let mut seen = HashSet::new();
        for kind in AuthorityRecordKind::ALL {
            for hash in self.idempotency_key_hashes(kind) {
                if !seen.insert(hash) {
                    return Err(conflict("governance records reuse an idempotency-key hash"));
                }
            }
        }
        Ok(())
    }
}

impl PromotionManager {
    /// Refuse an idempotency-key hash that any signed record of this tenant
    /// already carries. The caller holds the transition lock and has already
    /// returned an exact replay of its own record.
    pub(crate) async fn ensure_authority_idempotency_available_locked(
        &self,
        tenant: &str,
        incarnation: &str,
        idempotency_key_hash: &str,
    ) -> Result<(), CogniGraphError> {
        for kind in AuthorityRecordKind::ALL {
            let used = if is_semantic_repair_collection(kind.collection()) {
                self.semantic_repair_idempotency_hash_exists_locked(
                    kind.collection(),
                    kind.key_segment(),
                    tenant,
                    incarnation,
                    idempotency_key_hash,
                )
                .await?
            } else {
                self.backend
                    .list_documents(kind.collection(), None, None)
                    .await?
                    .iter()
                    .any(|record| {
                        record.get("tenant").and_then(Value::as_str) == Some(tenant)
                            && record.get("tenant_incarnation").and_then(Value::as_str)
                                == Some(incarnation)
                            && record.get("idempotency_key_hash").and_then(Value::as_str)
                                == Some(idempotency_key_hash)
                    })
            };
            if used {
                return Err(conflict(
                    "Idempotency-Key was already used for different signed authority",
                ));
            }
        }
        Ok(())
    }
}
