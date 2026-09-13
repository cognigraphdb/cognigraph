//! Authority contracts.

use super::*;

#[derive(Debug, Clone)]
pub(super) struct PinnedMaterializationAuthority {
    pub(super) head: PromotionHead,
    pub(super) decision: PromotionDecision,
    pub(super) evidence: PromotionEvidence,
    pub(super) semantic: ResolvedSemanticRepairAuthority,
}
#[derive(Debug, Clone, Serialize)]
pub struct MaterializationMutation<T> {
    pub record: T,
    pub replayed: bool,
}
#[derive(Debug, Clone, Serialize)]
pub struct DeploymentMutation {
    pub decision: SemanticRepairDeploymentDecision,
    pub head: SemanticRepairDeploymentHead,
    pub replayed: bool,
}
#[derive(Debug, Clone, Serialize)]
pub(crate) struct MaterializedRepairStatus {
    pub enabled: bool,
    pub generations: usize,
    pub deployment_decisions: usize,
    pub active_heads: usize,
    pub repair_required: bool,
    pub full_validation_performed: bool,
    pub native_atomic_only: bool,
}
pub(super) type FactKey = (String, String, String, String);
