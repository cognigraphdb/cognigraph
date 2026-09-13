//! Snapshot validation.

use super::*;

impl PromotionManager {
    pub(super) async fn preflight_snapshot_locked(
        &self,
        tenant: &str,
        incarnation: &str,
        snapshot: &Value,
    ) -> Result<(), CogniGraphError> {
        let mut evidence = self
            .all_scoped_records::<PromotionEvidence>(EVIDENCE_COLLECTION, tenant, incarnation, "e")
            .await?
            .into_iter()
            .map(|record| (record.id.clone(), record))
            .collect::<BTreeMap<_, _>>();
        let mut decisions = self
            .all_scoped_records::<PromotionDecision>(DECISIONS_COLLECTION, tenant, incarnation, "d")
            .await?
            .into_iter()
            .map(|record| (record.id.clone(), record))
            .collect::<BTreeMap<_, _>>();

        if let Some(documents) = snapshot
            .pointer(&format!("/collections/{EVIDENCE_COLLECTION}/documents"))
            .and_then(Value::as_object)
        {
            for (key, value) in documents {
                let record: PromotionEvidence = serde_json::from_value(value.clone())?;
                self.validate_evidence(&record, tenant, incarnation)?;
                if record.key != *key {
                    return Err(conflict(
                        "snapshot evidence map key and embedded key differ",
                    ));
                }
                if let Some(existing) = self.backend.get_document(EVIDENCE_COLLECTION, key).await?
                    && !same_stored_record(&existing, value)?
                {
                    return Err(conflict(format!(
                        "snapshot would overwrite immutable evidence record `{key}`"
                    )));
                }
                if let Some(existing) = evidence.insert(record.id.clone(), record.clone())
                    && existing != record
                {
                    return Err(conflict(
                        "snapshot contains a divergent duplicate evidence id",
                    ));
                }
            }
        }

        if let Some(documents) = snapshot
            .pointer(&format!("/collections/{DECISIONS_COLLECTION}/documents"))
            .and_then(Value::as_object)
        {
            for (key, value) in documents {
                let record: PromotionDecision = serde_json::from_value(value.clone())?;
                self.validate_decision(&record, tenant, incarnation)?;
                if record.key != *key {
                    return Err(conflict(
                        "snapshot decision map key and embedded key differ",
                    ));
                }
                if let Some(existing) = self.backend.get_document(DECISIONS_COLLECTION, key).await?
                    && !same_stored_record(&existing, value)?
                {
                    return Err(conflict(format!(
                        "snapshot would overwrite immutable decision record `{key}`"
                    )));
                }
                if let Some(existing) = decisions.insert(record.id.clone(), record.clone())
                    && existing != record
                {
                    return Err(conflict(
                        "snapshot contains a divergent duplicate decision id",
                    ));
                }
            }
        }

        self.validate_decision_set(tenant, incarnation, &evidence, &decisions)?;
        self.preflight_governance_snapshot_locked(
            tenant,
            incarnation,
            snapshot,
            &evidence,
            &decisions,
        )
        .await?;
        self.validate_snapshot_provenance(tenant, incarnation, snapshot, &evidence)
            .await
    }

    pub(super) async fn validate_snapshot_provenance(
        &self,
        tenant: &str,
        incarnation: &str,
        snapshot: &Value,
        evidence: &BTreeMap<String, PromotionEvidence>,
    ) -> Result<(), CogniGraphError> {
        let referenced_job_ids = evidence
            .values()
            .flat_map(|record| record.runs.iter().map(|run| run.source.job_id.as_str()))
            .collect::<HashSet<_>>();
        let mut stored_sources = HashMap::<String, PromotionEvaluationSource>::new();
        for job_id in &referenced_job_ids {
            for (collection, archived) in [(JOBS_COLLECTION, false), (JOB_ARCHIVE_COLLECTION, true)]
            {
                let Some(value) = self.backend.get_document(collection, job_id).await? else {
                    continue;
                };
                let source = self.jobs.promotion_evaluation_source_from_snapshot_value(
                    tenant,
                    incarnation,
                    value,
                    archived,
                )?;
                if source.job_id != *job_id {
                    return Err(conflict(
                        "stored promotion source job key and embedded id differ",
                    ));
                }
                if let Some(existing) = stored_sources.insert((*job_id).into(), source.clone())
                    && existing != source
                {
                    return Err(conflict(
                        "stored hot and archived promotion source jobs diverge",
                    ));
                }
            }
        }
        let mut incoming_sources = HashMap::<String, PromotionEvaluationSource>::new();
        let mut incoming_jobs = HashMap::<String, (crate::jobs::JobRecord, bool)>::new();
        for (collection, archived) in [(JOBS_COLLECTION, false), (JOB_ARCHIVE_COLLECTION, true)] {
            let Some(documents) = snapshot
                .pointer(&format!("/collections/{collection}/documents"))
                .and_then(Value::as_object)
            else {
                continue;
            };
            for (key, value) in documents {
                let job = self.jobs.validate_snapshot_job_value(
                    tenant,
                    incarnation,
                    key,
                    value,
                    archived,
                )?;
                if let Some((existing, existing_archived)) =
                    incoming_jobs.insert(key.clone(), (job.clone(), archived))
                {
                    let same_lineage = match (existing_archived, archived) {
                        (false, true) => crate::jobs::archive_matches_hot(&existing, &job)?,
                        (true, false) => crate::jobs::archive_matches_hot(&job, &existing)?,
                        _ => false,
                    };
                    if !same_lineage {
                        return Err(conflict(format!(
                            "snapshot hot and archived job `{key}` have divergent lineage"
                        )));
                    }
                }
                if !referenced_job_ids.contains(key.as_str()) {
                    continue;
                }
                if let Some(existing) = self.backend.get_document(collection, key).await?
                    && !same_stored_record(&existing, value)?
                {
                    return Err(conflict(format!(
                        "snapshot would overwrite immutable promotion source job `{key}`"
                    )));
                }
                let source = self.jobs.promotion_evaluation_source_from_snapshot_value(
                    tenant,
                    incarnation,
                    value.clone(),
                    archived,
                )?;
                if source.job_id != *key {
                    return Err(conflict(
                        "snapshot promotion source job map key and embedded id differ",
                    ));
                }
                if let Some(existing) = stored_sources.get(key)
                    && existing != &source
                {
                    return Err(conflict(format!(
                        "snapshot promotion source job `{key}` diverges from stored authority"
                    )));
                }
                if let Some(existing) = incoming_sources.insert(key.clone(), source.clone())
                    && existing != source
                {
                    return Err(conflict(
                        "snapshot hot and archived promotion source jobs diverge",
                    ));
                }
            }
        }

        for record in evidence.values() {
            for run in &record.runs {
                let authority = if let Some(source) = incoming_sources.get(&run.source.job_id) {
                    source.clone()
                } else if let Some(source) = stored_sources.get(&run.source.job_id) {
                    source.clone()
                } else {
                    self.authoritative_evaluation_source(tenant, incarnation, &run.source.job_id)
                        .await?
                };
                if authority != run.source {
                    return Err(conflict(format!(
                        "promotion evidence source job `{}` does not match authoritative job data",
                        run.source.job_id
                    )));
                }
            }
        }
        Ok(())
    }
}
