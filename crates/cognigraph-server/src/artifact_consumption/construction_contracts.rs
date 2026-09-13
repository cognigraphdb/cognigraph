//! Construction contracts.

use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConstructionCandidateArtifact {
    pub schema_version: u32,
    pub kind: String,
    pub id: String,
    pub revision: String,
    pub base_space_type: ConstructionSpaceType,
    pub accepted_neurons: Vec<ConstructionNeuron>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConstructionSpaceType {
    pub id: String,
    pub name: String,
    pub version: u32,
    pub description: String,
    pub entities: Vec<ConstructionEntity>,
    pub relation_rules: Vec<ConstructionRelationRule>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConstructionEntity {
    pub name: String,
    #[serde(rename = "type")]
    pub entity_type: String,
    pub aliases: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConstructionRelationRule {
    pub source: String,
    pub relation: String,
    pub target: String,
    pub when_any: Vec<String>,
    pub require_in_sentence: Vec<EndpointRef>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ConstructionNeuron {
    Alias {
        id: String,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "deserialize_non_null_option"
        )]
        reviewed_by: Option<String>,
        evidence: Vec<String>,
        entity: String,
        aliases: Vec<String>,
    },
    RelationHint {
        id: String,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "deserialize_non_null_option"
        )]
        reviewed_by: Option<String>,
        evidence: Vec<String>,
        source: String,
        relation: String,
        target: String,
        triggers: Vec<String>,
    },
    RelationBlocker {
        id: String,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "deserialize_non_null_option"
        )]
        reviewed_by: Option<String>,
        evidence: Vec<String>,
        source: String,
        relation: String,
        target: String,
        triggers: Vec<String>,
    },
}

#[derive(Serialize)]
pub(crate) struct ResolvedConstructionConfig<'a> {
    pub(crate) schema_version: u32,
    pub(crate) space_type: &'a SpaceType,
    pub(crate) vetoes: Vec<ResolvedVeto<'a>>,
}
#[derive(Serialize)]
pub(crate) struct ResolvedVeto<'a> {
    pub(super) source: &'a str,
    pub(super) relation: &'a str,
    pub(super) target: &'a str,
    pub(super) when_any: &'a [String],
}
