//! Construction refusal ledger (CG-90).
//!
//! What a deterministic gate declined to build is recorded in the generated
//! `construction_refusals` collection: deterministic keys (re-submitting the
//! same nomination records once), a per-tenant cap that drops the newest
//! rows rather than evicting history, and a write that happens in the same
//! request that produced the refusal. See
//! `docs/decisions/decision_construction_refusal_ledger.md`.

use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};

use cognigraph_core::{BatchOp, CollectionType, GraphBackend, Result};
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::system_collections::REFUSALS_COLLECTION;

/// Rows per tenant store; at the cap new refusals are reported, not stored.
pub const MAX_REFUSALS_PER_TENANT: usize = 10_000;

/// Request-level facts shared by every row of one construction call.
#[derive(Debug, Clone, Copy)]
pub struct RefusalContext<'a> {
    pub space_type: &'a str,
    /// `directed` or `propose`.
    pub origin: &'a str,
    /// Extraction policy revision when the origin has one.
    pub policy: Option<&'a str>,
    /// Model attribution, e.g. `directed:<model>@directed-policy-v2`.
    pub attribution: &'a str,
    pub actor: &'a str,
}

/// One refusal in the shape both origins can produce.
#[derive(Debug, Clone, Serialize)]
pub struct RefusalRow {
    pub gate: String,
    pub source: String,
    pub relation: String,
    pub target: String,
    pub chunk_id: Option<String>,
    pub evidence: Option<String>,
    pub reason: String,
}

impl From<&cognigraph_construct::refusals::Refusal> for RefusalRow {
    fn from(r: &cognigraph_construct::refusals::Refusal) -> Self {
        Self {
            gate: r.gate.code().into(),
            source: r.source.clone(),
            relation: r.relation.clone(),
            target: r.target.clone(),
            chunk_id: Some(r.chunk_id.clone()),
            evidence: Some(r.evidence.clone()),
            reason: r.reason.clone(),
        }
    }
}

/// What one ledger append did. `rows` echoes every input refusal with its
/// key and a `stored` flag; `stored` counts distinct rows now in the ledger.
#[derive(Debug, Default)]
pub struct LedgerOutcome {
    pub rows: Vec<Value>,
    pub stored: usize,
    pub dropped: usize,
}

/// Identity of a refusal: origin, space, gate, triple, chunk and evidence.
/// Attribution, actor and reason wording are context, not identity.
pub fn refusal_key(ctx: &RefusalContext<'_>, row: &RefusalRow) -> String {
    let mut digest = Sha256::new();
    for part in [
        ctx.origin,
        ctx.space_type,
        &row.gate,
        &row.source,
        &row.relation,
        &row.target,
    ] {
        digest.update([0]);
        digest.update(part.as_bytes());
    }
    for optional in [&row.chunk_id, &row.evidence] {
        match optional {
            Some(value) => {
                digest.update([1]);
                digest.update(value.as_bytes());
            }
            None => digest.update([2]),
        }
    }
    let hex: String = digest
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    format!("refusal-{hex}")
}

pub async fn record_refusals(
    backend: &dyn GraphBackend,
    ctx: &RefusalContext<'_>,
    refusals: &[RefusalRow],
) -> Result<LedgerOutcome> {
    record_refusals_capped(backend, ctx, refusals, MAX_REFUSALS_PER_TENANT).await
}

/// Append with an explicit cap (tests exercise the boundary directly).
pub async fn record_refusals_capped(
    backend: &dyn GraphBackend,
    ctx: &RefusalContext<'_>,
    refusals: &[RefusalRow],
    cap: usize,
) -> Result<LedgerOutcome> {
    let mut outcome = LedgerOutcome::default();
    if refusals.is_empty() {
        return Ok(outcome);
    }
    let existing = backend
        .list_collections()
        .await?
        .into_iter()
        .find(|c| c.name == REFUSALS_COLLECTION)
        .map(|c| usize::try_from(c.count).unwrap_or(usize::MAX));
    let mut capacity = cap.saturating_sub(existing.unwrap_or(0));
    let recorded_at = now_secs();
    let mut seen: HashSet<String> = HashSet::new();
    let mut ops = Vec::new();
    for row in refusals {
        let key = refusal_key(ctx, row);
        let mut doc = json!({
            "_key": key,
            "space_type": ctx.space_type,
            "origin": ctx.origin,
            "gate": row.gate,
            "source": row.source,
            "relation": row.relation,
            "target": row.target,
            "chunk_id": row.chunk_id,
            "evidence": row.evidence,
            "reason": row.reason,
            "policy": ctx.policy,
            "attribution": ctx.attribution,
            "actor": ctx.actor,
            "recorded_at": recorded_at,
        });
        let stored = if seen.contains(&key) {
            true
        } else if existing.is_some()
            && backend
                .get_document(REFUSALS_COLLECTION, &key)
                .await?
                .is_some()
        {
            seen.insert(key);
            outcome.stored += 1;
            true
        } else if capacity > 0 {
            capacity -= 1;
            seen.insert(key);
            outcome.stored += 1;
            ops.push(BatchOp::Insert {
                collection: REFUSALS_COLLECTION.into(),
                doc: doc.clone(),
            });
            true
        } else {
            outcome.dropped += 1;
            false
        };
        doc["stored"] = json!(stored);
        outcome.rows.push(doc);
    }
    if !ops.is_empty() {
        backend
            .ensure_collection(REFUSALS_COLLECTION, CollectionType::Document)
            .await?;
        backend.execute_batch(ops).await?;
    }
    Ok(outcome)
}

/// Put the ledger outcome on a construction response. A ledger failure is
/// reported, never allowed to fail the construction that already happened.
pub fn attach(response: &mut Value, outcome: Result<LedgerOutcome>) {
    let outcome = match outcome {
        Ok(outcome) => outcome,
        Err(error) => {
            tracing::warn!(error = %error, "construction refusal ledger write failed");
            response["refusals_error"] = json!(error.to_string());
            LedgerOutcome::default()
        }
    };
    response["refusals"] = json!(outcome.rows);
    response["refusals_stored"] = json!(outcome.stored);
    response["refusals_dropped"] = json!(outcome.dropped);
}

fn now_secs() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

#[cfg(test)]
mod tests;
