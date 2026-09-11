use std::cmp::Ordering;
use std::sync::atomic::Ordering as AtomicOrdering;

use cognigraph_core::{CogniGraphError, Result, SearchHit, VectorSearchOpts};
use serde_json::Value;

use crate::sidecar::{SidecarEntry, VectorSidecar, sidecar_path};

use super::helpers::{collection, cosine_from_values};
use super::{NativeBackend, NativeState, QuantizedEntry, QuantizedIndex, VectorMode, quantize};

impl NativeBackend {
    /// Sidecar-mode search: mmap i8 stage-1 scan, exact re-rank from the
    /// redb truth. Returned documents are the in-memory (stripped) docs.
    pub(super) fn vector_search_sidecar(
        &self,
        collection_name: &str,
        query_vector: &[f64],
        opts: &VectorSearchOpts,
    ) -> Result<Vec<SearchHit>> {
        // A rebuilt sidecar must include every write before its generation
        // stamp. Keep snapshot/build/publication ordered with writers; an old
        // build must not replace a sidecar that already absorbed a new delta.
        let state = self.read_state()?;
        let store = self
            .store
            .as_ref()
            .ok_or_else(|| CogniGraphError::BackendError("sidecar mode requires storage".into()))?;
        let db_path = self
            .db_path
            .as_ref()
            .expect("persistent backend has a path");
        let generation = store.data_generation()?;

        let cached = self
            .sidecars
            .read()
            .map_err(|_| CogniGraphError::BackendError("sidecar lock poisoned".into()))?
            .get(collection_name)
            .filter(|s| s.applied_generation() == generation && !s.wants_rebuild())
            .cloned();
        let sidecar = match cached {
            Some(sidecar) => sidecar,
            None => {
                // Serialize cold builds and recheck after waiting. Otherwise
                // concurrent readers can share the same temporary file and
                // replace each other's freshly published sidecars.
                let mut sidecars = self
                    .sidecars
                    .write()
                    .map_err(|_| CogniGraphError::BackendError("sidecar lock poisoned".into()))?;
                if let Some(sidecar) = sidecars
                    .get(collection_name)
                    .filter(|s| s.applied_generation() == generation && !s.wants_rebuild())
                {
                    sidecar.clone()
                } else {
                    let entries: Vec<SidecarEntry> = store
                        .scan_collection(collection_name)?
                        .into_iter()
                        .filter_map(|(key, doc)| {
                            let vector: Vec<f64> = doc
                                .get("embedding")?
                                .as_array()?
                                .iter()
                                .map(Value::as_f64)
                                .collect::<Option<_>>()?;
                            Some(SidecarEntry {
                                key,
                                vector,
                                model: doc
                                    .get("model_name")
                                    .and_then(Value::as_str)
                                    .map(str::to_owned),
                            })
                        })
                        .collect();
                    let dim = entries.first().map(|entry| entry.vector.len()).unwrap_or(0);
                    let path = sidecar_path(db_path, store.database_id(), collection_name);
                    let revision = store.data_revision()?;
                    let keys = entries
                        .iter()
                        .map(|entry| (entry.key.clone(), entry.model.clone()))
                        .collect();
                    let sidecar =
                        match VectorSidecar::open_if_current(&path, generation, revision, keys) {
                            Some(existing) => existing,
                            None => {
                                self.sidecar_rebuilds.fetch_add(1, AtomicOrdering::Relaxed);
                                VectorSidecar::build(&path, generation, revision, dim, entries)?
                            }
                        };
                    let sidecar = std::sync::Arc::new(sidecar);
                    #[cfg(test)]
                    self.pause_paged_test(super::paged_tests::PausePoint::SidecarPublication);
                    sidecars.insert(collection_name.to_string(), sidecar.clone());
                    sidecar
                }
            }
        };

        // Candidate resolution takes its own read locks. Never recursively
        // acquire the state lock while a writer could be waiting for it.
        drop(state);
        let candidates = sidecar.scan_top(
            query_vector,
            (opts.limit * 4).max(32),
            opts.model_name.as_deref(),
        );

        if self.paged() {
            let mut scored: Vec<(f64, Value)> = candidates
                .into_iter()
                .filter_map(|(_, key)| {
                    let doc = self.fetch_paged(collection_name, &key).ok().flatten()?;
                    if !opts.model_name.as_ref().is_none_or(|model| {
                        doc.get("model_name").and_then(Value::as_str) == Some(model.as_str())
                    }) {
                        return None;
                    }
                    let truth = store.get_document_raw(collection_name, &key).ok()??;
                    let values = truth.get("embedding")?.as_array()?.to_vec();
                    let score = cosine_from_values(query_vector, &values)?;
                    opts.threshold
                        .is_none_or(|t| score >= t)
                        .then_some((score, doc))
                })
                .collect();
            scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(Ordering::Equal));
            scored.truncate(opts.limit);
            return Ok(scored
                .into_iter()
                .map(|(score, document)| SearchHit {
                    document,
                    score,
                    source: Some("native".into()),
                })
                .collect());
        }
        let state = self.read_state()?;
        let docs = collection(&state, collection_name)?;
        let mut scored: Vec<(f64, &Value)> = candidates
            .into_iter()
            .filter_map(|(_, key)| {
                let doc = docs.get(&key)?;
                if !opts.model_name.as_ref().is_none_or(|model| {
                    doc.get("model_name").and_then(Value::as_str) == Some(model.as_str())
                }) {
                    return None;
                }
                let truth = store.get_document_raw(collection_name, &key).ok()??;
                let values = truth.get("embedding")?.as_array()?.to_vec();
                let score = cosine_from_values(query_vector, &values)?;
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

    /// Number of full sidecar file (re)builds — diagnostics; incremental
    /// deltas keep this flat under steady writes.
    pub fn sidecar_rebuild_count(&self) -> u64 {
        self.sidecar_rebuilds.load(AtomicOrdering::Relaxed)
    }

    /// Incremental sidecar maintenance: after a document write, feed the
    /// (pre-strip) embedding into the cached sidecar's delta so the next
    /// search needs no rebuild. No cached sidecar → nothing to do (the
    /// next search rebuilds from redb truth).
    /// Caller must hold the state write lock from commit through publication.
    pub(super) fn sidecar_apply_write(
        &self,
        collection_name: &str,
        key: &str,
        doc: Option<&Value>,
    ) {
        if self.vector_mode != VectorMode::Sidecar {
            return;
        }
        let Ok(guard) = self.sidecars.read() else {
            return;
        };
        let Some(sidecar) = guard.get(collection_name) else {
            return;
        };
        let generation = match self.store.as_ref().map(|s| s.data_generation()) {
            Some(Ok(generation)) => generation,
            _ => return,
        };
        let vector: Option<Vec<f64>> = doc
            .and_then(|d| d.get("embedding"))
            .and_then(Value::as_array)
            .and_then(|a| a.iter().map(Value::as_f64).collect());
        let model = doc
            .and_then(|doc| doc.get("model_name"))
            .and_then(Value::as_str);
        if let Err(e) = sidecar.apply_delta(key, vector.as_deref(), model, generation) {
            tracing::warn!(error = %e, collection = collection_name, key, "sidecar delta rejected");
        }
    }

    /// Get or (re)build the quantized index for a collection.
    pub(super) fn quantized_index(
        &self,
        state: &NativeState,
        collection_name: &str,
    ) -> Result<std::sync::Arc<QuantizedIndex>> {
        let version = self.write_version.load(AtomicOrdering::Relaxed);
        if let Some(index) = self
            .quantized
            .read()
            .map_err(|_| CogniGraphError::BackendError("quantized index lock poisoned".into()))?
            .get(collection_name)
            && index.version == version
        {
            return Ok(index.clone());
        }
        let docs = collection(state, collection_name)?;
        let entries: Vec<QuantizedEntry> = docs
            .iter()
            .filter_map(|(key, doc)| {
                let values = doc.get("embedding")?.as_array()?;
                let vector: Vec<f64> = values.iter().map(Value::as_f64).collect::<Option<_>>()?;
                let (values, scale, norm) = quantize(&vector);
                Some(QuantizedEntry {
                    key: key.clone(),
                    model: doc
                        .get("model_name")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    scale,
                    norm,
                    values,
                })
            })
            .collect();
        let index = std::sync::Arc::new(QuantizedIndex { version, entries });
        self.quantized
            .write()
            .map_err(|_| CogniGraphError::BackendError("quantized index lock poisoned".into()))?
            .insert(collection_name.to_string(), index.clone());
        Ok(index)
    }
}
