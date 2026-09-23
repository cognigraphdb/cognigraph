//! Write accepted collections into the staging store and resolve edges.

use std::collections::BTreeSet;
use std::path::Path;

use cognigraph_core::{CogniGraphError, CollectionType, GraphBackend, IndexDef, IndexType};
use cognigraph_native::{NativeBackend, StorageMode, VectorMode};
use serde_json::Value;

use super::options::Limits;
use super::records::{self, Budget, Collection};
use super::report::{Constraint, Problem};

/// Documents per store transaction, and a byte ceiling for one chunk.
const CHUNK_DOCUMENTS: usize = 1000;
const CHUNK_BYTES: usize = 8 << 20;
const RESOLVE_PAGE: usize = 1000;
/// Paged storage keeps key sets and this many cached document bytes in RAM.
const CACHE_BYTES: usize = 64 << 20;

/// A contract rejection, or a failure of the importer's own I/O or storage.
#[derive(Debug)]
pub(crate) enum Failure {
    Rejected(Problem),
    Internal(String),
}

impl From<Problem> for Failure {
    fn from(problem: Problem) -> Self {
        Failure::Rejected(problem)
    }
}

fn internal(e: CogniGraphError) -> Failure {
    Failure::Internal(e.to_string())
}

pub(crate) struct Loader {
    pub backend: NativeBackend,
    runtime: tokio::runtime::Runtime,
}

impl Loader {
    pub fn open(store: &Path) -> Result<Self, String> {
        let backend = NativeBackend::open_with_modes(
            store,
            VectorMode::Sidecar,
            StorageMode::Paged,
            CACHE_BYTES,
        )
        .map_err(|e| e.to_string())?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self { backend, runtime })
    }

    fn flush(&self, name: &str, chunk: &mut Vec<(String, Value)>) -> Result<(), Failure> {
        if chunk.is_empty() {
            return Ok(());
        }
        let docs: Vec<Value> = chunk.iter().map(|(_, doc)| doc.clone()).collect();
        if let Err(first) = self.backend.bulk_insert(name, docs) {
            if !matches!(
                first,
                CogniGraphError::UniqueViolation { .. } | CogniGraphError::DocumentConflict(_)
            ) {
                return Err(internal(first));
            }
            // Locate the first offending document in file order; staging is
            // discarded on rejection, so the documents before it may commit.
            for (key, doc) in chunk.iter() {
                match self.backend.bulk_insert(name, vec![doc.clone()]) {
                    Ok(()) => {}
                    Err(CogniGraphError::UniqueViolation { .. }) => {
                        return Err(Problem::new("unique_violation")
                            .collection(name)
                            .key(key)
                            .into());
                    }
                    Err(CogniGraphError::DocumentConflict(_)) => {
                        return Err(Problem::new("duplicate_key")
                            .collection(name)
                            .key(key)
                            .into());
                    }
                    Err(other) => return Err(internal(other)),
                }
            }
            return Err(internal(first));
        }
        chunk.clear();
        Ok(())
    }

    /// Create the collection, declare its carried constraints, stream its
    /// documents in. Returns the number of documents written.
    pub fn load(
        &self,
        directory: &Path,
        files: &[String],
        collection: &Collection<'_>,
        constraints: &[Constraint],
        limits: &Limits,
        budget: &mut Budget,
    ) -> Result<u64, Failure> {
        let kind = if collection.edge {
            CollectionType::Edge
        } else {
            CollectionType::Document
        };
        self.runtime
            .block_on(self.backend.ensure_collection(collection.name, kind))
            .map_err(internal)?;
        for constraint in constraints {
            let definition = IndexDef {
                index_type: IndexType::Persistent,
                fields: constraint.fields.clone(),
                unique: true,
                sparse: constraint.sparse,
                name: Some(constraint.name.clone()),
            };
            self.runtime
                .block_on(self.backend.ensure_index(collection.name, &definition))
                .map_err(internal)?;
        }
        let mut chunk: Vec<(String, Value)> = Vec::new();
        let mut chunk_bytes = 0usize;
        let mut written = 0u64;
        let mut failure: Option<Failure> = None;
        let read = records::read(directory, files, collection, limits, budget, |key, doc| {
            chunk_bytes += key.len() + doc.to_string().len();
            chunk.push((key, doc));
            if chunk.len() >= CHUNK_DOCUMENTS || chunk_bytes >= CHUNK_BYTES {
                let size = chunk.len() as u64;
                if let Err(f) = self.flush(collection.name, &mut chunk) {
                    // Surface an earlier-line storage refusal through the reader.
                    let problem = match &f {
                        Failure::Rejected(p) => p.clone(),
                        Failure::Internal(_) => Problem::new("internal"),
                    };
                    failure = Some(f);
                    return Err(problem);
                }
                written += size;
                chunk_bytes = 0;
            }
            Ok(())
        });
        if let Some(failure) = failure {
            return Err(failure);
        }
        // Pending documents precede the line that failed, so their refusal wins.
        let size = chunk.len() as u64;
        self.flush(collection.name, &mut chunk)?;
        read?;
        Ok(written + size)
    }

    /// Every edge of every loaded edge collection must point at a document of
    /// a loaded collection; edges into a collection that failed are skipped.
    pub fn resolve(
        &self,
        edges: &[String],
        loaded: &BTreeSet<String>,
        failed: &BTreeSet<String>,
    ) -> Result<Vec<Problem>, Failure> {
        let mut problems = Vec::new();
        for name in edges {
            let mut offset = 0;
            loop {
                let page = self
                    .runtime
                    .block_on(
                        self.backend
                            .list_documents(name, Some(RESOLVE_PAGE), Some(offset)),
                    )
                    .map_err(internal)?;
                for edge in &page {
                    for end in ["_from", "_to"] {
                        let target = edge.get(end).and_then(Value::as_str).unwrap_or_default();
                        let (collection, key) = target.split_once('/').unwrap_or((target, ""));
                        if failed.contains(collection) {
                            continue;
                        }
                        let found = loaded.contains(collection)
                            && self
                                .runtime
                                .block_on(self.backend.get_document(collection, key))
                                .map_err(internal)?
                                .is_some();
                        if !found {
                            let key = edge.get("_key").and_then(Value::as_str).unwrap_or_default();
                            problems
                                .push(Problem::new("unresolved_edge").collection(name).key(key));
                            break;
                        }
                    }
                }
                if page.len() < RESOLVE_PAGE {
                    break;
                }
                offset += page.len();
            }
        }
        Ok(problems)
    }
}
