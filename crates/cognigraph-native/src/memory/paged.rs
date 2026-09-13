use std::collections::{BTreeMap, HashMap, VecDeque};

use cognigraph_core::{
    CogniGraphError, Direction, Result, SearchHit, TraversalOpts, TraversalPath,
};
use serde_json::{Value, json};

use crate::text_index::TextIndex;

use super::helpers::{confidence, next_vertex};
use super::{AdjacencyIndex, NativeBackend, VectorMode};

/// Bytes-bounded LRU cache for paged mode.
pub(super) struct DocCache {
    map: HashMap<(String, String), (Value, usize)>,
    order: VecDeque<(String, String)>,
    bytes: usize,
    cap: usize,
}

impl DocCache {
    pub(super) fn new(cap: usize) -> Self {
        Self {
            map: HashMap::new(),
            order: VecDeque::new(),
            bytes: 0,
            cap,
        }
    }

    fn get(&mut self, collection: &str, key: &str) -> Option<Value> {
        let entry = (collection.to_string(), key.to_string());
        let value = self.map.get(&entry)?.0.clone();
        // Move to back (recently used).
        if let Some(position) = self.order.iter().position(|e| *e == entry) {
            self.order.remove(position);
            self.order.push_back(entry);
        }
        Some(value)
    }

    fn put(&mut self, collection: &str, key: &str, value: Value, size: usize) {
        let entry = (collection.to_string(), key.to_string());
        self.remove(collection, key);
        self.map.insert(entry.clone(), (value, size));
        self.order.push_back(entry);
        self.bytes += size;
        while self.bytes > self.cap {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            if let Some((_, size)) = self.map.remove(&oldest) {
                self.bytes -= size;
            }
        }
    }

    fn remove(&mut self, collection: &str, key: &str) {
        let entry = (collection.to_string(), key.to_string());
        if let Some((_, size)) = self.map.remove(&entry) {
            self.bytes -= size;
            if let Some(position) = self.order.iter().position(|e| *e == entry) {
                self.order.remove(position);
            }
        }
    }

    fn remove_collection(&mut self, collection: &str) {
        self.map.retain(|(name, _), (_, size)| {
            if name == collection {
                self.bytes -= *size;
                false
            } else {
                true
            }
        });
        self.order.retain(|(name, _)| name != collection);
    }
}

impl NativeBackend {
    pub(super) fn text_search_paged(
        &self,
        collection_name: &str,
        query: &str,
        fields: &[String],
        limit: usize,
    ) -> Result<Vec<SearchHit>> {
        let store = self.require_store()?;
        let version = store.data_revision()?.to_string();
        let cache_key =
            crate::text_index::index_identity(Some(store.database_id()), collection_name, fields);
        let cached = self
            .text_indexes
            .read()
            .map_err(|_| CogniGraphError::BackendError("text index lock poisoned".into()))?
            .get(&cache_key)
            .filter(|index| index.version == version)
            .cloned();
        let index = match cached {
            Some(index) => index,
            None => {
                let db_path = self.db_path.as_ref().expect("paged backend has a path");
                let dir = crate::derivative::derivative_path(db_path, &cache_key, "tantivy");
                let warm = TextIndex::open_if_current(&dir, fields, &version, &cache_key);
                let index = std::sync::Arc::new(match warm {
                    Some(index) => index,
                    None => {
                        let docs: BTreeMap<String, Value> = store
                            .scan_collection(collection_name)?
                            .into_iter()
                            .collect();
                        TextIndex::build(&docs, fields, &version, Some(&dir), &cache_key)?
                    }
                });
                self.text_indexes
                    .write()
                    .map_err(|_| CogniGraphError::BackendError("text index lock poisoned".into()))?
                    .insert(cache_key, index.clone());
                index
            }
        };
        Ok(index
            .search(query, limit)?
            .into_iter()
            .filter_map(|(score, key)| {
                Some(SearchHit {
                    document: self.fetch_paged(collection_name, &key).ok().flatten()?,
                    score,
                    source: Some("text".into()),
                })
            })
            .take(limit)
            .collect())
    }

    /// Paged traversal: adjacency built from a redb scan (generation-
    /// stamped), vertex/edge documents resolved through the LRU cache.
    pub(super) fn traverse_paged(
        &self,
        start_vertex: &str,
        opts: &TraversalOpts,
    ) -> Result<Vec<TraversalPath>> {
        {
            let state = self.read_state()?;
            if !state.keys.contains_key(&opts.edge_collection) {
                return Err(CogniGraphError::CollectionNotFound(
                    opts.edge_collection.clone(),
                ));
            }
        }
        let store = self.require_store()?;
        let generation = u64::from(store.data_generation()?);
        let cached = self
            .adjacency
            .read()
            .map_err(|_| CogniGraphError::BackendError("adjacency lock poisoned".into()))?
            .get(&opts.edge_collection)
            .filter(|index| index.version == generation)
            .cloned();
        let index = match cached {
            Some(index) => index,
            None => {
                let mut from: HashMap<String, Vec<String>> = HashMap::new();
                let mut to: HashMap<String, Vec<String>> = HashMap::new();
                for (key, edge) in store.scan_collection(&opts.edge_collection)? {
                    if let Some(v) = edge.get("_from").and_then(Value::as_str) {
                        from.entry(v.to_string()).or_default().push(key.clone());
                    }
                    if let Some(v) = edge.get("_to").and_then(Value::as_str) {
                        to.entry(v.to_string()).or_default().push(key);
                    }
                }
                let index = std::sync::Arc::new(AdjacencyIndex {
                    version: generation,
                    from,
                    to,
                });
                self.adjacency
                    .write()
                    .map_err(|_| CogniGraphError::BackendError("adjacency lock poisoned".into()))?
                    .insert(opts.edge_collection.clone(), index.clone());
                index
            }
        };

        let fetch_vertex = |vertex_id: &str| -> Value {
            vertex_id
                .split_once('/')
                .and_then(|(collection, key)| self.fetch_paged(collection, key).ok().flatten())
                .unwrap_or_else(|| json!({ "_id": vertex_id }))
        };
        let empty: Vec<String> = Vec::new();
        let mut paths = Vec::new();
        let mut queue = VecDeque::from([(
            start_vertex.to_string(),
            vec![fetch_vertex(start_vertex)],
            Vec::<Value>::new(),
        )]);
        while let Some((current, vertices, path_edges)) = queue.pop_front() {
            let depth = path_edges.len();
            if depth >= opts.min_depth as usize && depth <= opts.max_depth as usize {
                paths.push(TraversalPath {
                    vertices: vertices.clone(),
                    edges: path_edges.clone(),
                    depth,
                    score: path_edges.iter().map(confidence).product::<f64>()
                        * opts.path_decay.powi(depth as i32),
                });
            }
            if depth >= opts.max_depth as usize {
                continue;
            }
            let mut edge_keys: Vec<&String> = Vec::new();
            if matches!(opts.direction, Direction::Outbound | Direction::Any) {
                edge_keys.extend(index.from.get(&current).unwrap_or(&empty));
            }
            if matches!(opts.direction, Direction::Inbound | Direction::Any) {
                edge_keys.extend(index.to.get(&current).unwrap_or(&empty));
            }
            for edge_key in edge_keys {
                let Some(edge) = self.fetch_paged(&opts.edge_collection, edge_key)? else {
                    continue;
                };
                if opts
                    .min_confidence
                    .is_some_and(|min_confidence| confidence(&edge) < min_confidence)
                {
                    continue;
                }
                let Some(next) = next_vertex(&edge, &current, opts.direction) else {
                    continue;
                };
                if vertices
                    .iter()
                    .any(|vertex| vertex.get("_id").and_then(Value::as_str) == Some(next.as_str()))
                {
                    continue;
                }
                let mut next_vertices = vertices.clone();
                next_vertices.push(fetch_vertex(&next));
                let mut next_edges = path_edges.clone();
                next_edges.push(edge);
                queue.push_back((next, next_vertices, next_edges));
            }
        }
        Ok(paths)
    }

    /// Read one document in paged mode: LRU cache, then redb (embedding
    /// stripped — paged implies sidecar).
    pub(super) fn fetch_paged(&self, collection_name: &str, key: &str) -> Result<Option<Value>> {
        // Keep catalog validation, the redb snapshot, and any cache fill in
        // one read section. Writers retain the exclusive lock through cache
        // and sidecar publication, so old fills cannot outlive writes/drops.
        let state = self.read_state()?;
        if !state
            .keys
            .get(collection_name)
            .is_some_and(|keys| keys.contains(key))
        {
            return Ok(None);
        }
        if let Some(cache) = &self.doc_cache
            && let Ok(mut cache) = cache.lock()
            && let Some(doc) = cache.get(collection_name, key)
        {
            return Ok(Some(doc));
        }
        let Some(doc) = self
            .require_store()?
            .get_document_raw(collection_name, key)?
        else {
            return Ok(None);
        };
        let doc = self.strip_embedding_if_sidecar(doc);
        #[cfg(test)]
        self.pause_paged_test(super::paged_tests::PausePoint::ReadFill);
        let size = doc.to_string().len();
        if let Some(cache) = &self.doc_cache
            && let Ok(mut cache) = cache.lock()
        {
            cache.put(collection_name, key, doc.clone(), size);
        }
        Ok(Some(doc))
    }

    pub(super) fn cache_put(&self, collection_name: &str, key: &str, doc: &Value) {
        #[cfg(test)]
        self.pause_paged_test(super::paged_tests::PausePoint::WritePublication);
        if let Some(cache) = &self.doc_cache
            && let Ok(mut cache) = cache.lock()
        {
            let size = doc.to_string().len();
            cache.put(collection_name, key, doc.clone(), size);
        }
    }

    pub(super) fn cache_remove(&self, collection_name: &str, key: &str) {
        #[cfg(test)]
        self.pause_paged_test(super::paged_tests::PausePoint::DeletePublication);
        if let Some(cache) = &self.doc_cache
            && let Ok(mut cache) = cache.lock()
        {
            cache.remove(collection_name, key);
        }
    }

    /// Caller holds the state write lock through durable drop and cleanup.
    pub(super) fn cache_remove_collection(&self, collection_name: &str) {
        if let Some(cache) = &self.doc_cache
            && let Ok(mut cache) = cache.lock()
        {
            cache.remove_collection(collection_name);
        }
    }

    pub(super) fn strip_embedding_if_sidecar(&self, mut doc: Value) -> Value {
        if self.vector_mode == VectorMode::Sidecar
            && let Value::Object(obj) = &mut doc
        {
            obj.remove("embedding");
        }
        doc
    }
}

#[cfg(test)]
mod tests {
    use super::DocCache;
    use serde_json::json;

    #[test]
    fn collection_purge_preserves_other_entries_and_lru_accounting() {
        let mut cache = DocCache::new(30);
        cache.put("drop", "a", json!(1), 10);
        cache.put("keep", "a", json!(2), 10);
        cache.put("drop", "b", json!(3), 10);
        cache.remove_collection("drop");
        cache.remove_collection("drop");
        assert_eq!(cache.bytes, 10);
        assert_eq!(cache.order.len(), 1);
        assert_eq!(cache.map.len(), 1);
        assert_eq!(cache.get("keep", "a"), Some(json!(2)));
        assert_eq!(cache.get("drop", "a"), None);
        cache.put("new", "b", json!(4), 25);
        assert_eq!(cache.bytes, 25);
        assert_eq!(cache.get("keep", "a"), None);
        assert_eq!(cache.get("new", "b"), Some(json!(4)));
    }
}
