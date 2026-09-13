//! Consumption contracts.

use super::*;

pub(crate) struct ConsumedEvaluationOutcome {
    pub result: serde_json::Value,
    pub receipt: ArtifactConsumptionReceipt,
}
pub(crate) struct ConsumptionJobBinding<'a> {
    pub tenant: &'a str,
    pub tenant_incarnation: &'a str,
    pub job_id: &'a str,
    pub attempt: u32,
    pub recoveries: u32,
    pub input_digest: &'a str,
    pub execution_payload_digest: &'a str,
    pub eval_spec_digest: &'a str,
}
pub(super) struct ConsumedManifest {
    pub(super) receipt: ConsumedArtifactReceipt,
    pub(super) retained_bytes: HashMap<String, Vec<u8>>,
}
impl ConsumedManifest {
    pub(super) fn retained(&self, logical_path: &str) -> Result<&[u8], CogniGraphError> {
        self.retained_bytes
            .get(logical_path)
            .map(Vec::as_slice)
            .ok_or_else(|| {
                validation(format!(
                    "verified `{logical_path}` entrypoint bytes were not retained"
                ))
            })
    }
}
