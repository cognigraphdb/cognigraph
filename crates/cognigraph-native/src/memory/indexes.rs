//! Unique constraints on document collections (CG-86).
//!
//! A unique index maps a canonical value key to the document key that holds
//! it. Definitions and entries persist in redb (`storage_indexes.rs`) and are
//! mirrored in `NativeState` for O(1) checks under the write lock, in every
//! storage mode. Checks run before the store transaction; entries are
//! written in the same transaction as the document.

use std::collections::HashMap;

use cognigraph_core::{CogniGraphError, CollectionType, IndexDef, IndexType, Result};
use serde_json::Value;

use crate::storage::StoreOp;

use super::{NativeBackend, NativeState};

/// `(index name, value key)` pairs one document contributes.
pub(super) type Entries = Vec<(String, String)>;

/// The empty delta, for call sites that only add or only remove.
pub(super) static NONE: Entries = Vec::new();

pub(super) fn index_name(def: &IndexDef) -> String {
    match &def.name {
        Some(name) => name.clone(),
        None if def.unique => format!("{}_unique", def.fields.join("_")),
        None => format!("{}_idx", def.fields.join("_")),
    }
}

/// Resolve a dotted path; `None` when any segment is absent or crosses a
/// non-object. Field names containing `.` are not addressable.
fn field_value<'a>(doc: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.')
        .try_fold(doc, |current, segment| current.as_object()?.get(segment))
}

/// The canonical key a document stores under `def`, or `None` when a sparse
/// index exempts it (any field absent or null). Non-sparse indexes store
/// absent fields as null. JSON objects serialize with sorted keys, so the
/// encoding is canonical; numbers keep their integer/float identity.
pub(super) fn value_key(def: &IndexDef, doc: &Value) -> Option<String> {
    let mut values = Vec::with_capacity(def.fields.len());
    for field in &def.fields {
        let value = field_value(doc, field).cloned().unwrap_or(Value::Null);
        if def.sparse && value.is_null() {
            return None;
        }
        values.push(value);
    }
    serde_json::to_string(&values).ok()
}

pub(super) fn validate_definition(def: &IndexDef) -> Result<()> {
    if !matches!(def.index_type, IndexType::Persistent | IndexType::Hash) {
        return Err(CogniGraphError::ValidationError(format!(
            "index type {:?} is not supported; use persistent or hash",
            def.index_type
        )));
    }
    if def.fields.is_empty() || def.fields.iter().any(|f| f.trim().is_empty()) {
        return Err(CogniGraphError::ValidationError(
            "an index needs at least one non-empty field".into(),
        ));
    }
    if let Some(name) = &def.name
        && (name.is_empty() || name.contains('\u{0}') || name.contains('/'))
    {
        return Err(CogniGraphError::ValidationError(
            "index names must be non-empty and must not contain `/` or NUL".into(),
        ));
    }
    Ok(())
}

fn violation(collection: &str, index: &str, existing: &str) -> CogniGraphError {
    CogniGraphError::UniqueViolation {
        collection: collection.to_string(),
        index: index.to_string(),
        existing: existing.to_string(),
    }
}

/// Entries a document contributes to every unique index of `collection`.
pub(super) fn entries_of(state: &NativeState, collection: &str, doc: &Value) -> Entries {
    let Some(defs) = state.indexes.get(collection) else {
        return Vec::new();
    };
    defs.iter()
        .filter(|def| def.unique)
        .filter_map(|def| value_key(def, doc).map(|vk| (index_name(def), vk)))
        .collect()
}

/// In-flight overrides for one batch: `Some(key)` = pending holder,
/// `None` = freed within the batch.
pub(super) type Shadow = HashMap<(String, String, String), Option<String>>;

/// Check `doc` (to be stored as `key`) against the stored entries plus the
/// batch shadow; returns the entries it will contribute.
pub(super) fn check(
    state: &NativeState,
    shadow: &Shadow,
    collection: &str,
    key: &str,
    doc: &Value,
) -> Result<Entries> {
    let entries = entries_of(state, collection, doc);
    for (index, vk) in &entries {
        let shadowed = shadow.get(&(collection.to_string(), index.clone(), vk.clone()));
        let holder = match shadowed {
            Some(pending) => pending.clone(),
            None => state
                .unique
                .get(collection)
                .and_then(|per_index| per_index.get(index))
                .and_then(|map| map.get(vk))
                .cloned(),
        };
        if let Some(holder) = holder
            && holder != key
        {
            return Err(violation(collection, index, &holder));
        }
    }
    Ok(entries)
}

/// Record a write in the batch shadow: old entries freed, new ones held.
pub(super) fn shadow_write(
    shadow: &mut Shadow,
    collection: &str,
    key: &str,
    removed: &Entries,
    added: &Entries,
) {
    for (index, vk) in removed {
        shadow.insert((collection.into(), index.clone(), vk.clone()), None);
    }
    for (index, vk) in added {
        shadow.insert(
            (collection.into(), index.clone(), vk.clone()),
            Some(key.to_string()),
        );
    }
}

/// Store operations for one document's entry delta: deletes before puts so
/// an unchanged value nets to a put.
pub(super) fn ops<'a>(
    collection: &'a str,
    key: &'a str,
    removed: &'a Entries,
    added: &'a Entries,
) -> Vec<StoreOp<'a>> {
    let mut ops = Vec::with_capacity(removed.len() + added.len());
    for (index, vk) in removed {
        ops.push(StoreOp::DeleteIndexEntry {
            collection,
            index,
            value_key: vk,
        });
    }
    for (index, vk) in added {
        ops.push(StoreOp::PutIndexEntry {
            collection,
            index,
            value_key: vk,
            doc_key: key,
        });
    }
    ops
}

/// Mirror a committed entry delta into the resident maps.
pub(super) fn apply(
    state: &mut NativeState,
    collection: &str,
    key: &str,
    removed: &Entries,
    added: &Entries,
) {
    let per_collection = state.unique.entry(collection.to_string()).or_default();
    for (index, vk) in removed {
        if let Some(map) = per_collection.get_mut(index)
            && map.get(vk).is_some_and(|holder| holder == key)
        {
            map.remove(vk);
        }
    }
    for (index, vk) in added {
        per_collection
            .entry(index.clone())
            .or_default()
            .insert(vk.clone(), key.to_string());
    }
}

impl NativeBackend {
    pub(super) fn ensure_index_impl(&self, collection: &str, def: &IndexDef) -> Result<()> {
        validate_definition(def)?;
        let name = index_name(def);
        let stored = IndexDef {
            name: Some(name.clone()),
            ..def.clone()
        };
        let mut state = self.write_state()?;
        // Unique constraints are a document-collection contract; non-unique
        // declarations are recorded for any collection (internal callers
        // declare them on edge collections).
        if stored.unique && state.collection_types.get(collection) == Some(&CollectionType::Edge) {
            return Err(CogniGraphError::ValidationError(format!(
                "unique indexes are declared on document collections; `{collection}` is an edge collection"
            )));
        }
        if let Some(existing) = state
            .indexes
            .get(collection)
            .and_then(|defs| defs.iter().find(|d| d.name.as_deref() == Some(&name)))
        {
            if *existing == stored {
                return Ok(());
            }
            return Err(CogniGraphError::ValidationError(format!(
                "index `{name}` on `{collection}` already exists with a different definition"
            )));
        }
        // Build the entries over existing rows; any duplicate refuses the
        // whole declaration before anything is persisted.
        let mut map: HashMap<String, String> = HashMap::new();
        if stored.unique {
            let rows: Vec<(String, Value)> = if self.paged() {
                match &self.store {
                    Some(store) if state.keys.contains_key(collection) => {
                        store.scan_collection(collection)?
                    }
                    _ => Vec::new(),
                }
            } else {
                state
                    .collections
                    .get(collection)
                    .map(|docs| docs.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                    .unwrap_or_default()
            };
            for (key, doc) in rows {
                if let Some(vk) = value_key(&stored, &doc)
                    && let Some(holder) = map.insert(vk, key.clone())
                {
                    return Err(violation(collection, &name, &holder));
                }
            }
        }
        // A unique constraint on an unknown collection materializes it as a
        // document collection; a plain declaration never creates one.
        let known = state.collection_types.contains_key(collection);
        let mut ops = Vec::new();
        if !known && stored.unique {
            ops.push(StoreOp::PutCollection {
                name: collection,
                collection_type: CollectionType::Document,
            });
        }
        ops.push(StoreOp::PutIndex {
            collection,
            name: &name,
            def: &stored,
        });
        let entries: Vec<(&String, &String)> = map.iter().collect();
        for (vk, key) in &entries {
            ops.push(StoreOp::PutIndexEntry {
                collection,
                index: &name,
                value_key: vk,
                doc_key: key,
            });
        }
        self.persist(&ops)?;
        if !known && stored.unique {
            state
                .collection_types
                .insert(collection.to_string(), CollectionType::Document);
            if self.paged() {
                state.keys.entry(collection.to_string()).or_default();
            } else {
                state.collections.entry(collection.to_string()).or_default();
            }
        }
        state
            .indexes
            .entry(collection.to_string())
            .or_default()
            .push(stored);
        if !map.is_empty() || def.unique {
            state
                .unique
                .entry(collection.to_string())
                .or_default()
                .insert(name, map);
        }
        Ok(())
    }

    pub(super) fn list_indexes_impl(&self, collection: &str) -> Result<Vec<IndexDef>> {
        let state = self.read_state()?;
        Ok(state.indexes.get(collection).cloned().unwrap_or_default())
    }

    pub(super) fn drop_index_impl(&self, collection: &str, name: &str) -> Result<bool> {
        let mut state = self.write_state()?;
        let Some(defs) = state.indexes.get_mut(collection) else {
            return Ok(false);
        };
        let Some(position) = defs.iter().position(|d| d.name.as_deref() == Some(name)) else {
            return Ok(false);
        };
        self.persist(&[StoreOp::DropIndex { collection, name }])?;
        defs.remove(position);
        if let Some(per_collection) = state.unique.get_mut(collection) {
            per_collection.remove(name);
        }
        Ok(true)
    }

    /// Forget a collection's definitions and entries after `DropCollection`.
    pub(super) fn indexes_forget_collection(&self, state: &mut NativeState, collection: &str) {
        state.indexes.remove(collection);
        state.unique.remove(collection);
    }
}

#[cfg(test)]
#[path = "indexes_tests.rs"]
mod tests;
