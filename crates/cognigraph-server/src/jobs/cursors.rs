//! Cursors.

use super::*;

pub(super) fn list_cursor_fingerprint(
    scope: &str,
    kind: Option<JobKind>,
    status: Option<JobStatus>,
    archived: ArchiveFilter,
) -> Result<String, CogniGraphError> {
    digest_json(&json!({
        "scope": scope,
        "kind": kind,
        "status": status,
        "archived": archived,
    }))
}
pub(super) fn encode_scoped_cursor(purpose: &str, fingerprint: &str, position: &str) -> String {
    format!(
        "v2.{purpose}.{}.{}",
        &fingerprint[..16],
        URL_SAFE_NO_PAD.encode(position.as_bytes())
    )
}
pub(super) fn decode_cursor(
    cursor: &str,
    purpose: &str,
    fingerprint: &str,
) -> Result<String, CogniGraphError> {
    let mut parts = cursor.splitn(4, '.');
    let version = parts.next();
    let valid = parts.next() == Some(purpose) && parts.next() == Some(&fingerprint[..16]);
    let encoded = parts.next();
    if !valid || encoded.is_none() {
        return Err(CogniGraphError::ValidationError(format!(
            "invalid or stale {purpose} cursor"
        )));
    }
    let encoded = encoded.unwrap();
    let position = match version {
        Some("v2") if encoded.len() <= (MAX_CURSOR_POSITION_BYTES * 4 / 3) + 4 => {
            let bytes = URL_SAFE_NO_PAD.decode(encoded).map_err(|_| {
                CogniGraphError::ValidationError(format!("invalid or stale {purpose} cursor"))
            })?;
            if bytes.is_empty() || bytes.len() > MAX_CURSOR_POSITION_BYTES {
                return Err(CogniGraphError::ValidationError(format!(
                    "invalid or stale {purpose} cursor"
                )));
            }
            String::from_utf8(bytes).map_err(|_| {
                CogniGraphError::ValidationError(format!("invalid or stale {purpose} cursor"))
            })?
        }
        // Read legacy v1 cursors so an in-flight client page does not break
        // during the M17 upgrade. New responses always emit v2.
        Some("v1")
            if !encoded.is_empty()
                && encoded.len() <= MAX_CURSOR_POSITION_BYTES
                && encoded.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'~')
                }) =>
        {
            encoded.to_string()
        }
        _ => {
            return Err(CogniGraphError::ValidationError(format!(
                "invalid or stale {purpose} cursor"
            )));
        }
    };
    Ok(position)
}
pub(super) fn decode_scoped_cursor(
    cursor: &str,
    purpose: &str,
    fingerprint: &str,
    scope: &str,
) -> Result<String, CogniGraphError> {
    let position = decode_cursor(cursor, purpose, fingerprint)?;
    if !position.starts_with(scope) {
        return Err(CogniGraphError::ValidationError(format!(
            "invalid or cross-tenant {purpose} cursor"
        )));
    }
    Ok(position)
}
pub(super) fn parse_reconcile_position(
    position: Option<&str>,
    scope: &str,
) -> Result<(&'static str, Option<String>), CogniGraphError> {
    let Some(position) = position else {
        return Ok(("live", None));
    };
    let (phase, after) = position.split_once('~').ok_or_else(|| {
        CogniGraphError::ValidationError("invalid reconciliation cursor position".into())
    })?;
    let phase = match phase {
        "live" => "live",
        "archive" => "archive",
        "catalog" => "catalog",
        _ => {
            return Err(CogniGraphError::ValidationError(
                "invalid reconciliation cursor phase".into(),
            ));
        }
    };
    if phase == "catalog" && after != scope && !after.starts_with(scope) {
        return Err(CogniGraphError::ValidationError(
            "cross-tenant reconciliation cursor".into(),
        ));
    }
    Ok((phase, (!after.is_empty()).then(|| after.to_string())))
}
