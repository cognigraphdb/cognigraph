//! Supported plans.

use super::*;

impl ArtifactConsumptionPlan {
    pub fn supported_v1() -> Self {
        let slot = |artifact_kind, artifact_format: &str, entrypoint: Option<&str>, mode| {
            ArtifactConsumptionSlotPlan {
                artifact_kind,
                artifact_format: artifact_format.into(),
                entrypoint: entrypoint.map(str::to_string),
                auxiliary_entrypoints: Vec::new(),
                mode,
            }
        };
        let mut plan = Self {
            schema_version: M21_CONSUMPTION_PLAN_SCHEMA_VERSION,
            resolver: LOCAL_CAS_RESOLVER.into(),
            loader_id: ARTIFACT_LOADER_ID.into(),
            loader_version: ARTIFACT_LOADER_VERSION.into(),
            loader_semantics_digest: digest_bytes(ARTIFACT_LOADER_SEMANTICS.as_bytes()),
            scorer_abi_digest: digest_bytes(SCORER_ABI.as_bytes()),
            verifier_abi_digest: digest_bytes(VERIFIER_ABI.as_bytes()),
            corpus: slot(
                ArtifactKind::Corpus,
                CORPUS_ARTIFACT_FORMAT,
                None,
                ArtifactConsumptionMode::CompleteManifest,
            ),
            graph: slot(
                ArtifactKind::Graph,
                GRAPH_ARTIFACT_FORMAT,
                Some(GRAPH_ENTRYPOINT),
                ArtifactConsumptionMode::EvaluationGraphJson,
            ),
            oracle: slot(
                ArtifactKind::Oracle,
                ORACLE_ARTIFACT_FORMAT,
                Some(ORACLE_ENTRYPOINT),
                ArtifactConsumptionMode::PromotionOracleJson,
            ),
            scorer: slot(
                ArtifactKind::Scorer,
                EXECUTABLE_ARTIFACT_FORMAT,
                Some(EXECUTABLE_ENTRYPOINT),
                ArtifactConsumptionMode::CurrentExecutable,
            ),
            verifier: slot(
                ArtifactKind::Verifier,
                EXECUTABLE_ARTIFACT_FORMAT,
                Some(EXECUTABLE_ENTRYPOINT),
                ArtifactConsumptionMode::CurrentExecutable,
            ),
            derivation: None,
            preparation: None,
            plan_digest: String::new(),
        };
        plan.plan_digest = record_digest(&plan, "plan_digest")
            .expect("the pinned M21 consumption plan is canonical");
        plan
    }

    pub fn supported_v2() -> Self {
        let slot = |artifact_kind, artifact_format: &str, entrypoint: Option<&str>, mode| {
            ArtifactConsumptionSlotPlan {
                artifact_kind,
                artifact_format: artifact_format.into(),
                entrypoint: entrypoint.map(str::to_string),
                auxiliary_entrypoints: Vec::new(),
                mode,
            }
        };
        let mut graph = slot(
            ArtifactKind::Graph,
            M22_GRAPH_ARTIFACT_FORMAT,
            Some(GRAPH_ENTRYPOINT),
            ArtifactConsumptionMode::ReproducibleEvaluationGraphPackage,
        );
        graph.auxiliary_entrypoints = vec![CANDIDATE_ENTRYPOINT.into()];
        let mut plan = Self {
            schema_version: M22_CONSUMPTION_PLAN_SCHEMA_VERSION,
            resolver: LOCAL_CAS_RESOLVER.into(),
            loader_id: ARTIFACT_LOADER_ID.into(),
            loader_version: M22_ARTIFACT_LOADER_VERSION.into(),
            loader_semantics_digest: digest_bytes(M22_ARTIFACT_LOADER_SEMANTICS.as_bytes()),
            scorer_abi_digest: digest_bytes(SCORER_ABI.as_bytes()),
            verifier_abi_digest: digest_bytes(VERIFIER_ABI.as_bytes()),
            corpus: slot(
                ArtifactKind::Corpus,
                M22_CORPUS_ARTIFACT_FORMAT,
                Some(CORPUS_ENTRYPOINT),
                ArtifactConsumptionMode::PreparedChunkCorpusJson,
            ),
            graph,
            oracle: slot(
                ArtifactKind::Oracle,
                ORACLE_ARTIFACT_FORMAT,
                Some(ORACLE_ENTRYPOINT),
                ArtifactConsumptionMode::PromotionOracleJson,
            ),
            scorer: slot(
                ArtifactKind::Scorer,
                EXECUTABLE_ARTIFACT_FORMAT,
                Some(EXECUTABLE_ENTRYPOINT),
                ArtifactConsumptionMode::CurrentExecutable,
            ),
            verifier: slot(
                ArtifactKind::Verifier,
                EXECUTABLE_ARTIFACT_FORMAT,
                Some(EXECUTABLE_ENTRYPOINT),
                ArtifactConsumptionMode::CurrentExecutable,
            ),
            derivation: Some(Box::new(CorpusGraphDerivationPlan::supported_v1())),
            preparation: None,
            plan_digest: String::new(),
        };
        plan.plan_digest = record_digest(&plan, "plan_digest")
            .expect("the pinned M22 consumption plan is canonical");
        plan
    }

    pub fn supported_v3() -> Self {
        let slot = |artifact_kind, artifact_format: &str, entrypoint: Option<&str>, mode| {
            ArtifactConsumptionSlotPlan {
                artifact_kind,
                artifact_format: artifact_format.into(),
                entrypoint: entrypoint.map(str::to_string),
                auxiliary_entrypoints: Vec::new(),
                mode,
            }
        };
        let mut corpus = slot(
            ArtifactKind::Corpus,
            M23_CORPUS_ARTIFACT_FORMAT,
            Some(CORPUS_ENTRYPOINT),
            ArtifactConsumptionMode::ReproduciblePreparedChunkCorpusPackage,
        );
        corpus.auxiliary_entrypoints = vec![DOCUMENTS_ENTRYPOINT.into()];
        let mut graph = slot(
            ArtifactKind::Graph,
            M22_GRAPH_ARTIFACT_FORMAT,
            Some(GRAPH_ENTRYPOINT),
            ArtifactConsumptionMode::ReproducibleEvaluationGraphPackage,
        );
        graph.auxiliary_entrypoints = vec![CANDIDATE_ENTRYPOINT.into()];
        let mut plan = Self {
            schema_version: M23_CONSUMPTION_PLAN_SCHEMA_VERSION,
            resolver: LOCAL_CAS_RESOLVER.into(),
            loader_id: ARTIFACT_LOADER_ID.into(),
            loader_version: M23_ARTIFACT_LOADER_VERSION.into(),
            loader_semantics_digest: digest_bytes(M23_ARTIFACT_LOADER_SEMANTICS.as_bytes()),
            scorer_abi_digest: digest_bytes(SCORER_ABI.as_bytes()),
            verifier_abi_digest: digest_bytes(VERIFIER_ABI.as_bytes()),
            corpus,
            graph,
            oracle: slot(
                ArtifactKind::Oracle,
                ORACLE_ARTIFACT_FORMAT,
                Some(ORACLE_ENTRYPOINT),
                ArtifactConsumptionMode::PromotionOracleJson,
            ),
            scorer: slot(
                ArtifactKind::Scorer,
                EXECUTABLE_ARTIFACT_FORMAT,
                Some(EXECUTABLE_ENTRYPOINT),
                ArtifactConsumptionMode::CurrentExecutable,
            ),
            verifier: slot(
                ArtifactKind::Verifier,
                EXECUTABLE_ARTIFACT_FORMAT,
                Some(EXECUTABLE_ENTRYPOINT),
                ArtifactConsumptionMode::CurrentExecutable,
            ),
            derivation: Some(Box::new(CorpusGraphDerivationPlan::supported_v1())),
            preparation: Some(Box::new(RawCorpusPreparationPlan::supported_v1())),
            plan_digest: String::new(),
        };
        plan.plan_digest = record_digest(&plan, "plan_digest")
            .expect("the pinned M23 consumption plan is canonical");
        plan
    }

    pub fn validate(&self) -> Result<(), CogniGraphError> {
        match self.schema_version {
            M21_CONSUMPTION_PLAN_SCHEMA_VERSION if self == &Self::supported_v1() => Ok(()),
            M22_CONSUMPTION_PLAN_SCHEMA_VERSION if self == &Self::supported_v2() => self
                .derivation
                .as_deref()
                .expect("supported M22 plan carries derivation")
                .validate(),
            M23_CONSUMPTION_PLAN_SCHEMA_VERSION if self == &Self::supported_v3() => {
                self.derivation
                    .as_deref()
                    .expect("supported M23 plan carries derivation")
                    .validate()?;
                let preparation = self
                    .preparation
                    .as_deref()
                    .expect("supported M23 plan carries preparation");
                preparation.validate()?;
                if preparation.max_chunks
                    != self
                        .derivation
                        .as_deref()
                        .expect("supported M23 plan carries derivation")
                        .max_chunks
                {
                    return Err(validation(
                        "M23 preparation and derivation plans disagree on the chunk limit",
                    ));
                }
                Ok(())
            }
            _ => Err(validation(
                "artifact consumption plan does not match a pinned local-CAS loader and ABI generation",
            )),
        }
    }
}
