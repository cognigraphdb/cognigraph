//! redb-backed durable storage for the native backend.
//!
//! Storage model: memory-primary, redb-durable — see
//! `docs/native-storage-model.md`. One write transaction per backend
//! operation, committed before the in-memory state is mutated.

use std::collections::{BTreeMap, HashMap};
use std::ops::Bound;
use std::path::Path;

use cognigraph_core::{CogniGraphError, CollectionType, FieldPredicate, Result};
use redb::{Database, ReadableDatabase, ReadableTable, TableDefinition};
use serde_json::Value;
use uuid::Uuid;

const META: TableDefinition<'_, &str, u32> = TableDefinition::new("meta");
const COLLECTIONS: TableDefinition<'_, &str, u8> = TableDefinition::new("collections");
const DOCUMENTS: TableDefinition<'_, &str, &str> = TableDefinition::new("documents");
const IDENTITY: TableDefinition<'_, &str, &str> = TableDefinition::new("identity");
/// `"{collection}\0{index}"` -> JSON `IndexDef` (CG-86).
pub(crate) const INDEXES: TableDefinition<'_, &str, &str> = TableDefinition::new("indexes");
/// `"{collection}\0{index}\0{value key}"` -> document key (CG-86).
pub(crate) const INDEX_ENTRIES: TableDefinition<'_, &str, &str> =
    TableDefinition::new("index_entries");

/// Version 3 adds the two index tables; the upgrade is additive.
const SCHEMA_VERSION: u32 = 3;
const DATA_GENERATION_KEY: &str = "data_generation";

/// Composite-key separator. Collection names and document keys must not
/// contain it; `apply` rejects offenders.
const SEP: char = '\u{0}';

/// A single mutation to persist. One `apply` call = one transaction.
pub(crate) enum StoreOp<'a> {
    PutCollection {
        name: &'a str,
        collection_type: CollectionType,
    },
    DropCollection {
        name: &'a str,
    },
    PutDocument {
        collection: &'a str,
        key: &'a str,
        doc: &'a Value,
    },
    DeleteDocument {
        collection: &'a str,
        key: &'a str,
    },
    PutIndex {
        collection: &'a str,
        name: &'a str,
        def: &'a cognigraph_core::IndexDef,
    },
    /// Removes the definition and every entry under it.
    DropIndex {
        collection: &'a str,
        name: &'a str,
    },
    PutIndexEntry {
        collection: &'a str,
        index: &'a str,
        value_key: &'a str,
        doc_key: &'a str,
    },
    DeleteIndexEntry {
        collection: &'a str,
        index: &'a str,
        value_key: &'a str,
    },
}

pub(crate) struct RedbStore {
    pub(crate) db: Database,
    database_id: String,
}

pub(crate) fn store_err(e: impl std::fmt::Display) -> CogniGraphError {
    CogniGraphError::BackendError(format!("native storage: {e}"))
}

fn check_name(name: &str) -> Result<()> {
    if name.contains(SEP) {
        return Err(CogniGraphError::ValidationError(
            "collection names and document keys must not contain NUL".into(),
        ));
    }
    Ok(())
}

pub(crate) fn composite_key(collection: &str, key: &str) -> String {
    format!("{collection}{SEP}{key}")
}

/// Every key of `table` that starts with `prefix`. Prefixes end in the
/// NUL separator, so the exclusive upper bound is the prefix with that
/// final NUL replaced by the next byte: everything under the prefix sorts
/// below it and nothing else sorts between.
fn keys_with_prefix<T: ReadableTable<&'static str, &'static str>>(
    table: &T,
    prefix: &str,
) -> Result<Vec<String>> {
    let base = prefix.strip_suffix(SEP).unwrap_or(prefix);
    let end = format!("{base}\u{1}");
    table
        .range(prefix..end.as_str())
        .map_err(store_err)?
        .map(|entry| entry.map(|(key, _)| key.value().to_string()))
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(store_err)
}

fn collection_type_to_u8(collection_type: CollectionType) -> u8 {
    match collection_type {
        CollectionType::Document => 0,
        CollectionType::Edge => 1,
    }
}

fn collection_type_from_u8(raw: u8) -> Result<CollectionType> {
    match raw {
        0 => Ok(CollectionType::Document),
        1 => Ok(CollectionType::Edge),
        other => Err(store_err(format!("unknown collection type tag {other}"))),
    }
}

impl RedbStore {
    /// Create or open the database at `path` and validate the schema version.
    pub(crate) fn open(path: &Path) -> Result<Self> {
        let db = Database::create(path).map_err(store_err)?;

        let txn = db.begin_write().map_err(store_err)?;
        let database_id;
        {
            let mut meta = txn.open_table(META).map_err(store_err)?;
            let version = meta
                .get("schema_version")
                .map_err(store_err)?
                .map(|guard| guard.value());
            match version {
                Some(v) if v > SCHEMA_VERSION => {
                    return Err(store_err(format!(
                        "database schema version {v} is newer than supported {SCHEMA_VERSION}"
                    )));
                }
                _ => {}
            }
            let mut identity = txn.open_table(IDENTITY).map_err(store_err)?;
            if version != Some(SCHEMA_VERSION) {
                // Atomic metadata-only upgrade. Older binaries must reject
                // this version: they cannot maintain the revision UUID.
                for key in ["database_id", "data_revision"] {
                    identity
                        .insert(key, Uuid::new_v4().to_string().as_str())
                        .map_err(store_err)?;
                }
                meta.insert("schema_version", SCHEMA_VERSION)
                    .map_err(store_err)?;
            }
            let read_id = |key: &str| -> Result<Uuid> {
                let value = identity
                    .get(key)
                    .map_err(store_err)?
                    .ok_or_else(|| store_err(format!("missing {key}")))?;
                Uuid::parse_str(value.value()).map_err(store_err)
            };
            database_id = read_id("database_id")?.to_string();
            read_id("data_revision")?;
            // Ensure the data tables exist so load() never has to handle
            // missing tables.
            txn.open_table(COLLECTIONS).map_err(store_err)?;
            txn.open_table(DOCUMENTS).map_err(store_err)?;
            txn.open_table(INDEXES).map_err(store_err)?;
            txn.open_table(INDEX_ENTRIES).map_err(store_err)?;
        }
        txn.commit().map_err(store_err)?;

        Ok(Self { db, database_id })
    }

    pub(crate) fn database_id(&self) -> &str {
        &self.database_id
    }

    /// A unique token for this exact commit, including divergent restores
    /// whose database UUID and numeric generation are otherwise identical.
    pub(crate) fn data_revision(&self) -> Result<Uuid> {
        let txn = self.db.begin_read().map_err(store_err)?;
        let identity = txn.open_table(IDENTITY).map_err(store_err)?;
        let value = identity
            .get("data_revision")
            .map_err(store_err)?
            .ok_or_else(|| store_err("missing data_revision"))?;
        Uuid::parse_str(value.value()).map_err(store_err)
    }

    /// Cheap readiness probe for the durable native store.
    ///
    /// Opening the backend validates the schema once, but readiness is a
    /// live question: a later redb/read failure must make `/health/database`
    /// fail instead of inheriting the in-memory backend's unconditional
    /// success. Touch the metadata record and both data tables in one read
    /// transaction without scanning user data.
    pub(crate) fn ping(&self) -> Result<()> {
        let txn = self.db.begin_read().map_err(store_err)?;
        let meta = txn.open_table(META).map_err(store_err)?;
        if meta.get("schema_version").map_err(store_err)?.is_none() {
            return Err(store_err("missing schema version"));
        }
        txn.open_table(COLLECTIONS).map_err(store_err)?;
        txn.open_table(DOCUMENTS).map_err(store_err)?;
        Ok(())
    }

    /// Load the entire database into the in-memory representation.
    #[allow(clippy::type_complexity)]
    pub(crate) fn load(
        &self,
    ) -> Result<(
        HashMap<String, BTreeMap<String, Value>>,
        HashMap<String, CollectionType>,
    )> {
        let txn = self.db.begin_read().map_err(store_err)?;

        let mut collections: HashMap<String, BTreeMap<String, Value>> = HashMap::new();
        let mut collection_types = HashMap::new();

        let types_table = txn.open_table(COLLECTIONS).map_err(store_err)?;
        for entry in types_table.iter().map_err(store_err)? {
            let (key, value) = entry.map_err(store_err)?;
            let name = key.value().to_string();
            collection_types.insert(name.clone(), collection_type_from_u8(value.value())?);
            collections.entry(name).or_default();
        }

        let docs_table = txn.open_table(DOCUMENTS).map_err(store_err)?;
        for entry in docs_table.iter().map_err(store_err)? {
            let (key, value) = entry.map_err(store_err)?;
            let composite = key.value();
            let Some((collection, doc_key)) = composite.split_once(SEP) else {
                return Err(store_err(format!("malformed document key `{composite}`")));
            };
            let doc: Value = serde_json::from_str(value.value()).map_err(store_err)?;
            collections
                .entry(collection.to_string())
                .or_default()
                .insert(doc_key.to_string(), doc);
        }

        Ok((collections, collection_types))
    }

    /// Load only collection types and key sets (no JSON parsing) — the
    /// resident set for paged mode.
    #[allow(clippy::type_complexity)]
    pub(crate) fn load_keys(
        &self,
    ) -> Result<(
        HashMap<String, std::collections::BTreeSet<String>>,
        HashMap<String, CollectionType>,
    )> {
        let txn = self.db.begin_read().map_err(store_err)?;
        let mut keys: HashMap<String, std::collections::BTreeSet<String>> = HashMap::new();
        let mut collection_types = HashMap::new();
        let types_table = txn.open_table(COLLECTIONS).map_err(store_err)?;
        for entry in types_table.iter().map_err(store_err)? {
            let (key, value) = entry.map_err(store_err)?;
            let name = key.value().to_string();
            collection_types.insert(name.clone(), collection_type_from_u8(value.value())?);
            keys.entry(name).or_default();
        }
        let docs_table = txn.open_table(DOCUMENTS).map_err(store_err)?;
        for entry in docs_table.iter().map_err(store_err)? {
            let (key, _) = entry.map_err(store_err)?;
            if let Some((collection, doc_key)) = key.value().split_once(SEP) {
                keys.entry(collection.to_string())
                    .or_default()
                    .insert(doc_key.to_string());
            }
        }
        Ok((keys, collection_types))
    }

    /// Range-scan one collection with limit/offset applied mid-scan.
    pub(crate) fn scan_collection_page(
        &self,
        collection: &str,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<Value>> {
        let txn = self.db.begin_read().map_err(store_err)?;
        let table = txn.open_table(DOCUMENTS).map_err(store_err)?;
        let start = format!("{collection}{SEP}");
        let end = format!("{collection}\u{1}");
        table
            .range(start.as_str()..end.as_str())
            .map_err(store_err)?
            .skip(offset.unwrap_or(0))
            .take(limit.unwrap_or(usize::MAX))
            .map(|entry| {
                let (_, value) = entry.map_err(store_err)?;
                serde_json::from_str(value.value()).map_err(store_err)
            })
            .collect()
    }

    /// Range-scan one collection strictly after a document key. The redb
    /// table is ordered by `collection NUL key`, so this maps directly to a
    /// primary-key cursor without walking the skipped prefix.
    pub(crate) fn scan_collection_after_key(
        &self,
        collection: &str,
        after_key: Option<&str>,
        limit: usize,
    ) -> Result<Vec<Value>> {
        let txn = self.db.begin_read().map_err(store_err)?;
        let table = txn.open_table(DOCUMENTS).map_err(store_err)?;
        let start = after_key.map_or_else(
            || format!("{collection}{SEP}"),
            |key| composite_key(collection, key),
        );
        let end = format!("{collection}\u{1}");
        let lower = if after_key.is_some() {
            Bound::Excluded(start.as_str())
        } else {
            Bound::Included(start.as_str())
        };
        table
            .range::<&str>((lower, Bound::Excluded(end.as_str())))
            .map_err(store_err)?
            .take(limit)
            .map(|entry| {
                let (_, value) = entry.map_err(store_err)?;
                serde_json::from_str(value.value()).map_err(store_err)
            })
            .collect()
    }

    /// Filter one collection while walking the redb range, retaining only the
    /// requested post-filter page. Unlike [`Self::scan_collection`], this
    /// never materializes every document in the collection before applying a
    /// selective predicate or limit.
    pub(crate) fn scan_collection_filtered(
        &self,
        collection: &str,
        predicates: &[FieldPredicate],
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<Value>> {
        let txn = self.db.begin_read().map_err(store_err)?;
        let table = txn.open_table(DOCUMENTS).map_err(store_err)?;
        let start = format!("{collection}{SEP}");
        let end = format!("{collection}\u{1}");
        let offset = offset.unwrap_or(0);
        let limit = limit.unwrap_or(usize::MAX);
        let mut matched = 0usize;
        let mut rows = Vec::new();

        if limit == 0 {
            return Ok(rows);
        }
        for entry in table
            .range(start.as_str()..end.as_str())
            .map_err(store_err)?
        {
            let (_, value) = entry.map_err(store_err)?;
            let document: Value = serde_json::from_str(value.value()).map_err(store_err)?;
            if !predicates
                .iter()
                .all(|predicate| predicate.matches(&document))
            {
                continue;
            }
            if matched < offset {
                matched = matched.saturating_add(1);
                continue;
            }
            rows.push(document);
            if rows.len() == limit {
                break;
            }
        }
        Ok(rows)
    }

    /// Monotonic counter bumped by every committed write batch. Used to
    /// validate derived structures (the vector sidecar) across restarts.
    pub(crate) fn data_generation(&self) -> Result<u32> {
        let txn = self.db.begin_read().map_err(store_err)?;
        let meta = txn.open_table(META).map_err(store_err)?;
        Ok(meta
            .get(DATA_GENERATION_KEY)
            .map_err(store_err)?
            .map(|g| g.value())
            .unwrap_or(0))
    }

    /// Read one document's full JSON (the source of truth, including
    /// fields stripped from memory in sidecar mode).
    pub(crate) fn get_document_raw(&self, collection: &str, key: &str) -> Result<Option<Value>> {
        let txn = self.db.begin_read().map_err(store_err)?;
        let table = txn.open_table(DOCUMENTS).map_err(store_err)?;
        match table
            .get(composite_key(collection, key).as_str())
            .map_err(store_err)?
        {
            Some(guard) => Ok(Some(
                serde_json::from_str(guard.value()).map_err(store_err)?,
            )),
            None => Ok(None),
        }
    }

    /// Iterate one collection's documents (key order) from the truth store.
    pub(crate) fn scan_collection(&self, collection: &str) -> Result<Vec<(String, Value)>> {
        let txn = self.db.begin_read().map_err(store_err)?;
        let table = txn.open_table(DOCUMENTS).map_err(store_err)?;
        let start = format!("{collection}{SEP}");
        let end = format!("{collection}\u{1}");
        table
            .range(start.as_str()..end.as_str())
            .map_err(store_err)?
            .map(|entry| {
                let (key, value) = entry.map_err(store_err)?;
                let doc_key = key
                    .value()
                    .split_once(SEP)
                    .map(|(_, k)| k.to_string())
                    .unwrap_or_default();
                Ok((
                    doc_key,
                    serde_json::from_str(value.value()).map_err(store_err)?,
                ))
            })
            .collect()
    }

    /// Apply a batch of mutations in one committed transaction.
    pub(crate) fn apply(&self, ops: &[StoreOp<'_>]) -> Result<()> {
        let txn = self.db.begin_write().map_err(store_err)?;
        {
            let mut meta = txn.open_table(META).map_err(store_err)?;
            let generation = meta
                .get(DATA_GENERATION_KEY)
                .map_err(store_err)?
                .map(|g| g.value())
                .unwrap_or(0);
            meta.insert(DATA_GENERATION_KEY, generation.wrapping_add(1))
                .map_err(store_err)?;
            txn.open_table(IDENTITY)
                .map_err(store_err)?
                .insert("data_revision", Uuid::new_v4().to_string().as_str())
                .map_err(store_err)?;
        }
        {
            let mut collections = txn.open_table(COLLECTIONS).map_err(store_err)?;
            let mut documents = txn.open_table(DOCUMENTS).map_err(store_err)?;
            let mut indexes = txn.open_table(INDEXES).map_err(store_err)?;
            let mut entries = txn.open_table(INDEX_ENTRIES).map_err(store_err)?;

            for op in ops {
                match op {
                    StoreOp::PutCollection {
                        name,
                        collection_type,
                    } => {
                        check_name(name)?;
                        collections
                            .insert(*name, collection_type_to_u8(*collection_type))
                            .map_err(store_err)?;
                    }
                    StoreOp::DropCollection { name } => {
                        collections.remove(*name).map_err(store_err)?;
                        let prefix = format!("{name}{SEP}");
                        for key in keys_with_prefix(&documents, &prefix)? {
                            documents.remove(key.as_str()).map_err(store_err)?;
                        }
                        for key in keys_with_prefix(&indexes, &prefix)? {
                            indexes.remove(key.as_str()).map_err(store_err)?;
                        }
                        for key in keys_with_prefix(&entries, &prefix)? {
                            entries.remove(key.as_str()).map_err(store_err)?;
                        }
                    }
                    StoreOp::PutDocument {
                        collection,
                        key,
                        doc,
                    } => {
                        check_name(collection)?;
                        check_name(key)?;
                        let json = serde_json::to_string(doc).map_err(store_err)?;
                        documents
                            .insert(composite_key(collection, key).as_str(), json.as_str())
                            .map_err(store_err)?;
                    }
                    StoreOp::DeleteDocument { collection, key } => {
                        documents
                            .remove(composite_key(collection, key).as_str())
                            .map_err(store_err)?;
                    }
                    StoreOp::PutIndex {
                        collection,
                        name,
                        def,
                    } => {
                        check_name(collection)?;
                        check_name(name)?;
                        let json = serde_json::to_string(def).map_err(store_err)?;
                        indexes
                            .insert(composite_key(collection, name).as_str(), json.as_str())
                            .map_err(store_err)?;
                    }
                    StoreOp::DropIndex { collection, name } => {
                        indexes
                            .remove(composite_key(collection, name).as_str())
                            .map_err(store_err)?;
                        let prefix = format!("{collection}{SEP}{name}{SEP}");
                        for key in keys_with_prefix(&entries, &prefix)? {
                            entries.remove(key.as_str()).map_err(store_err)?;
                        }
                    }
                    StoreOp::PutIndexEntry {
                        collection,
                        index,
                        value_key,
                        doc_key,
                    } => {
                        entries
                            .insert(
                                format!("{collection}{SEP}{index}{SEP}{value_key}").as_str(),
                                *doc_key,
                            )
                            .map_err(store_err)?;
                    }
                    StoreOp::DeleteIndexEntry {
                        collection,
                        index,
                        value_key,
                    } => {
                        entries
                            .remove(format!("{collection}{SEP}{index}{SEP}{value_key}").as_str())
                            .map_err(store_err)?;
                    }
                }
            }
        }
        txn.commit().map_err(store_err)?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "storage_identity_tests.rs"]
mod identity_tests;
