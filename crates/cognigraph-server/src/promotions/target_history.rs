//! Target history.

use super::*;

impl PromotionManager {
    pub(super) async fn target_decisions(
        &self,
        tenant: &str,
        incarnation: &str,
        target: &PromotionTarget,
    ) -> Result<Vec<PromotionDecision>, CogniGraphError> {
        let predicates = [
            FieldPredicate {
                path: vec!["tenant".into()],
                op: PredicateOp::Eq,
                value: json!(tenant),
            },
            FieldPredicate {
                path: vec!["tenant_incarnation".into()],
                op: PredicateOp::Eq,
                value: json!(incarnation),
            },
            FieldPredicate {
                path: vec!["target".into(), "space_type".into()],
                op: PredicateOp::Eq,
                value: json!(target.space_type),
            },
            FieldPredicate {
                path: vec!["target".into(), "channel".into()],
                op: PredicateOp::Eq,
                value: json!(target.channel),
            },
        ];
        let fields = promotion_record_fields(DECISIONS_COLLECTION)?;
        let rows = self
            .backend
            .list_documents_filtered(DECISIONS_COLLECTION, &predicates, Some(&fields), None, None)
            .await?;
        rows.into_iter()
            .map(|value| {
                let record: PromotionDecision = serde_json::from_value(value).map_err(|error| {
                    let error = CogniGraphError::from(error);
                    self.record_error(tenant, error.to_string());
                    error
                })?;
                self.validate_stored_decision(&record, tenant, incarnation)?;
                if record.target != *target {
                    let error =
                        conflict("promotion target decision filter returned another target");
                    self.record_error(tenant, error.to_string());
                    return Err(error);
                }
                Ok(record)
            })
            .collect()
    }

    pub(crate) fn record_error(&self, tenant: &str, error: String) {
        self.errors
            .lock()
            .expect("promotion health lock")
            .insert(tenant.into(), error);
    }

    pub(crate) fn ensure_mutations_healthy(&self, tenant: &str) -> Result<(), CogniGraphError> {
        if let Some(error) = self
            .errors
            .lock()
            .expect("promotion health lock")
            .get(tenant)
            .cloned()
        {
            return Err(CogniGraphError::ConnectionError(format!(
                "promotion authority is degraded; new mutations are blocked until recovery succeeds: {error}"
            )));
        }
        Ok(())
    }

    pub(crate) fn ensure_tenant_active(&self, tenant: &str) -> Result<(), CogniGraphError> {
        if self
            .paused_tenants
            .lock()
            .expect("promotion tenant fence lock")
            .contains(tenant)
        {
            return Err(CogniGraphError::Forbidden(format!(
                "tenant `{tenant}` promotion operations are suspended"
            )));
        }
        Ok(())
    }
}
