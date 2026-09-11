//! Repository.

use super::*;

impl PromotionManager {
    /// Configure the one out-of-store trust anchor. Missing configuration is
    /// permitted for graph-only or historical M18 deployments, but all M19
    /// mutations fail closed until a valid root is configured.
    pub fn configure_governance_root(
        &self,
        public_key: Option<&str>,
    ) -> Result<(), CogniGraphError> {
        let root = public_key
            .map(|public_key| {
                let key_id = key_id_from_public_key(public_key).map_err(governance_config_error)?;
                let key = VerificationKey {
                    schema_version: cognigraph_governance::GOVERNANCE_SCHEMA_VERSION,
                    algorithm: cognigraph_governance::SIGNATURE_ALGORITHM.into(),
                    key_id,
                    purpose: KeyPurpose::TrustRoot,
                    public_key: public_key.into(),
                };
                key.validate().map_err(governance_config_error)?;
                Ok::<VerificationKey, CogniGraphError>(key)
            })
            .transpose()?;
        *self.governance_root.write().expect("governance root lock") = root;
        Ok(())
    }

    pub fn governance_root_key_id(&self) -> Option<String> {
        self.governance_root
            .read()
            .expect("governance root lock")
            .as_ref()
            .map(|root| root.key_id.clone())
    }

    pub(crate) fn root_key(&self) -> Result<VerificationKey, CogniGraphError> {
        self.governance_root
        .read()
        .expect("governance root lock")
        .clone()
        .ok_or_else(|| {
            CogniGraphError::ConnectionError(
                "signed governance is unavailable: COGNIGRAPH_GOVERNANCE_ROOT_PUBLIC_KEY is not configured"
                    .into(),
            )
        })
    }

    pub(crate) async fn ensure_governance_repository(
        &self,
        tenant: &str,
    ) -> Result<(), CogniGraphError> {
        for collection in [
            GOVERNANCE_KEYS_COLLECTION,
            GOVERNANCE_KEY_REVOCATIONS_COLLECTION,
            POLICY_REVISIONS_COLLECTION,
            POLICY_APPROVALS_COLLECTION,
            ARTIFACT_ATTESTATIONS_COLLECTION,
            SEMANTIC_REPAIR_REVISIONS_COLLECTION,
            SEMANTIC_REPAIR_REVIEWS_COLLECTION,
        ] {
            if let Err(error) = self
                .backend
                .ensure_collection(collection, CollectionType::Document)
                .await
            {
                self.record_error(
                    tenant,
                    format!("governance repository initialization failed: {error}"),
                );
                return Err(error);
            }
        }
        Ok(())
    }
}
