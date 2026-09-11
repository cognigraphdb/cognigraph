//! Reconciliation contracts.

use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReconcileRequest {
    pub target: PromotionTarget,
    #[serde(default)]
    pub dry_run: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReconcileResult {
    pub target: PromotionTarget,
    pub dry_run: bool,
    pub decisions_scanned: usize,
    pub head_changed: bool,
    pub head: Option<PromotionHead>,
}
impl ReconcileResult {
    pub fn public_value(&self) -> Result<Value, CogniGraphError> {
        let mut value = serde_json::to_value(self)?;
        if let Some(head) = value.get_mut("head").and_then(Value::as_object_mut) {
            head.remove("_key");
        }
        Ok(value)
    }
}
