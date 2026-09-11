//! Memory-mapped int8 vector sidecar (storage-model migration step 3).
//!
//! One file per collection, a rebuildable derivative of the redb truth:
//! header (magic, data generation, dim, count, revision UUID) + fixed-width slots of
//! `f32 scale | f32 norm | i8[dim]`. Slot order is the collection's key
//! order; a matching commit revision guarantees the in-memory key list
//! reproduces it, so keys are never stored in the file.

use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use std::sync::atomic::{AtomicU32, Ordering};

use cognigraph_core::{CogniGraphError, Result};
use memmap2::Mmap;
use rayon::prelude::*;
use uuid::Uuid;

const MAGIC: &[u8; 8] = b"CGVEC2\0\0";
const HEADER_LEN: usize = 8 + 4 + 4 + 4 + 16;

fn sidecar_err(e: impl std::fmt::Display) -> CogniGraphError {
    CogniGraphError::BackendError(format!("vector sidecar: {e}"))
}

pub(crate) fn sidecar_path(db_path: &Path, database_id: &str, collection: &str) -> PathBuf {
    let identity =
        serde_json::to_string(&(database_id, collection)).expect("string tuple is serializable");
    crate::derivative::derivative_path(db_path, &identity, "vectors")
}

pub(crate) struct VectorSidecar {
    mmap: Mmap,
    /// Base-file generation; `applied_generation` advances past it as
    /// incremental writes land in the delta.
    pub(crate) generation: u32,
    revision: Uuid,
    applied_generation: AtomicU32,
    pub(crate) dim: usize,
    pub(crate) keys: Vec<String>,
    /// Reconstructed from the same redb revision as `keys`, never persisted in
    /// the vector file. Interning costs four bytes per slot plus model names.
    model_slots: Vec<u32>,
    models: HashMap<String, u32>,
    /// Incremental writes since the base file was built: `Some` = new or
    /// changed vector (shadows the base slot for that key), `None` = the
    /// key is no longer searchable (deleted, or embedding removed).
    delta: RwLock<HashMap<String, Option<DeltaSlot>>>,
}

struct DeltaSlot {
    model: Option<String>,
    scale: f64,
    norm: f64,
    values: Vec<i8>,
}

pub(crate) struct SidecarEntry {
    pub key: String,
    pub vector: Vec<f64>,
    pub model: Option<String>,
}

fn slot_len(dim: usize) -> usize {
    8 + dim
}

impl VectorSidecar {
    /// Write vectors in slot order and map the file. Model metadata stays in
    /// memory and is reconstructed from redb when reopening the same revision.
    pub(crate) fn build(
        path: &Path,
        generation: u32,
        revision: Uuid,
        dim: usize,
        entries: Vec<SidecarEntry>,
    ) -> Result<Self> {
        let mut buf = Vec::with_capacity(HEADER_LEN + entries.len() * slot_len(dim));
        buf.extend_from_slice(MAGIC);
        buf.extend_from_slice(&generation.to_le_bytes());
        buf.extend_from_slice(&(dim as u32).to_le_bytes());
        buf.extend_from_slice(&(entries.len() as u32).to_le_bytes());
        buf.extend_from_slice(revision.as_bytes());
        let mut keys = Vec::with_capacity(entries.len());
        for SidecarEntry { key, vector, model } in &entries {
            if vector.len() != dim {
                return Err(CogniGraphError::ValidationError(format!(
                    "sidecar mode requires a fixed embedding dimension ({dim}); `{key}` has {}",
                    vector.len()
                )));
            }
            let max_abs = vector.iter().fold(0.0f64, |m, v| m.max(v.abs()));
            let scale = if max_abs == 0.0 { 1.0 } else { max_abs / 127.0 };
            let norm = vector.iter().map(|v| v * v).sum::<f64>().sqrt();
            buf.extend_from_slice(&(scale as f32).to_le_bytes());
            buf.extend_from_slice(&(norm as f32).to_le_bytes());
            for v in vector {
                buf.push((v / scale).round().clamp(-127.0, 127.0) as i8 as u8);
            }
            keys.push((key.clone(), model.clone()));
        }
        let tmp = path.with_extension("vectors.tmp");
        {
            let mut file = fs::File::create(&tmp).map_err(sidecar_err)?;
            file.write_all(&buf).map_err(sidecar_err)?;
            file.sync_all().map_err(sidecar_err)?;
        }
        fs::rename(&tmp, path).map_err(sidecar_err)?;
        Self::open(path, keys)
    }

    /// Map an existing file; caller supplies keys and model names in slot order
    /// from the redb revision whose vectors are in this file.
    pub(crate) fn open(path: &Path, keys: Vec<(String, Option<String>)>) -> Result<Self> {
        let file = fs::File::open(path).map_err(sidecar_err)?;
        let mmap = unsafe { Mmap::map(&file) }.map_err(sidecar_err)?;
        if mmap.len() < HEADER_LEN || &mmap[..8] != MAGIC {
            return Err(sidecar_err("bad header"));
        }
        let generation = u32::from_le_bytes(mmap[8..12].try_into().unwrap());
        let dim = u32::from_le_bytes(mmap[12..16].try_into().unwrap()) as usize;
        let count = u32::from_le_bytes(mmap[16..20].try_into().unwrap()) as usize;
        let revision = Uuid::from_slice(&mmap[20..36]).map_err(sidecar_err)?;
        if keys.len() != count || mmap.len() < HEADER_LEN + count * slot_len(dim) {
            return Err(sidecar_err("size mismatch"));
        }
        let mut models = HashMap::new();
        let mut model_slots = Vec::with_capacity(keys.len());
        let keys = keys
            .into_iter()
            .map(|(key, model)| {
                let id = model.map_or(0, |model| {
                    let next = models.len() as u32 + 1;
                    *models.entry(model).or_insert(next)
                });
                model_slots.push(id);
                key
            })
            .collect();
        Ok(Self {
            mmap,
            generation,
            revision,
            applied_generation: AtomicU32::new(generation),
            dim,
            keys,
            model_slots,
            models,
            delta: RwLock::new(HashMap::new()),
        })
    }

    /// Try to reuse an on-disk file if its stamp matches the live data.
    pub(crate) fn open_if_current(
        path: &Path,
        generation: u32,
        revision: Uuid,
        keys: Vec<(String, Option<String>)>,
    ) -> Option<Self> {
        let sidecar = Self::open(path, keys).ok()?;
        (sidecar.generation == generation && sidecar.revision == revision).then_some(sidecar)
    }

    /// The newest data generation this sidecar (base + delta) reflects.
    pub(crate) fn applied_generation(&self) -> u32 {
        self.applied_generation.load(Ordering::Acquire)
    }

    /// True when the delta has grown enough that a base rebuild pays off.
    pub(crate) fn wants_rebuild(&self) -> bool {
        let delta_len = self.delta.read().map(|d| d.len()).unwrap_or(usize::MAX);
        delta_len > (self.keys.len() / 10).max(64)
    }

    /// Record an incremental write: `vector` = Some(new embedding) or
    /// None (key no longer searchable). Advances the applied generation.
    pub(crate) fn apply_delta(
        &self,
        key: &str,
        vector: Option<&[f64]>,
        model: Option<&str>,
        new_generation: u32,
    ) -> Result<()> {
        let slot = match vector {
            Some(vector) => {
                if self.dim != 0 && vector.len() != self.dim {
                    return Err(CogniGraphError::ValidationError(format!(
                        "sidecar mode requires a fixed embedding dimension ({}); got {}",
                        self.dim,
                        vector.len()
                    )));
                }
                let max_abs = vector.iter().fold(0.0f64, |m, v| m.max(v.abs()));
                let scale = if max_abs == 0.0 { 1.0 } else { max_abs / 127.0 };
                Some(DeltaSlot {
                    model: model.map(str::to_owned),
                    scale,
                    norm: vector.iter().map(|v| v * v).sum::<f64>().sqrt(),
                    values: vector
                        .iter()
                        .map(|v| (v / scale).round().clamp(-127.0, 127.0) as i8)
                        .collect(),
                })
            }
            None => None,
        };
        self.delta
            .write()
            .map_err(|_| sidecar_err("delta lock poisoned"))?
            .insert(key.to_string(), slot);
        self.applied_generation
            .store(new_generation, Ordering::Release);
        Ok(())
    }

    /// Stage-1 scan over the mmap'd base (keys shadowed by the delta are
    /// skipped) plus the delta itself. Returns the top `take` candidates
    /// as (approx_score, key), best first. Model filtering applies to both
    /// populations before truncation, including an explicit empty-string model.
    pub(crate) fn scan_top(
        &self,
        query: &[f64],
        take: usize,
        model: Option<&str>,
    ) -> Vec<(f64, String)> {
        if self.dim != 0 && query.len() != self.dim {
            return Vec::new();
        }
        let query_norm = query.iter().map(|v| v * v).sum::<f64>().sqrt();
        if query_norm == 0.0 {
            return Vec::new();
        }
        let delta = match self.delta.read() {
            Ok(delta) => delta,
            Err(_) => return Vec::new(),
        };
        let mut candidates = self.scan_base(query, query_norm, &delta, model);
        // Delta entries (small) scanned serially.
        let max_abs = query.iter().fold(0.0f64, |m, v| m.max(v.abs()));
        let q_scale = if max_abs == 0.0 { 1.0 } else { max_abs / 127.0 };
        let q_values: Vec<i8> = query
            .iter()
            .map(|v| (v / q_scale).round().clamp(-127.0, 127.0) as i8)
            .collect();
        for (key, slot) in delta.iter() {
            let Some(slot) = slot else { continue };
            if model.is_some_and(|model| slot.model.as_deref() != Some(model)) {
                continue;
            }
            if slot.norm == 0.0 || slot.values.len() != q_values.len() {
                continue;
            }
            let dot: i64 = q_values
                .iter()
                .zip(&slot.values)
                .map(|(a, b)| i64::from(*a) * i64::from(*b))
                .sum();
            candidates.push((
                dot as f64 * q_scale * slot.scale / (query_norm * slot.norm),
                key.clone(),
            ));
        }
        candidates.sort_by(|a, b| {
            b.0.partial_cmp(&a.0)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.1.cmp(&b.1))
        });
        candidates.truncate(take);
        candidates
    }

    fn scan_base(
        &self,
        query: &[f64],
        query_norm: f64,
        delta: &HashMap<String, Option<DeltaSlot>>,
        model: Option<&str>,
    ) -> Vec<(f64, String)> {
        let max_abs = query.iter().fold(0.0f64, |m, v| m.max(v.abs()));
        let q_scale = if max_abs == 0.0 { 1.0 } else { max_abs / 127.0 };
        let q_values: Vec<i8> = query
            .iter()
            .map(|v| (v / q_scale).round().clamp(-127.0, 127.0) as i8)
            .collect();
        let record = slot_len(self.dim);
        let model_id = model.and_then(|model| self.models.get(model));
        (0..self.keys.len())
            .into_par_iter()
            .filter_map(|slot| {
                if delta.contains_key(&self.keys[slot]) {
                    return None; // shadowed by an incremental write
                }
                if model.is_some() && model_id != Some(&self.model_slots[slot]) {
                    return None;
                }
                let base = HEADER_LEN + slot * record;
                let bytes = &self.mmap[base..base + record];
                let scale = f32::from_le_bytes(bytes[0..4].try_into().unwrap()) as f64;
                let norm = f32::from_le_bytes(bytes[4..8].try_into().unwrap()) as f64;
                if norm == 0.0 {
                    return None;
                }
                let dot: i64 = q_values
                    .iter()
                    .zip(&bytes[8..])
                    .map(|(a, b)| i64::from(*a) * i64::from(*b as i8))
                    .sum();
                Some((
                    dot as f64 * q_scale * scale / (query_norm * norm),
                    self.keys[slot].clone(),
                ))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warm_open_requires_exact_revision_and_current_format() {
        let dir = std::env::temp_dir().join(format!("cg-vector-revision-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("data.vectors");
        let revision = Uuid::new_v4();
        let keys = vec![("a".to_string(), Some("model-a".to_string()))];
        drop(
            VectorSidecar::build(
                &path,
                7,
                revision,
                2,
                vec![SidecarEntry {
                    key: "a".into(),
                    vector: vec![1.0, 0.0],
                    model: Some("model-a".into()),
                }],
            )
            .unwrap(),
        );
        assert!(VectorSidecar::open_if_current(&path, 7, revision, keys.clone()).is_some());
        assert!(VectorSidecar::open_if_current(&path, 7, Uuid::new_v4(), keys.clone()).is_none());
        assert!(VectorSidecar::open_if_current(&path, 8, revision, keys.clone()).is_none());
        let mut bytes = std::fs::read(&path).unwrap();
        bytes[..8].copy_from_slice(b"CGVEC1\0\0");
        std::fs::write(&path, bytes).unwrap();
        assert!(VectorSidecar::open_if_current(&path, 7, revision, keys).is_none());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
