//! Heads.

use super::*;

impl PromotionManager {
    pub(super) fn validate_stored_evidence(
        &self,
        record: &PromotionEvidence,
        tenant: &str,
        incarnation: &str,
    ) -> Result<(), CogniGraphError> {
        self.validate_evidence(record, tenant, incarnation)
            .inspect_err(|error| self.record_error(tenant, error.to_string()))
    }

    pub(super) fn validate_stored_decision(
        &self,
        record: &PromotionDecision,
        tenant: &str,
        incarnation: &str,
    ) -> Result<(), CogniGraphError> {
        self.validate_decision(record, tenant, incarnation)
            .inspect_err(|error| self.record_error(tenant, error.to_string()))
    }

    pub(super) async fn get_evidence_locked(
        &self,
        tenant: &str,
        incarnation: &str,
        id: &str,
    ) -> Result<PromotionEvidence, CogniGraphError> {
        let key = scoped_key(tenant, incarnation, "e", id);
        let record = self
            .get_authority_raw(tenant, EVIDENCE_COLLECTION, &key)
            .await?
            .ok_or_else(|| not_found("promotion evidence", id))?;
        self.validate_stored_evidence(&record, tenant, incarnation)?;
        Ok(record)
    }

    pub(crate) async fn current_raw(
        &self,
        tenant: &str,
        incarnation: &str,
        target: &PromotionTarget,
    ) -> Result<Option<PromotionHead>, CogniGraphError> {
        let key = head_key(tenant, incarnation, target)?;
        let Some(record) = self
            .get_raw::<PromotionHead>(HEADS_COLLECTION, &key)
            .await?
        else {
            return Ok(None);
        };
        if !valid_head(&record, tenant, incarnation, target, &key)? {
            let error = conflict("promotion head projection is malformed");
            self.record_error(tenant, error.to_string());
            return Err(error);
        }
        Ok(Some(record))
    }

    pub(super) async fn head_for_reconciliation(
        &self,
        tenant: &str,
        incarnation: &str,
        target: &PromotionTarget,
    ) -> Result<(Option<PromotionHead>, bool), CogniGraphError> {
        let key = head_key(tenant, incarnation, target)?;
        let Some(value) = self.backend.get_document(HEADS_COLLECTION, &key).await? else {
            return Ok((None, false));
        };
        let Ok(record) = serde_json::from_value::<PromotionHead>(value) else {
            return Ok((None, true));
        };
        if valid_head(&record, tenant, incarnation, target, &key)? {
            Ok((Some(record), false))
        } else {
            Ok((None, true))
        }
    }

    pub(crate) fn head_from_decision(
        &self,
        decision: &PromotionDecision,
        selection: &PromotionSelection,
    ) -> Result<PromotionHead, CogniGraphError> {
        let mut head = PromotionHead {
            key: head_key(
                &decision.tenant,
                &decision.tenant_incarnation,
                &decision.target,
            )?,
            schema_version: if decision.schema_version == M23_PROMOTION_DECISION_SCHEMA_VERSION {
                M23_PROMOTION_HEAD_SCHEMA_VERSION
            } else if decision.schema_version == M22_PROMOTION_DECISION_SCHEMA_VERSION {
                M22_PROMOTION_HEAD_SCHEMA_VERSION
            } else if decision.schema_version == M21_PROMOTION_DECISION_SCHEMA_VERSION {
                M21_PROMOTION_HEAD_SCHEMA_VERSION
            } else {
                PROMOTION_HEAD_SCHEMA_VERSION
            },
            digest_algorithm: DIGEST_ALGORITHM.into(),
            tenant: decision.tenant.clone(),
            tenant_incarnation: decision.tenant_incarnation.clone(),
            target: decision.target.clone(),
            applied_decision_id: decision.id.clone(),
            selection: selection.clone(),
            updated_at_ms: decision.created_at_ms,
            projection_digest: String::new(),
        };
        head.projection_digest = record_digest(&head, "projection_digest")?;
        Ok(head)
    }

    pub(super) async fn ensure_head_from_decision(
        &self,
        decision: &PromotionDecision,
        selection: &PromotionSelection,
    ) -> Result<(), CogniGraphError> {
        let head = self.head_from_decision(decision, selection)?;
        self.save_head(&head).await
    }

    pub(super) async fn save_head(&self, head: &PromotionHead) -> Result<(), CogniGraphError> {
        if self
            .backend
            .get_document(HEADS_COLLECTION, &head.key)
            .await?
            .is_some()
        {
            self.backend
                .replace_document(HEADS_COLLECTION, &head.key, serde_json::to_value(head)?)
                .await?;
        } else {
            self.backend
                .create_document(HEADS_COLLECTION, serde_json::to_value(head)?)
                .await?;
        }
        Ok(())
    }
}
