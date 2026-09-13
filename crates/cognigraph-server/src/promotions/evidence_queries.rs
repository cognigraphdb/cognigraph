//! Evidence queries.

use super::*;

impl PromotionManager {
    pub(crate) async fn validate_evidence_artifact_authority_active(
        &self,
        tenant: &str,
        incarnation: &str,
        evidence: &PromotionEvidence,
        at_ms: u64,
    ) -> Result<(), CogniGraphError> {
        let Some(authority) = &evidence.artifact_attestations else {
            return Ok(());
        };
        let candidate = evidence
            .runs
            .iter()
            .find(|run| run.role == EvidenceRunRole::CandidateOriginal)
            .ok_or_else(|| conflict("promotion evidence has no candidate original run"))?;
        let baseline = evidence
            .runs
            .iter()
            .find(|run| run.role == EvidenceRunRole::BaselineOriginal)
            .ok_or_else(|| conflict("promotion evidence has no baseline original run"))?;
        self.validate_artifact_binding_set_active(
            tenant,
            incarnation,
            &candidate.source.context,
            &authority.candidate,
            at_ms,
        )
        .await?;
        self.validate_artifact_binding_set_active(
            tenant,
            incarnation,
            &baseline.source.context,
            &authority.baseline,
            at_ms,
        )
        .await
    }

    pub async fn get_evidence(
        &self,
        tenant: &str,
        incarnation: &str,
        id: &str,
    ) -> Result<PromotionEvidence, CogniGraphError> {
        validate_record_id("evidence id", id)?;
        self.ensure_repository(tenant).await?;
        let key = scoped_key(tenant, incarnation, "e", id);
        let record = self
            .get_authority_raw(tenant, EVIDENCE_COLLECTION, &key)
            .await?
            .ok_or_else(|| not_found("promotion evidence", id))?;
        self.validate_stored_evidence(&record, tenant, incarnation)?;
        Ok(record)
    }

    pub async fn list_evidence(
        &self,
        tenant: &str,
        incarnation: &str,
        limit: usize,
        cursor: Option<&str>,
    ) -> Result<PromotionPage<PromotionEvidence>, CogniGraphError> {
        self.list_records(
            EVIDENCE_COLLECTION,
            tenant,
            incarnation,
            "e",
            limit,
            cursor,
            |record| self.validate_stored_evidence(record, tenant, incarnation),
        )
        .await
    }
}
