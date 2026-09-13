//! Plan contracts.

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactConsumptionMode {
    CompleteManifest,
    PreparedChunkCorpusJson,
    ReproduciblePreparedChunkCorpusPackage,
    EvaluationGraphJson,
    ReproducibleEvaluationGraphPackage,
    PromotionOracleJson,
    CurrentExecutable,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactConsumptionSlotPlan {
    pub artifact_kind: ArtifactKind,
    pub artifact_format: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_optional_entrypoint"
    )]
    pub entrypoint: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub auxiliary_entrypoints: Vec<String>,
    pub mode: ArtifactConsumptionMode,
}
pub(super) fn deserialize_optional_entrypoint<'de, D>(
    deserializer: D,
) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    // Missing is the only valid representation of no entrypoint. Reject an
    // explicit JSON null so the closed pinned plan has one exact wire shape.
    String::deserialize(deserializer).map(Some)
}
pub(super) fn deserialize_non_null_option<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    // `default` handles an absent field. If the field is present, deserialize
    // the inner value directly so explicit JSON null cannot be normalized into
    // omission before a durable snapshot is imported.
    T::deserialize(deserializer).map(Some)
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactConsumptionPlan {
    pub schema_version: u32,
    pub resolver: String,
    pub loader_id: String,
    pub loader_version: String,
    pub loader_semantics_digest: String,
    pub scorer_abi_digest: String,
    pub verifier_abi_digest: String,
    pub corpus: ArtifactConsumptionSlotPlan,
    pub graph: ArtifactConsumptionSlotPlan,
    pub oracle: ArtifactConsumptionSlotPlan,
    pub scorer: ArtifactConsumptionSlotPlan,
    pub verifier: ArtifactConsumptionSlotPlan,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub derivation: Option<Box<CorpusGraphDerivationPlan>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_non_null_option"
    )]
    pub preparation: Option<Box<RawCorpusPreparationPlan>>,
    pub plan_digest: String,
}
