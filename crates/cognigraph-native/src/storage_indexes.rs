//! Loading persisted unique-index definitions and entries (CG-86). Writes
//! go through `StoreOp` in `storage.rs`; this file only reads.

use std::collections::HashMap;

use cognigraph_core::{IndexDef, Result};
use redb::{ReadableDatabase, ReadableTable};

use crate::storage::{INDEX_ENTRIES, INDEXES, RedbStore, store_err};

/// Definitions per collection.
pub(crate) type IndexDefs = HashMap<String, Vec<IndexDef>>;
/// collection -> index name -> value key -> document key.
pub(crate) type UniqueEntries = HashMap<String, HashMap<String, HashMap<String, String>>>;

const SEP: char = '\u{0}';

impl RedbStore {
    pub(crate) fn load_indexes(&self) -> Result<(IndexDefs, UniqueEntries)> {
        let txn = self.db.begin_read().map_err(store_err)?;
        let mut defs: IndexDefs = HashMap::new();
        let table = txn.open_table(INDEXES).map_err(store_err)?;
        for entry in table.iter().map_err(store_err)? {
            let (key, value) = entry.map_err(store_err)?;
            let Some((collection, _name)) = key.value().split_once(SEP) else {
                return Err(store_err(format!("malformed index key `{}`", key.value())));
            };
            let def: IndexDef = serde_json::from_str(value.value()).map_err(store_err)?;
            defs.entry(collection.to_string()).or_default().push(def);
        }
        let mut unique: UniqueEntries = HashMap::new();
        let table = txn.open_table(INDEX_ENTRIES).map_err(store_err)?;
        for entry in table.iter().map_err(store_err)? {
            let (key, value) = entry.map_err(store_err)?;
            let mut parts = key.value().splitn(3, SEP);
            let (Some(collection), Some(index), Some(value_key)) =
                (parts.next(), parts.next(), parts.next())
            else {
                return Err(store_err(format!(
                    "malformed index entry key `{}`",
                    key.value()
                )));
            };
            unique
                .entry(collection.to_string())
                .or_default()
                .entry(index.to_string())
                .or_default()
                .insert(value_key.to_string(), value.value().to_string());
        }
        Ok((defs, unique))
    }
}
