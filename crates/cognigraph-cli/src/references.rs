//! Bounded, offline inspection and explicitly mapped repair of legacy references.
//! No normalization of keys, automatic target choice, or live mutation.
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};
use std::io::Read;
use unicode_normalization::{UnicodeNormalization, is_nfc};

const MAX_BYTES: usize = 64 << 20;
const MAX_DOCUMENTS: usize = 100_000;
const MAX_FINDINGS: usize = 2_000;
const MAX_REPORT_BYTES: usize = 2 << 20;
const MAX_CHANGES: usize = 1_000;

fn nfc(value: &str) -> String {
    value.nfc().collect()
}
fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
fn read(path: &str) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|e| format!("open {path}: {e}"))?
        .take(MAX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("read {path}: {e}"))?;
    if bytes.len() > MAX_BYTES {
        return Err("reference input exceeds 64 MiB; use a bounded export".into());
    }
    Ok(bytes)
}
fn parse(bytes: &[u8]) -> Result<Value, String> {
    serde_json::from_slice(bytes).map_err(|e| format!("invalid reference input JSON: {e}"))
}
fn object<'a>(value: &'a Value, label: &str) -> Result<&'a Map<String, Value>, String> {
    value
        .as_object()
        .ok_or_else(|| format!("{label} must be an object"))
}
fn text<'a>(value: &'a Value, field: &str) -> Result<&'a str, String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{field} must be a string"))
}
struct Record<'a> {
    collection: &'a str,
    key: &'a str,
    doc: &'a Value,
}
fn records(snapshot: &Value) -> Result<Vec<Record<'_>>, String> {
    let mut records = Vec::new();
    for (collection, entry) in object(&snapshot["collections"], "collections")? {
        let kind = text(entry, "type")?;
        if !matches!(kind, "document" | "edge") {
            return Err("unknown snapshot collection type".into());
        }
        for (key, doc) in object(&entry["documents"], "documents")? {
            object(doc, "document")?;
            if collection.len() + key.len() > 4096 {
                return Err("reference audit supports handles up to 4096 bytes".into());
            }
            if doc.get("_key").is_some_and(|v| v.as_str() != Some(key)) {
                return Err("snapshot document key disagrees with its map key".into());
            }
            records.push(Record {
                collection,
                key,
                doc,
            });
            if records.len() > MAX_DOCUMENTS {
                return Err("reference audit exceeds 100000 documents".into());
            }
        }
    }
    Ok(records)
}
// Keep aligned with the public mutation guard; never repair authority or
// generated projections through this generic offline operation.
fn protected(collection: &str) -> bool {
    collection.starts_with('_')
        || [
            "space_types",
            "neurons",
            "review_policies",
            "eval_specs",
            "entities",
            "chunks",
            "mentions",
            "facts",
            "side_views",
            "fact_semantics",
        ]
        .contains(&collection)
}
fn target_exists(snapshot: &Value, handle: &str) -> bool {
    handle.split_once('/').is_some_and(|(c, k)| {
        snapshot["collections"]
            .get(c)
            .and_then(|v| v["documents"].get(k))
            .is_some()
    })
}
#[derive(Default)]
struct Findings {
    values: Vec<Value>,
    total: usize,
    bytes: usize,
}
impl Findings {
    fn push(&mut self, value: Value) {
        self.total += 1;
        let size = value.to_string().len();
        if self.values.len() < MAX_FINDINGS && self.bytes + size <= MAX_REPORT_BYTES {
            self.bytes += size;
            self.values.push(value);
        }
    }
}

pub fn audit_file(path: &str) -> Result<Value, String> {
    audit(&read(path)?)
}
fn audit(bytes: &[u8]) -> Result<Value, String> {
    let snapshot = parse(bytes)?;
    let records = records(&snapshot)?;
    let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for record in &records {
        if !record.collection.contains('/') {
            let handle = format!("{}/{}", record.collection, record.key);
            groups.entry(nfc(&handle)).or_default().push(handle);
        }
    }
    let mut findings = Findings::default();
    let mut references_checked = 0usize;
    for record in &records {
        if record.collection.contains('/') || !is_nfc(record.collection) || !is_nfc(record.key) {
            findings.push(json!({"kind":"identity_notice","collection":record.collection,"key":record.key,
                "addressable":!record.collection.contains('/'),"action":"preserve exact identity; do not rename automatically"}));
        }
        let mut inspect = |field: &str, reference: &str| {
            references_checked += 1;
            let candidates = groups
                .get(&nfc(reference))
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            let exists = target_exists(&snapshot, reference);
            let other = candidates.iter().any(|c| c != reference);
            if !exists || other || !is_nfc(reference) {
                findings.push(json!({"kind":if !exists {"unresolved_reference"} else if other {"ambiguous_reference"} else {"exact_reference"},
                    "collection":record.collection,"key":record.key,"field":field,"reference":reference,
                    "exact_target_exists":exists,"canonical_candidates":candidates.iter().take(20).collect::<Vec<_>>(),
                    "candidate_count":candidates.len(),"protected":protected(record.collection),
                    "action":if exists && !other {"exact target resolves; preserve this reference"} else if protected(record.collection) {"cancel/resubmit jobs or regenerate/rederive through the typed workflow; do not patch"}
                    else {"verify intended target from independent source evidence before an explicit repair mapping"}}));
            }
        };
        for field in ["_from", "_to", "document_id"] {
            if let Some(reference) = record.doc.get(field).and_then(Value::as_str) {
                inspect(field, reference);
            }
        }
        // Durable side-view jobs freeze the collection separately from their keys.
        if let Some(collection) = record.doc["_execution"]["collection"].as_str()
            && let Some(keys) = record.doc["_execution"]["keys"].as_array()
        {
            for (index, key) in keys.iter().enumerate() {
                if let Some(key) = key.as_str() {
                    inspect(
                        &format!("/_execution/keys/{index}"),
                        &format!("{collection}/{key}"),
                    );
                }
            }
        }
    }
    Ok(
        json!({"snapshot_sha256":digest(bytes),"documents_checked":records.len(),"references_checked":references_checked,
        "findings_total":findings.total,"truncated":findings.total!=findings.values.len(),"findings":findings.values,
        "scope":"root _from, _to, document_id and durable _execution collection/keys only; arbitrary application reference fields are not inferred",
        "warning":"A resolved NFC handle may already target the wrong existing document. Missing originals cannot be reconstructed from normalized storage; an empty report does not establish historical correctness.",
        "limits":{"input_bytes":MAX_BYTES,"documents":MAX_DOCUMENTS,"findings":MAX_FINDINGS,"finding_bytes":MAX_REPORT_BYTES},
        "repair_plan_template":{"snapshot_sha256":digest(bytes),"changes":[]}}),
    )
}

pub fn repair_files(snapshot: &str, plan: &str) -> Result<(Vec<u8>, Value), String> {
    repair(&read(snapshot)?, &read(plan)?)
}
fn repair(bytes: &[u8], plan_bytes: &[u8]) -> Result<(Vec<u8>, Value), String> {
    let mut snapshot = parse(bytes)?;
    records(&snapshot)?;
    let plan = parse(plan_bytes)?;
    exact_fields(&plan, &["snapshot_sha256", "changes"])?;
    if text(&plan, "snapshot_sha256")? != digest(bytes) {
        return Err("repair plan does not match the exact snapshot bytes".into());
    }
    let changes = plan["changes"]
        .as_array()
        .ok_or("changes must be an array")?;
    if changes.is_empty() || changes.len() > MAX_CHANGES {
        return Err("repair requires 1..1000 explicit changes".into());
    }
    let mut seen = HashSet::new();
    for change in changes {
        exact_fields(change, &["collection", "key", "field", "from", "to"])?;
        let (collection, key, field, from, to) = (
            text(change, "collection")?,
            text(change, "key")?,
            text(change, "field")?,
            text(change, "from")?,
            text(change, "to")?,
        );
        if protected(collection) {
            return Err("repair of system, governed, or generated collections is forbidden".into());
        }
        if !matches!(field, "_from" | "_to" | "document_id") {
            return Err("repair supports only root _from, _to, or document_id".into());
        }
        if !seen.insert((collection, key, field)) {
            return Err("duplicate repair location".into());
        }
        if from == to || nfc(from) != nfc(to) {
            return Err("repair target must be a distinct canonically equivalent handle".into());
        }
        if !target_exists(&snapshot, to) {
            return Err("repair target must exist at its exact collection/key".into());
        }
        let entry = snapshot["collections"]
            .get_mut(collection)
            .ok_or("repair collection missing")?;
        if matches!(field, "_from" | "_to") && entry["type"] != "edge" {
            return Err("edge endpoint repair requires an edge collection".into());
        }
        let doc = entry["documents"]
            .get_mut(key)
            .ok_or("repair document missing")?;
        if doc.get(field).and_then(Value::as_str) != Some(from) {
            return Err("repair source value changed or is missing".into());
        }
        doc[field] = json!(to);
    }
    let output = serde_json::to_vec_pretty(&snapshot).map_err(|e| e.to_string())?;
    if output.len() > MAX_BYTES {
        return Err("repaired snapshot exceeds 64 MiB".into());
    }
    let report = json!({"source_sha256":digest(bytes),"output_sha256":digest(&output),"changes":changes.len(),
        "status":"offline snapshot written; no live import performed"});
    Ok((output, report))
}
fn exact_fields(value: &Value, fields: &[&str]) -> Result<(), String> {
    let object = object(value, "repair object")?;
    if object.len() != fields.len() || object.keys().any(|k| !fields.contains(&k.as_str())) {
        return Err(format!("repair object requires exactly {fields:?}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
