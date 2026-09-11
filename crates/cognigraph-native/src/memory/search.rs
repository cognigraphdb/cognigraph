//! Traversal, vector search, and text search bodies for the `GraphBackend`
//! impl — synchronous inherent methods; `backend.rs` delegates to them.

use std::cmp::Ordering;
use std::collections::VecDeque;
use std::sync::atomic::Ordering as AtomicOrdering;

use cognigraph_core::{
    CogniGraphError, Direction, Result, SearchHit, TraversalOpts, TraversalPath, VectorSearchOpts,
};
use rayon::prelude::*;
use serde_json::Value;

use crate::text_index::TextIndex;

use super::helpers::{collection, confidence, cosine_from_values, full_vertex_doc, next_vertex};
use super::{NativeBackend, QuantizedEntry, VectorMode, quantize};

impl NativeBackend {
    pub(super) fn traverse_impl(
        &self,
        start_vertex: &str,
        opts: &TraversalOpts,
    ) -> Result<Vec<TraversalPath>> {
        if self.paged() {
            return self.traverse_paged(start_vertex, opts);
        }
        let state = self.read_state()?;
        let edges = collection(&state, &opts.edge_collection)?;

        // Cached adjacency index (generation-invalidated): traversals are
        // O(degree) per vertex instead of O(E) per call. Confidence and
        // direction filters apply at expansion time.
        let index = self.adjacency_index(&state, &opts.edge_collection)?;
        let empty: Vec<String> = Vec::new();
        let neighbors = |vertex: &str| -> Vec<&Value> {
            let mut keys: Vec<&String> = Vec::new();
            if matches!(opts.direction, Direction::Outbound | Direction::Any) {
                keys.extend(index.from.get(vertex).unwrap_or(&empty));
            }
            if matches!(opts.direction, Direction::Inbound | Direction::Any) {
                keys.extend(index.to.get(vertex).unwrap_or(&empty));
            }
            keys.into_iter()
                .filter_map(|key| edges.get(key))
                .filter(|edge| {
                    !opts
                        .min_confidence
                        .is_some_and(|min_confidence| confidence(edge) < min_confidence)
                })
                .collect()
        };

        let mut paths = Vec::new();
        let mut queue = VecDeque::from([(
            start_vertex.to_string(),
            vec![full_vertex_doc(&state, start_vertex)],
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

            for edge in neighbors(current.as_str()) {
                let Some(next) = next_vertex(edge, &current, opts.direction) else {
                    continue;
                };
                if vertices
                    .iter()
                    .any(|vertex| vertex.get("_id").and_then(Value::as_str) == Some(next.as_str()))
                {
                    continue;
                }
                let mut next_vertices = vertices.clone();
                next_vertices.push(full_vertex_doc(&state, &next));
                let mut next_edges = path_edges.clone();
                next_edges.push((*edge).clone());
                queue.push_back((next, next_vertices, next_edges));
            }
        }

        Ok(paths)
    }

    pub(super) fn vector_search_impl(
        &self,
        collection_name: &str,
        query_vector: &[f64],
        opts: &VectorSearchOpts,
    ) -> Result<Vec<SearchHit>> {
        if self.vector_mode == VectorMode::Sidecar {
            return self.vector_search_sidecar(collection_name, query_vector, opts);
        }
        let state = self.read_state()?;
        let docs = collection(&state, collection_name)?;
        let index = self.quantized_index(&state, collection_name)?;

        // Stage 1: int8 scan — approximate cosine via integer dot products,
        // oversampled 4x (min 32) so quantization error cannot evict true
        // top-k candidates at realistic error rates (<0.5% for int8).
        let query_norm = query_vector.iter().map(|v| v * v).sum::<f64>().sqrt();
        if query_norm == 0.0 || index.entries.is_empty() {
            return Ok(Vec::new());
        }
        let (q_values, q_scale, _) = quantize(query_vector);
        let mut candidates: Vec<(f64, &QuantizedEntry)> = index
            .entries
            .par_iter()
            .filter(|entry| {
                entry.values.len() == query_vector.len()
                    && opts
                        .model_name
                        .as_ref()
                        .is_none_or(|model| entry.model.as_deref() == Some(model.as_str()))
                    && entry.norm > 0.0
            })
            .map(|entry| {
                let dot: i64 = q_values
                    .iter()
                    .zip(&entry.values)
                    .map(|(a, b)| i64::from(*a) * i64::from(*b))
                    .sum();
                let approx = dot as f64 * q_scale * entry.scale / (query_norm * entry.norm);
                (approx, entry)
            })
            .collect();
        candidates.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(Ordering::Equal));
        candidates.truncate((opts.limit * 4).max(32));

        // Stage 2: exact f64 re-rank of the candidates; thresholds and
        // returned scores use exact cosine only.
        let mut scored: Vec<(f64, &Value)> = candidates
            .into_iter()
            .filter_map(|(_, entry)| {
                let doc = docs.get(&entry.key)?;
                let values = doc.get("embedding")?.as_array()?;
                let score = cosine_from_values(query_vector, values)?;
                opts.threshold
                    .is_none_or(|t| score >= t)
                    .then_some((score, doc))
            })
            .collect();
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(Ordering::Equal));
        scored.truncate(opts.limit);
        Ok(scored
            .into_iter()
            .map(|(score, doc)| SearchHit {
                document: doc.clone(),
                score,
                source: Some("native".into()),
            })
            .collect())
    }

    /// BM25 via a lazily built in-RAM tantivy index (rebuilt when the
    /// collection changes — same invalidation as the vector indexes).
    pub(super) fn text_search_impl(
        &self,
        collection_name: &str,
        query: &str,
        fields: &[String],
        limit: usize,
    ) -> Result<Vec<SearchHit>> {
        crate::text_index::validate_text_search_fields(fields)?;
        let cache_key = crate::text_index::index_identity(
            self.store.as_ref().map(|store| store.database_id()),
            collection_name,
            fields,
        );
        if self.paged() {
            let state = self.read_state()?;
            if !state.keys.contains_key(collection_name) {
                return Err(CogniGraphError::CollectionNotFound(
                    collection_name.to_string(),
                ));
            }
            drop(state);
            return self.text_search_paged(collection_name, query, fields, limit);
        }
        let state = self.read_state()?;
        let docs = collection(&state, collection_name)?;
        if docs.is_empty() {
            return Ok(Vec::new());
        }
        // Persistent revisions remain unique across divergent backup restores;
        // in-memory indexes only need the process-local write version.
        let (version, index_dir) = match (&self.store, &self.db_path) {
            (Some(store), Some(db_path)) => {
                let dir = crate::derivative::derivative_path(db_path, &cache_key, "tantivy");
                (store.data_revision()?.to_string(), Some(dir))
            }
            _ => (
                self.write_version.load(AtomicOrdering::Relaxed).to_string(),
                None,
            ),
        };
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
                let warm = index_dir
                    .as_deref()
                    .and_then(|dir| TextIndex::open_if_current(dir, fields, &version, &cache_key));
                let index = std::sync::Arc::new(match warm {
                    Some(index) => index,
                    None => {
                        TextIndex::build(docs, fields, &version, index_dir.as_deref(), &cache_key)?
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
                    document: docs.get(&key)?.clone(),
                    score,
                    source: Some("text".into()),
                })
            })
            .take(limit)
            .collect())
    }
}
