mod backend;
mod batch;
#[cfg(test)]
mod collection_tests;
mod documents;
mod edges;
mod helpers;
mod indexes;
mod paged;
#[cfg(test)]
mod paged_tests;
mod search;
mod vector;

use std::collections::{BTreeMap, HashMap};
use std::sync::RwLock;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};

use cognigraph_core::{CogniGraphError, CollectionType, GraphBackend, Result};
use serde_json::{Map, Value, json};

use crate::sidecar::VectorSidecar;
use crate::storage::{RedbStore, StoreOp};
use crate::text_index::TextIndex;

use helpers::collection;
use paged::DocCache;

/// Native backend: memory-primary with optional redb durability.
///
/// All reads are served from memory. When opened with a storage path, every
/// write is committed to redb before the in-memory state mutates — see
/// `docs/native-storage-model.md`.
/// Where embeddings live. `Sidecar` (persistent backends only) strips
/// `embedding` from in-memory documents — redb keeps the truth, searches
/// scan a memory-mapped int8 file. See docs/vector-sidecar-design.md.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VectorMode {
    #[default]
    Embedded,
    Sidecar,
}

/// Where documents live at read time. `Paged` keeps only key sets in RAM
/// plus a bytes-bounded LRU cache; documents page in from redb. Requires
/// `VectorMode::Sidecar`. See docs/redb-primary-design.md.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StorageMode {
    #[default]
    Resident,
    Paged,
}

#[derive(Default)]
pub struct NativeBackend {
    /// Commit/publication boundary: writers hold this through derivative
    /// updates; paged point reads hold it through catalog checks and fills.
    /// Lock order is state, then document cache / derivative locks.
    state: RwLock<NativeState>,
    store: Option<RedbStore>,
    vector_mode: VectorMode,
    db_path: Option<std::path::PathBuf>,
    /// Bumped on every write; invalidates the quantized vector indexes.
    write_version: AtomicU64,
    quantized: RwLock<HashMap<String, std::sync::Arc<QuantizedIndex>>>,
    sidecars: RwLock<HashMap<String, std::sync::Arc<VectorSidecar>>>,
    sidecar_rebuilds: AtomicU64,
    /// Lazily built tantivy indexes, keyed by collection + field set.
    text_indexes: RwLock<HashMap<String, std::sync::Arc<TextIndex>>>,
    /// Lazily built adjacency indexes per edge collection.
    adjacency: RwLock<HashMap<String, std::sync::Arc<AdjacencyIndex>>>,
    /// (from, to, relation_type) -> edge key per edge collection: the
    /// upsert lookup in O(1). Unlike the version-stamped derivatives this
    /// uses per-collection invalidation and `upsert_edge` maintains it
    /// across its own writes — an upsert-heavy ingest loop must not
    /// invalidate its own index. Any other write to the collection drops
    /// the entry; the next upsert rebuilds it.
    triples: RwLock<HashMap<String, TripleIndex>>,
    storage_mode: StorageMode,
    doc_cache: Option<std::sync::Mutex<DocCache>>,
    #[cfg(test)]
    paged_pause: std::sync::Mutex<Option<paged_tests::Pause>>,
}

/// (from, to, relation_type) → edge key for one edge collection.
type TripleIndex = HashMap<(String, String, String), String>;

/// Vertex → edge-key lists for one edge collection; a generation-stamped
/// rebuildable derivative like every other index. Traversals become
/// O(degree) instead of O(E) per call.
struct AdjacencyIndex {
    version: u64,
    from: HashMap<String, Vec<String>>,
    to: HashMap<String, Vec<String>>,
}

/// Int8-quantized embeddings for one collection: a two-stage search scans
/// these (4x smaller, integer dot products), then re-ranks the oversampled
/// candidates with exact f64 cosine — returned scores are always exact.
struct QuantizedIndex {
    version: u64,
    entries: Vec<QuantizedEntry>,
}

struct QuantizedEntry {
    key: String,
    model: Option<String>,
    /// Dequantization scale (max |component| / 127).
    scale: f64,
    /// Exact f64 vector norm, precomputed.
    norm: f64,
    values: Vec<i8>,
}

fn quantize(vector: &[f64]) -> (Vec<i8>, f64, f64) {
    let max_abs = vector.iter().fold(0.0f64, |m, v| m.max(v.abs()));
    let scale = if max_abs == 0.0 { 1.0 } else { max_abs / 127.0 };
    let values = vector
        .iter()
        .map(|v| (v / scale).round().clamp(-127.0, 127.0) as i8)
        .collect();
    let norm = vector.iter().map(|v| v * v).sum::<f64>().sqrt();
    (values, scale, norm)
}

#[derive(Debug, Default)]
struct NativeState {
    collections: HashMap<String, BTreeMap<String, Value>>,
    /// Paged mode: resident key sets instead of resident documents.
    keys: HashMap<String, std::collections::BTreeSet<String>>,
    collection_types: HashMap<String, CollectionType>,
    /// Index definitions per collection (CG-86), names resolved.
    indexes: crate::storage_indexes::IndexDefs,
    /// Unique index entries mirrored from redb: collection -> index -> value key -> doc key.
    unique: crate::storage_indexes::UniqueEntries,
}

impl NativeBackend {
    /// Pure in-memory backend (no durability).
    pub fn new() -> Self {
        Self::default()
    }

    /// Persistent backend: creates or opens a redb database at `path` and
    /// loads it into memory.
    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self> {
        Self::open_with_mode(path, VectorMode::Embedded)
    }

    /// Persistent backend with an explicit vector mode. In `Sidecar` mode
    /// in-memory documents (and search results) do not carry `embedding`.
    pub fn open_with_mode(
        path: impl AsRef<std::path::Path>,
        vector_mode: VectorMode,
    ) -> Result<Self> {
        Self::open_with_modes(path, vector_mode, StorageMode::Resident, 256 * 1024 * 1024)
    }

    /// Full-control constructor. Paged storage requires sidecar vectors
    /// (embedded f64 vectors would defeat the paging).
    pub fn open_with_modes(
        path: impl AsRef<std::path::Path>,
        vector_mode: VectorMode,
        storage_mode: StorageMode,
        cache_bytes: usize,
    ) -> Result<Self> {
        if storage_mode == StorageMode::Paged && vector_mode != VectorMode::Sidecar {
            return Err(CogniGraphError::ValidationError(
                "paged storage mode requires COGNIGRAPH_VECTOR_MODE=sidecar".into(),
            ));
        }
        let store = RedbStore::open(path.as_ref())?;
        let (indexes, unique) = store.load_indexes()?;
        let (collections, keys, collection_types) = if storage_mode == StorageMode::Paged {
            let (keys, collection_types) = store.load_keys()?;
            (HashMap::new(), keys, collection_types)
        } else {
            let (mut collections, collection_types) = store.load()?;
            if vector_mode == VectorMode::Sidecar {
                for docs in collections.values_mut() {
                    for doc in docs.values_mut() {
                        if let Value::Object(obj) = doc {
                            obj.remove("embedding");
                        }
                    }
                }
            }
            (collections, HashMap::new(), collection_types)
        };
        Ok(Self {
            state: RwLock::new(NativeState {
                collections,
                keys,
                collection_types,
                indexes,
                unique,
            }),
            storage_mode,
            doc_cache: (storage_mode == StorageMode::Paged)
                .then(|| std::sync::Mutex::new(DocCache::new(cache_bytes))),
            #[cfg(test)]
            paged_pause: Default::default(),
            store: Some(store),
            vector_mode,
            db_path: Some(path.as_ref().to_path_buf()),
            write_version: AtomicU64::new(0),
            quantized: RwLock::new(HashMap::new()),
            sidecars: RwLock::new(HashMap::new()),
            sidecar_rebuilds: AtomicU64::new(0),
            text_indexes: RwLock::new(HashMap::new()),
            adjacency: RwLock::new(HashMap::new()),
            triples: RwLock::new(HashMap::new()),
        })
    }

    fn adjacency_index(
        &self,
        state: &NativeState,
        edge_collection: &str,
    ) -> Result<std::sync::Arc<AdjacencyIndex>> {
        let version = self.write_version.load(AtomicOrdering::Relaxed);
        if let Some(index) = self
            .adjacency
            .read()
            .map_err(|_| CogniGraphError::BackendError("adjacency lock poisoned".into()))?
            .get(edge_collection)
            .filter(|index| index.version == version)
        {
            return Ok(index.clone());
        }
        let edges = collection(state, edge_collection)?;
        let mut from: HashMap<String, Vec<String>> = HashMap::new();
        let mut to: HashMap<String, Vec<String>> = HashMap::new();
        for (key, edge) in edges {
            if let Some(v) = edge.get("_from").and_then(Value::as_str) {
                from.entry(v.to_string()).or_default().push(key.clone());
            }
            if let Some(v) = edge.get("_to").and_then(Value::as_str) {
                to.entry(v.to_string()).or_default().push(key.clone());
            }
        }
        let index = std::sync::Arc::new(AdjacencyIndex { version, from, to });
        self.adjacency
            .write()
            .map_err(|_| CogniGraphError::BackendError("adjacency lock poisoned".into()))?
            .insert(edge_collection.to_string(), index.clone());
        Ok(index)
    }

    /// Adjacency for an edge collection in EITHER storage mode.
    ///
    /// Traversal already had this index (built per mode, in two places); the
    /// filtered-listing path did not, so `FILTER e._to == x` cost a full
    /// collection scan while the equivalent traversal was ~16x faster on the
    /// same rows. Both now come through here
    /// (decision_cgql_v2_workload_gaps.md, D1).
    fn edge_adjacency(&self, edge_collection: &str) -> Result<std::sync::Arc<AdjacencyIndex>> {
        if !self.paged() {
            let state = self.read_state()?;
            return self.adjacency_index(&state, edge_collection);
        }
        {
            let state = self.read_state()?;
            if !state.keys.contains_key(edge_collection) {
                return Err(CogniGraphError::CollectionNotFound(
                    edge_collection.to_string(),
                ));
            }
        }
        let store = self.require_store()?;
        let version = u64::from(store.data_generation()?);
        if let Some(index) = self
            .adjacency
            .read()
            .map_err(|_| CogniGraphError::BackendError("adjacency lock poisoned".into()))?
            .get(edge_collection)
            .filter(|index| index.version == version)
        {
            return Ok(index.clone());
        }
        let mut from: HashMap<String, Vec<String>> = HashMap::new();
        let mut to: HashMap<String, Vec<String>> = HashMap::new();
        for (key, edge) in store.scan_collection(edge_collection)? {
            if let Some(v) = edge.get("_from").and_then(Value::as_str) {
                from.entry(v.to_string()).or_default().push(key.clone());
            }
            if let Some(v) = edge.get("_to").and_then(Value::as_str) {
                to.entry(v.to_string()).or_default().push(key);
            }
        }
        let index = std::sync::Arc::new(AdjacencyIndex { version, from, to });
        self.adjacency
            .write()
            .map_err(|_| CogniGraphError::BackendError("adjacency lock poisoned".into()))?
            .insert(edge_collection.to_string(), index.clone());
        Ok(index)
    }

    fn paged(&self) -> bool {
        self.storage_mode == StorageMode::Paged
    }

    fn require_store(&self) -> Result<&RedbStore> {
        self.store
            .as_ref()
            .ok_or_else(|| CogniGraphError::BackendError("paged mode requires storage".into()))
    }

    /// Snapshot the entire backend as a JSON document (see
    /// `docs/native-storage-model.md` for the shape).
    pub async fn export_json(&self) -> Result<Value> {
        if (self.vector_mode == VectorMode::Sidecar || self.paged())
            && let Some(store) = &self.store
        {
            // Export the redb truth so embeddings survive the roundtrip.
            let state = self.read_state()?;
            let names: Vec<String> = if self.paged() {
                state.keys.keys().cloned().collect()
            } else {
                state.collections.keys().cloned().collect()
            };
            let mut collections = Map::new();
            for name in &names {
                let collection_type = match state.collection_types.get(name) {
                    Some(CollectionType::Edge) => "edge",
                    _ => "document",
                };
                let documents: Map<String, Value> =
                    store.scan_collection(name)?.into_iter().collect();
                collections.insert(
                    name.clone(),
                    Self::snapshot_entry(&state, name, collection_type, documents),
                );
            }
            return Ok(json!({ "collections": collections }));
        }
        let state = self.read_state()?;
        let mut collections = Map::new();
        for (name, docs) in &state.collections {
            let collection_type = match state.collection_types.get(name) {
                Some(CollectionType::Edge) => "edge",
                _ => "document",
            };
            let documents: Map<String, Value> =
                docs.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
            collections.insert(
                name.clone(),
                Self::snapshot_entry(&state, name, collection_type, documents),
            );
        }
        Ok(json!({ "collections": collections }))
    }

    /// One snapshot collection; `indexes` is present only when declared.
    fn snapshot_entry(
        state: &NativeState,
        name: &str,
        collection_type: &str,
        documents: Map<String, Value>,
    ) -> Value {
        let mut entry = json!({ "type": collection_type, "documents": documents });
        if let Some(defs) = state.indexes.get(name)
            && !defs.is_empty()
        {
            entry["indexes"] = json!(defs);
        }
        entry
    }

    /// Import a snapshot produced by `export_json`, creating collections and
    /// overwriting documents with the same keys.
    pub async fn import_json(&self, data: &Value) -> Result<()> {
        self.triples_clear();
        let Some(collections) = data.get("collections").and_then(Value::as_object) else {
            return Err(CogniGraphError::ValidationError(
                "import payload must contain a `collections` object".into(),
            ));
        };
        for (name, entry) in collections {
            let collection_type = match entry.get("type").and_then(Value::as_str) {
                Some("edge") => CollectionType::Edge,
                _ => CollectionType::Document,
            };
            self.ensure_collection(name, collection_type).await?;
            if let Some(defs) = entry.get("indexes").and_then(Value::as_array) {
                for def in defs {
                    let def: cognigraph_core::IndexDef = serde_json::from_value(def.clone())
                        .map_err(|e| {
                            CogniGraphError::ValidationError(format!(
                                "snapshot index on `{name}` is invalid: {e}"
                            ))
                        })?;
                    self.ensure_index_impl(name, &def)?;
                }
            }
            let Some(docs) = entry.get("documents").and_then(Value::as_object) else {
                continue;
            };
            let mut state = self.write_state()?;
            for (key, doc) in docs {
                let previous = self.stored_document(&state, name, key)?;
                let removed = previous
                    .as_ref()
                    .map(|old| indexes::entries_of(&state, name, old))
                    .unwrap_or_default();
                let added = indexes::check(&state, &Default::default(), name, key, doc)?;
                let mut ops = vec![StoreOp::PutDocument {
                    collection: name,
                    key,
                    doc,
                }];
                ops.extend(indexes::ops(name, key, &removed, &added));
                self.persist(&ops)?;
                indexes::apply(&mut state, name, key, &removed, &added);
                if self.paged() {
                    state
                        .keys
                        .entry(name.clone())
                        .or_default()
                        .insert(key.clone());
                    self.cache_put(name, key, &self.strip_embedding_if_sidecar(doc.clone()));
                } else {
                    state
                        .collections
                        .entry(name.clone())
                        .or_default()
                        .insert(key.clone(), self.strip_embedding_if_sidecar(doc.clone()));
                }
                self.sidecar_apply_write(name, key, Some(doc));
            }
        }
        Ok(())
    }

    fn triples_invalidate(&self, collection: &str) {
        if let Ok(mut triples) = self.triples.write() {
            triples.remove(collection);
        }
    }

    fn triples_clear(&self) {
        if let Ok(mut triples) = self.triples.write() {
            triples.clear();
        }
    }

    /// The edge key for (from, to, relation_type), building the collection's
    /// triple index on demand. Must be called with the state write lock held
    /// (every writer takes it, so the index cannot go stale mid-flight).
    fn triple_key(
        &self,
        state: &NativeState,
        collection_name: &str,
        from: &str,
        to: &str,
        relation_type: &str,
    ) -> Result<Option<String>> {
        let triple = (from.to_string(), to.to_string(), relation_type.to_string());
        let mut triples = self
            .triples
            .write()
            .map_err(|_| CogniGraphError::BackendError("triple index lock poisoned".into()))?;
        if let Some(index) = triples.get(collection_name) {
            return Ok(index.get(&triple).cloned());
        }
        let mut index: TripleIndex = HashMap::new();
        let mut add = |key: &str, edge: &Value| {
            let (Some(f), Some(t)) = (
                edge.get("_from").and_then(Value::as_str),
                edge.get("_to").and_then(Value::as_str),
            ) else {
                return;
            };
            let r = edge
                .get("relation_type")
                .and_then(Value::as_str)
                .unwrap_or_default();
            // First occurrence wins, matching the old linear scan order.
            index
                .entry((f.to_string(), t.to_string(), r.to_string()))
                .or_insert_with(|| key.to_string());
        };
        if self.paged() {
            if let Some(store) = &self.store {
                for (key, edge) in store.scan_collection(collection_name).unwrap_or_default() {
                    add(&key, &edge);
                }
            }
        } else if let Some(edges) = state.collections.get(collection_name) {
            for (key, edge) in edges {
                add(key, edge);
            }
        }
        let found = index.get(&triple).cloned();
        triples.insert(collection_name.to_string(), index);
        Ok(found)
    }

    /// Record a just-written edge in the collection's triple index (no-op
    /// when the index was never built). State write lock must be held.
    fn triple_record(
        &self,
        collection_name: &str,
        from: &str,
        to: &str,
        relation_type: &str,
        key: &str,
    ) {
        if let Ok(mut triples) = self.triples.write()
            && let Some(index) = triples.get_mut(collection_name)
        {
            index
                .entry((from.to_string(), to.to_string(), relation_type.to_string()))
                .or_insert_with(|| key.to_string());
        }
    }

    /// The document as currently stored, for index bookkeeping on overwrite.
    fn stored_document(
        &self,
        state: &NativeState,
        collection: &str,
        key: &str,
    ) -> Result<Option<Value>> {
        if self.paged() || self.vector_mode == VectorMode::Sidecar {
            let known = state
                .keys
                .get(collection)
                .is_some_and(|keys| keys.contains(key))
                || state
                    .collections
                    .get(collection)
                    .is_some_and(|docs| docs.contains_key(key));
            return match (&self.store, known) {
                (Some(store), true) => store.get_document_raw(collection, key),
                _ => Ok(None),
            };
        }
        Ok(state
            .collections
            .get(collection)
            .and_then(|docs| docs.get(key))
            .cloned())
    }

    /// Write-through: commit to durable storage (when present) before the
    /// caller mutates memory. Called on every write path, so it also
    /// invalidates the quantized vector indexes.
    fn persist(&self, ops: &[StoreOp<'_>]) -> Result<()> {
        self.write_version.fetch_add(1, AtomicOrdering::Relaxed);
        match &self.store {
            Some(store) => store.apply(ops),
            None => Ok(()),
        }
    }

    fn read_state(&self) -> Result<std::sync::RwLockReadGuard<'_, NativeState>> {
        self.state
            .read()
            .map_err(|_| CogniGraphError::BackendError("native backend lock poisoned".into()))
    }

    fn write_state(&self) -> Result<std::sync::RwLockWriteGuard<'_, NativeState>> {
        self.state
            .write()
            .map_err(|_| CogniGraphError::BackendError("native backend lock poisoned".into()))
    }
}
