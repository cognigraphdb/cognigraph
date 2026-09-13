//! Generation size.

use super::*;

pub(super) fn require_canonical_generation_byte_bound(bytes: u64) -> Result<(), CogniGraphError> {
    if bytes > MAX_M26_CANONICAL_GENERATION_BYTES as u64 {
        return Err(capacity(format!(
            "M26 canonical generation has {bytes} bytes, exceeding {MAX_M26_CANONICAL_GENERATION_BYTES}"
        )));
    }
    Ok(())
}
pub(super) fn stabilize_generation_record_size(
    record: &mut SemanticRepairGenerationRecord,
) -> Result<(), CogniGraphError> {
    for _ in 0..8 {
        record.semantic_repair_generation_digest =
            record_digest(record, "semantic_repair_generation_digest")?;
        let bytes = canonical_json_bytes(record)?.len() as u64;
        if bytes == record.canonical_record_bytes {
            require_canonical_generation_byte_bound(bytes)?;
            return Ok(());
        }
        record.canonical_record_bytes = bytes;
    }
    Err(conflict(
        "M26 canonical generation byte-length binding did not stabilize",
    ))
}
