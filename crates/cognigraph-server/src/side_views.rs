//! Side-view lifecycle shared by public deletes and the generating job.
//!
//! Short, tenant/incarnation-scoped critical sections cover source capture,
//! deletion, and publication. Provider calls hold only a revocable source token.
//! Deleting a parent (even if recreated under the same key) revokes that token.
//! Weak registrations disappear when work completes; no durable tombstone table
//! is needed because provider futures cannot survive a process restart.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, Weak};

use cognigraph_core::{
    BatchOp, CogniGraphError, CollectionType, DocumentId, FieldPredicate, GraphBackend,
    PredicateOp, Result,
};
use serde_json::{Value, json};
use tokio::sync::Mutex as AsyncMutex;

use crate::system_collections::{SIDE_VIEWS_COLLECTION, deny_public_collection_mutation};
use crate::tenancy::{current_tenant, request_incarnation};

type Registrations = HashMap<DocumentId, Weak<()>>;
type Scope = Arc<AsyncMutex<Registrations>>;
type ScopeRegistry = HashMap<(String, String), Weak<AsyncMutex<Registrations>>>;

#[derive(Default)]
pub(crate) struct SideViews {
    scopes: Mutex<ScopeRegistry>,
}

pub(crate) struct Source {
    pub document: Value,
    parent: DocumentId,
    token: Arc<()>,
    scope: Scope,
}

impl SideViews {
    fn scope(&self, tenant: &str, incarnation: &str) -> Scope {
        let mut scopes = self.scopes.lock().expect("side-view scopes lock");
        scopes.retain(|_, scope| scope.strong_count() > 0);
        let slot = scopes
            .entry((tenant.into(), incarnation.into()))
            .or_default();
        if let Some(scope) = slot.upgrade() {
            return scope;
        }
        let scope = Arc::new(AsyncMutex::new(HashMap::new()));
        *slot = Arc::downgrade(&scope);
        scope
    }

    fn current_scope(&self) -> Scope {
        let tenant = current_tenant();
        let incarnation = request_incarnation(&tenant).unwrap_or_else(|| {
            if tenant == cognigraph_auth::DEFAULT_TENANT {
                tenant.clone()
            } else {
                format!("unmanaged:{tenant}")
            }
        });
        self.scope(&tenant, &incarnation)
    }

    pub async fn prepare(
        &self,
        backend: &dyn GraphBackend,
        tenant: &str,
        incarnation: &str,
        parent: DocumentId,
        regenerate: bool,
    ) -> Result<Option<Source>> {
        require_atomic_publication(backend)?;
        require_source_identity(&parent)?;
        let scope = self.scope(tenant, incarnation);
        let mut active = scope.lock().await;
        let Some(document) = backend
            .get_document(&parent.collection, &parent.key)
            .await?
        else {
            return Ok(None);
        };
        if !regenerate && !parent_rows(backend, &parent).await?.is_empty() {
            return Ok(None);
        }
        active.retain(|_, token| token.strong_count() > 0);
        let slot = active.entry(parent.clone()).or_default();
        let token = slot.upgrade().unwrap_or_else(|| Arc::new(()));
        *slot = Arc::downgrade(&token);
        drop(active);
        Ok(Some(Source {
            document,
            parent,
            token,
            scope,
        }))
    }

    pub async fn publish(
        &self,
        backend: &dyn GraphBackend,
        source: Source,
        documents: Vec<Value>,
        regenerate: bool,
    ) -> Result<usize> {
        require_atomic_publication(backend)?;
        let active = source.scope.lock().await;
        let valid = active
            .get(&source.parent)
            .and_then(Weak::upgrade)
            .is_some_and(|token| Arc::ptr_eq(&token, &source.token));
        if !valid
            || backend
                .get_document(&source.parent.collection, &source.parent.key)
                .await?
                .is_none()
        {
            return Err(CogniGraphError::DocumentConflict(format!(
                "side-view source `{}` was deleted or invalidated during generation; retry with the current source",
                source.parent
            )));
        }
        // Read again under the publication fence: overlapping generation must
        // not append duplicates or delete an obsolete snapshot of the old rows.
        let existing = parent_rows(backend, &source.parent).await?;
        if !regenerate && !existing.is_empty() {
            return Ok(0);
        }
        let mut ops = delete_ops(existing)?;
        let count = documents.len();
        for mut document in documents {
            document["document_id"] = json!(source.parent.full_id());
            ops.push(BatchOp::Insert {
                collection: SIDE_VIEWS_COLLECTION.into(),
                doc: document,
            });
        }
        if !ops.is_empty() {
            backend
                .ensure_collection(SIDE_VIEWS_COLLECTION, CollectionType::Document)
                .await?;
            backend.execute_batch(ops).await?;
        }
        Ok(count)
    }

    /// Returns the parent deletion result and the number of removed side-views.
    /// The same method serves the HTTP count envelope and GraphBackend facade.
    pub async fn delete_document(
        &self,
        backend: &dyn GraphBackend,
        collection: &str,
        key: &str,
    ) -> Result<(bool, usize)> {
        deny_public_collection_mutation(collection)?;
        let scope = self.current_scope();
        let mut active = scope.lock().await;
        let parent = DocumentId::new(collection, key);
        let mut ops = delete_ops(parent_rows(backend, &parent).await?)?;
        active.remove(&parent);
        let count = ops.len();
        if count == 0 {
            return Ok((backend.delete_document(collection, key).await?, 0));
        }
        if backend.supports_atomic_batches() {
            let exists = backend.get_document(collection, key).await?.is_some();
            if exists {
                ops.push(BatchOp::Delete {
                    collection: collection.into(),
                    key: key.into(),
                });
            }
            backend.execute_batch(ops).await?;
            Ok((exists, count))
        } else {
            // Non-transactional backends cannot generate side-views, but may
            // contain imported ones. Remove derivatives first; any error stops
            // before deleting the parent, and retry finishes the cleanup.
            let removed = cleanup(backend, ops).await?;
            Ok((backend.delete_document(collection, key).await?, removed))
        }
    }

    pub async fn execute_batch(
        &self,
        backend: &dyn GraphBackend,
        mut ops: Vec<BatchOp>,
    ) -> Result<Vec<Value>> {
        let parents: HashSet<_> = ops
            .iter()
            .filter_map(|op| match op {
                BatchOp::Delete { collection, key } => Some(DocumentId::new(collection, key)),
                _ => None,
            })
            .collect();
        if parents.is_empty() {
            return backend.execute_batch(ops).await;
        }
        let scope = self.current_scope();
        let mut active = scope.lock().await;
        let original_len = ops.len();
        for parent in &parents {
            ops.extend(delete_ops(parent_rows(backend, parent).await?)?);
        }
        // Native batches remove source and derivative rows in one transaction.
        // Never split a caller's batch into a partially applied cascade.
        if ops.len() > original_len && !backend.supports_atomic_batches() {
            return Err(CogniGraphError::BackendError(
                "side-view cascade requires atomic batch support".into(),
            ));
        }
        for parent in parents {
            active.remove(&parent);
        }
        let mut results = backend.execute_batch(ops).await?;
        results.truncate(original_len);
        Ok(results)
    }

    pub async fn drop_collection(&self, backend: &dyn GraphBackend, name: &str) -> Result<()> {
        deny_public_collection_mutation(name)?;
        let scope = self.current_scope();
        let mut active = scope.lock().await;
        let rows = if has_side_views(backend).await? {
            backend
                .list_documents_projected(
                    SIDE_VIEWS_COLLECTION,
                    &["document_id".into()],
                    None,
                    None,
                )
                .await?
        } else {
            Vec::new()
        };
        let prefix = format!("{name}/");
        let ops = delete_ops(
            rows.into_iter()
                .filter(|row| {
                    row.get("document_id")
                        .and_then(Value::as_str)
                        .is_some_and(|id| id.starts_with(&prefix))
                })
                .collect(),
        )?;
        active.retain(|parent, _| parent.collection != name);
        // Collection DDL cannot join a document batch. Cleaning first makes a
        // failed drop or crash retryable without ever leaving orphaned rows.
        cleanup(backend, ops).await?;
        backend.drop_collection(name).await
    }
}

pub(crate) fn require_atomic_publication(backend: &dyn GraphBackend) -> Result<()> {
    if !backend.supports_atomic_batches() {
        return Err(CogniGraphError::ValidationError(
            "side-view generation requires atomic batch support".into(),
        ));
    }
    Ok(())
}

/// A full handle separates the exact collection and key at the first slash.
/// Native storage preserves both Unicode forms, including job payloads and
/// generated parent references. Only ambiguous collection delimiters are denied.
pub(crate) fn require_source_identity(parent: &DocumentId) -> Result<()> {
    if parent.collection.contains('/') {
        return Err(CogniGraphError::ValidationError(
            "side-view source collection names must not contain `/`".into(),
        ));
    }
    Ok(())
}

async fn has_side_views(backend: &dyn GraphBackend) -> Result<bool> {
    // Explicit existence check also supports remote backends that surface a
    // missing collection as a query error. Other errors must propagate.
    Ok(backend
        .list_collections()
        .await?
        .iter()
        .any(|c| c.name == SIDE_VIEWS_COLLECTION))
}

async fn parent_rows(backend: &dyn GraphBackend, parent: &DocumentId) -> Result<Vec<Value>> {
    if !has_side_views(backend).await? {
        return Ok(Vec::new());
    }
    backend
        .list_documents_filtered(
            SIDE_VIEWS_COLLECTION,
            &[FieldPredicate {
                path: vec!["document_id".into()],
                op: PredicateOp::Eq,
                value: json!(parent.full_id()),
            }],
            Some(&["_key".into()]),
            None,
            None,
        )
        .await
}

fn delete_ops(rows: Vec<Value>) -> Result<Vec<BatchOp>> {
    rows.into_iter()
        .map(|row| {
            let key = row
                .get("_key")
                .and_then(Value::as_str)
                .filter(|key| !key.is_empty())
                .ok_or_else(|| {
                    CogniGraphError::BackendError(
                        "side-view cleanup encountered a row without a valid _key".into(),
                    )
                })?;
            Ok(BatchOp::Delete {
                collection: SIDE_VIEWS_COLLECTION.into(),
                key: key.into(),
            })
        })
        .collect()
}

async fn cleanup(backend: &dyn GraphBackend, ops: Vec<BatchOp>) -> Result<usize> {
    let count = ops.len();
    if count == 0 {
        return Ok(0);
    }
    if backend.supports_atomic_batches() {
        backend.execute_batch(ops).await?;
        return Ok(count);
    }
    let mut removed = 0;
    for op in ops {
        if let BatchOp::Delete { collection, key } = op {
            removed += usize::from(backend.delete_document(&collection, &key).await?);
        }
    }
    Ok(removed)
}

#[cfg(test)]
mod tests;
