//! Public-backend guard: underscore-prefixed collections (`_users`,
//! `_tokens`, `_tenants`, …) are control storage, never application data,
//! while the Semantic Neuron authority and its derived graph collections are
//! readable but may only be mutated by typed internal workflows.
//! In single-store mode they live in the same `GraphBackend` as documents,
//! so every public data route must refuse them — otherwise a caller with
//! DocumentsRead can read password hashes and DocumentsWrite can escalate
//! a role (decision_system_collections.md).
//!
//! `GuardedBackend` is the choke point: `AppState` wraps every public backend
//! in it, so documents, graph, embedding, batch, CGQL execution, and Lua all
//! inherit the policy without per-route checks. System components and typed
//! construction workflows hold raw backend handles taken BEFORE the wrap.
//! Snapshot export/import stay unguarded on purpose: they are whole-database
//! backup under the Admin scope.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use cognigraph_core::{
    BatchOp, CogniGraphError, CollectionType, Direction, DocumentId, FieldPredicate, GraphBackend,
    IndexDef, QueryLanguage, Result, SearchHit, TraversalOpts, TraversalPath,
};

/// Semantic authority and deterministically derived graph collections.
///
/// These deliberately remain ordinary (non-underscore) collection names so
/// generic document/search reads keep working. Public mutation is denied;
/// only typed internal construction and governance workflows may write them.
const MANAGED_COLLECTIONS: [&str; 8] = [
    "space_types",
    "neurons",
    "review_policies",
    "eval_specs",
    "entities",
    "chunks",
    "mentions",
    "facts",
];

/// The single tenant-scoped collection holding machine-generated retrieval
/// "side-views" (context-expansion Q&A). Exported so the generation job and the
/// search/cascade routes all name it from one place.
pub const SIDE_VIEWS_COLLECTION: &str = "side_views";

/// Ledger of what the construction gates refused (CG-90). Generated, not
/// governed: readable and exportable, publicly write-protected, never part of
/// promotion or attestation.
pub const REFUSALS_COLLECTION: &str = "construction_refusals";

/// Machine-generated, non-authoritative collections. Like the managed
/// collections these keep an ordinary (non-underscore) name so generic reads and
/// vector search keep working, and public mutation is denied on every surface —
/// but they are deliberately kept SEPARATE from [`MANAGED_COLLECTIONS`]: a
/// side-view is a search aid and a semantics verdict is a review aid, not
/// governed facts, so neither is ever swept into the promotion / attestation /
/// materialization machinery. Only the generating job or construction itself
/// writes them (through the trusted internal backend handle), exactly as
/// construction writes the derived graph collections.
///
/// `fact_semantics` in particular exists as a sidecar precisely so that
/// improving the detector never invalidates a signed M26 projection
/// (decision_pilot_clinical_graph.md, D2).
const GENERATED_COLLECTIONS: [&str; 3] =
    [SIDE_VIEWS_COLLECTION, "fact_semantics", REFUSALS_COLLECTION];

pub fn is_system_collection(name: &str) -> bool {
    name.starts_with('_')
}

pub fn is_managed_collection(name: &str) -> bool {
    MANAGED_COLLECTIONS.contains(&name)
}

pub fn is_generated_collection(name: &str) -> bool {
    GENERATED_COLLECTIONS.contains(&name)
}

fn forbidden(name: &str) -> CogniGraphError {
    CogniGraphError::Forbidden(format!(
        "collection `{name}` is system-reserved and not accessible through the API"
    ))
}

fn managed_mutation_forbidden(name: &str) -> CogniGraphError {
    CogniGraphError::Forbidden(format!(
        "collection `{name}` is managed by governed Semantic Neuron workflows and is read-only through generic APIs"
    ))
}

fn generated_mutation_forbidden(name: &str) -> CogniGraphError {
    CogniGraphError::Forbidden(format!(
        "collection `{name}` holds machine-generated, non-authoritative data and is read-only through generic APIs; only the job that generates it may write it"
    ))
}

/// Reject an underscore-prefixed collection name.
pub fn deny_system_collection(name: &str) -> Result<()> {
    if is_system_collection(name) {
        return Err(forbidden(name));
    }
    Ok(())
}

/// Reject a public mutation of Semantic Neuron authority or derived graph
/// state. Reads intentionally continue through the guarded backend.
pub fn deny_managed_collection_mutation(name: &str) -> Result<()> {
    if is_managed_collection(name) {
        return Err(managed_mutation_forbidden(name));
    }
    Ok(())
}

/// Reject a public mutation of a machine-generated side-view collection. Reads
/// (including vector search) intentionally continue through the guarded backend;
/// only the generation job writes these, through the trusted internal handle.
pub fn deny_generated_collection_mutation(name: &str) -> Result<()> {
    if is_generated_collection(name) {
        return Err(generated_mutation_forbidden(name));
    }
    Ok(())
}

pub(crate) fn deny_public_collection_mutation(name: &str) -> Result<()> {
    deny_system_collection(name)?;
    deny_managed_collection_mutation(name)?;
    deny_generated_collection_mutation(name)
}

/// Reject a vertex/document id (`collection/key`) inside a system
/// collection — traversals and edges must not touch control documents.
pub fn deny_system_vertex(id: &str) -> Result<()> {
    match id.split_once('/') {
        Some((collection, _)) => deny_system_collection(collection),
        None => Ok(()),
    }
}

fn deny_managed_vertex_mutation(id: &str) -> Result<()> {
    match id.split_once('/') {
        Some((collection, _)) => deny_managed_collection_mutation(collection),
        None => Ok(()),
    }
}

/// Reject system vertex references carried by an edge-shaped document or
/// partial update. Native traversal intentionally accepts any collection that
/// contains `_from`/`_to`, so this guard applies to ordinary document writes
/// and batches as well as the explicit edge APIs.
fn deny_system_vertices_in_value(value: &serde_json::Value) -> Result<()> {
    for field in ["_from", "_to"] {
        if let Some(id) = value.get(field).and_then(serde_json::Value::as_str) {
            deny_system_vertex(id)?;
        }
    }
    Ok(())
}

fn deny_managed_vertices_in_mutation(value: &serde_json::Value) -> Result<()> {
    for field in ["_from", "_to"] {
        if let Some(id) = value.get(field).and_then(serde_json::Value::as_str) {
            deny_managed_vertex_mutation(id)?;
        }
    }
    Ok(())
}

/// Statically reject a CGQL query that reads or writes any system collection,
/// or mutates a governed Semantic Neuron collection. Managed collection reads
/// remain available. Unparseable queries pass — the executor reports the
/// parse error with its usual diagnostics.
pub fn deny_system_collections_in_cgql(query: &str) -> Result<()> {
    if let Ok(parsed) = cognigraph_query::parse_query(query) {
        for collection in parsed.referenced_collections() {
            deny_system_collection(&collection)?;
        }
        if let Some(mutation) = parsed.mutation {
            use cognigraph_query::MutationClause;
            let collection = match mutation {
                MutationClause::Insert { collection, .. }
                | MutationClause::Update { collection, .. }
                | MutationClause::Replace { collection, .. }
                | MutationClause::Remove { collection, .. }
                | MutationClause::Upsert { collection, .. } => collection,
            };
            deny_managed_collection_mutation(&collection)?;
            deny_generated_collection_mutation(&collection)?;
        }
    }
    Ok(())
}

/// A `GraphBackend` facade that enforces the system-collection policy on
/// every operation. Deletes share the side-view lifecycle fence with generation;
/// other operations delegate to the wrapped backend.
pub struct GuardedBackend {
    inner: Arc<dyn GraphBackend>,
    #[cfg(feature = "enterprise")]
    side_views: Arc<crate::side_views::SideViews>,
}

impl GuardedBackend {
    #[cfg(any(test, not(feature = "enterprise")))]
    pub fn new(inner: Arc<dyn GraphBackend>) -> Self {
        #[cfg(feature = "enterprise")]
        {
            Self::with_side_views(inner, Arc::new(crate::side_views::SideViews::default()))
        }
        #[cfg(not(feature = "enterprise"))]
        {
            Self { inner }
        }
    }

    #[cfg(feature = "enterprise")]
    pub(crate) fn with_side_views(
        inner: Arc<dyn GraphBackend>,
        side_views: Arc<crate::side_views::SideViews>,
    ) -> Self {
        Self { inner, side_views }
    }
}

#[async_trait]
impl GraphBackend for GuardedBackend {
    fn backend_name(&self) -> &str {
        self.inner.backend_name()
    }

    fn query_language(&self) -> QueryLanguage {
        self.inner.query_language()
    }

    fn supports_atomic_batches(&self) -> bool {
        self.inner.supports_atomic_batches()
    }

    async fn ping(&self) -> Result<()> {
        self.inner.ping().await
    }

    async fn create_document(
        &self,
        collection: &str,
        doc: serde_json::Value,
    ) -> Result<DocumentId> {
        deny_public_collection_mutation(collection)?;
        deny_system_vertices_in_value(&doc)?;
        deny_managed_vertices_in_mutation(&doc)?;
        self.inner.create_document(collection, doc).await
    }

    async fn get_document(&self, collection: &str, key: &str) -> Result<Option<serde_json::Value>> {
        deny_system_collection(collection)?;
        let document = self.inner.get_document(collection, key).await?;
        if let Some(value) = &document {
            deny_system_vertices_in_value(value)?;
        }
        Ok(document)
    }

    async fn update_document(
        &self,
        collection: &str,
        key: &str,
        update: serde_json::Value,
    ) -> Result<serde_json::Value> {
        deny_public_collection_mutation(collection)?;
        deny_system_vertices_in_value(&update)?;
        deny_managed_vertices_in_mutation(&update)?;
        let document = self.inner.update_document(collection, key, update).await?;
        deny_system_vertices_in_value(&document)?;
        Ok(document)
    }

    async fn replace_document(
        &self,
        collection: &str,
        key: &str,
        doc: serde_json::Value,
    ) -> Result<serde_json::Value> {
        deny_public_collection_mutation(collection)?;
        deny_system_vertices_in_value(&doc)?;
        deny_managed_vertices_in_mutation(&doc)?;
        let document = self.inner.replace_document(collection, key, doc).await?;
        deny_system_vertices_in_value(&document)?;
        Ok(document)
    }

    async fn delete_document(&self, collection: &str, key: &str) -> Result<bool> {
        deny_public_collection_mutation(collection)?;
        #[cfg(feature = "enterprise")]
        {
            self.side_views
                .delete_document(self.inner.as_ref(), collection, key)
                .await
                .map(|(deleted, _)| deleted)
        }
        #[cfg(not(feature = "enterprise"))]
        {
            self.inner.delete_document(collection, key).await
        }
    }

    async fn list_documents(
        &self,
        collection: &str,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<serde_json::Value>> {
        deny_system_collection(collection)?;
        let documents = self.inner.list_documents(collection, limit, offset).await?;
        for document in &documents {
            deny_system_vertices_in_value(document)?;
        }
        Ok(documents)
    }

    async fn list_documents_projected(
        &self,
        collection: &str,
        fields: &[String],
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<serde_json::Value>> {
        deny_system_collection(collection)?;
        let documents = self
            .inner
            .list_documents_projected(collection, fields, limit, offset)
            .await?;
        for document in &documents {
            deny_system_vertices_in_value(document)?;
        }
        Ok(documents)
    }

    async fn list_documents_after_key(
        &self,
        collection: &str,
        after_key: Option<&str>,
        fields: &[String],
        limit: usize,
    ) -> Result<Vec<serde_json::Value>> {
        deny_system_collection(collection)?;
        let documents = self
            .inner
            .list_documents_after_key(collection, after_key, fields, limit)
            .await?;
        for document in &documents {
            deny_system_vertices_in_value(document)?;
        }
        Ok(documents)
    }

    async fn list_documents_filtered(
        &self,
        collection: &str,
        predicates: &[FieldPredicate],
        fields: Option<&[String]>,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<serde_json::Value>> {
        deny_system_collection(collection)?;
        let documents = self
            .inner
            .list_documents_filtered(collection, predicates, fields, limit, offset)
            .await?;
        for document in &documents {
            deny_system_vertices_in_value(document)?;
        }
        Ok(documents)
    }

    async fn list_collections(&self) -> Result<Vec<cognigraph_core::CollectionInfo>> {
        Ok(self
            .inner
            .list_collections()
            .await?
            .into_iter()
            .filter(|info| !is_system_collection(&info.name))
            .collect())
    }

    async fn export_snapshot(&self) -> Result<serde_json::Value> {
        self.inner.export_snapshot().await
    }

    async fn import_snapshot(&self, data: &serde_json::Value) -> Result<()> {
        self.inner.import_snapshot(data).await
    }

    async fn create_edge(&self, collection: &str, edge: serde_json::Value) -> Result<DocumentId> {
        deny_public_collection_mutation(collection)?;
        deny_system_vertices_in_value(&edge)?;
        deny_managed_vertices_in_mutation(&edge)?;
        self.inner.create_edge(collection, edge).await
    }

    async fn upsert_edge(
        &self,
        collection: &str,
        from: &str,
        to: &str,
        relation_type: &str,
        data: serde_json::Value,
    ) -> Result<serde_json::Value> {
        deny_public_collection_mutation(collection)?;
        deny_system_vertex(from)?;
        deny_system_vertex(to)?;
        deny_managed_vertex_mutation(from)?;
        deny_managed_vertex_mutation(to)?;
        deny_system_vertices_in_value(&data)?;
        deny_managed_vertices_in_mutation(&data)?;
        let edge = self
            .inner
            .upsert_edge(collection, from, to, relation_type, data)
            .await?;
        deny_system_vertices_in_value(&edge)?;
        Ok(edge)
    }

    async fn get_edges(
        &self,
        collection: &str,
        vertex_id: &str,
        direction: Direction,
    ) -> Result<Vec<serde_json::Value>> {
        deny_system_collection(collection)?;
        deny_system_vertex(vertex_id)?;
        let edges = self
            .inner
            .get_edges(collection, vertex_id, direction)
            .await?;
        for edge in &edges {
            deny_system_vertices_in_value(edge)?;
        }
        Ok(edges)
    }

    async fn execute_batch(&self, ops: Vec<BatchOp>) -> Result<Vec<serde_json::Value>> {
        for op in &ops {
            let (collection, value) = match op {
                BatchOp::Insert { collection, doc } => (collection, Some(doc)),
                BatchOp::Update {
                    collection, merge, ..
                } => (collection, Some(merge)),
                BatchOp::Replace {
                    collection, doc, ..
                } => (collection, Some(doc)),
                BatchOp::Delete { collection, .. } => (collection, None),
            };
            deny_public_collection_mutation(collection)?;
            if let Some(value) = value {
                deny_system_vertices_in_value(value)?;
                deny_managed_vertices_in_mutation(value)?;
            }
        }
        #[cfg(not(feature = "enterprise"))]
        let results = self.inner.execute_batch(ops).await?;
        #[cfg(feature = "enterprise")]
        let results = self
            .side_views
            .execute_batch(self.inner.as_ref(), ops)
            .await?;
        for result in &results {
            deny_system_vertices_in_value(result)?;
        }
        Ok(results)
    }

    async fn traverse(
        &self,
        start_vertex: &str,
        opts: &TraversalOpts,
    ) -> Result<Vec<TraversalPath>> {
        deny_system_vertex(start_vertex)?;
        deny_system_collection(&opts.edge_collection)?;
        let paths = self.inner.traverse(start_vertex, opts).await?;
        for path in &paths {
            for vertex in &path.vertices {
                if let Some(id) = vertex.get("_id").and_then(serde_json::Value::as_str) {
                    deny_system_vertex(id)?;
                }
            }
            for edge in &path.edges {
                deny_system_vertices_in_value(edge)?;
            }
        }
        Ok(paths)
    }

    async fn vector_search(
        &self,
        collection: &str,
        query_vector: &[f64],
        opts: &cognigraph_core::VectorSearchOpts,
    ) -> Result<Vec<SearchHit>> {
        deny_system_collection(collection)?;
        let hits = self
            .inner
            .vector_search(collection, query_vector, opts)
            .await?;
        for hit in &hits {
            deny_system_vertices_in_value(&hit.document)?;
        }
        Ok(hits)
    }

    async fn text_search(
        &self,
        collection: &str,
        query: &str,
        fields: &[String],
        limit: usize,
    ) -> Result<Vec<SearchHit>> {
        deny_system_collection(collection)?;
        let hits = self
            .inner
            .text_search(collection, query, fields, limit)
            .await?;
        for hit in &hits {
            deny_system_vertices_in_value(&hit.document)?;
        }
        Ok(hits)
    }

    async fn query(
        &self,
        query: &str,
        bind_vars: HashMap<String, serde_json::Value>,
    ) -> Result<Vec<serde_json::Value>> {
        // Native backends execute CGQL internally (against themselves, not
        // through this facade). Execute the parsed query against this facade
        // so runtime traversal starts and returned vertices receive the same
        // system-collection checks as direct trait calls.
        if self.inner.query_language() != QueryLanguage::Cgql {
            return Err(CogniGraphError::Forbidden(
                "queries require a backend with parsed CGQL support".into(),
            ));
        }
        deny_system_collections_in_cgql(query)?;
        match cognigraph_query::parse_and_execute_backend(query, self, &bind_vars).await {
            Ok(rows) => Ok(rows),
            Err(cognigraph_query::ExecutionError::Forbidden(message)) => {
                Err(CogniGraphError::Forbidden(message))
            }
            Err(error) => Err(CogniGraphError::QueryError(error.to_string())),
        }
    }

    async fn ensure_collection(&self, name: &str, collection_type: CollectionType) -> Result<()> {
        deny_public_collection_mutation(name)?;
        self.inner.ensure_collection(name, collection_type).await
    }

    async fn ensure_index(&self, collection: &str, index: &IndexDef) -> Result<()> {
        deny_public_collection_mutation(collection)?;
        self.inner.ensure_index(collection, index).await
    }

    async fn drop_collection(&self, name: &str) -> Result<()> {
        deny_public_collection_mutation(name)?;
        #[cfg(feature = "enterprise")]
        {
            self.side_views
                .drop_collection(self.inner.as_ref(), name)
                .await
        }
        #[cfg(not(feature = "enterprise"))]
        {
            self.inner.drop_collection(name).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cognigraph_native::NativeBackend;
    use serde_json::json;

    /// A native backend holding a control-store user document, wrapped in
    /// the guard — the single-store layout under test.
    async fn guarded() -> GuardedBackend {
        let raw = NativeBackend::new();
        raw.create_document(
            "_users",
            json!({"_key": "admin", "password_hash": "bcrypt$secret"}),
        )
        .await
        .unwrap();
        raw.create_document("notes", json!({"_key": "n1", "title": "public"}))
            .await
            .unwrap();
        raw.create_document("neurons", json!({"_key": "neuron-1", "status": "accepted"}))
            .await
            .unwrap();
        GuardedBackend::new(Arc::new(raw))
    }

    fn assert_forbidden<T: std::fmt::Debug>(result: Result<T>) {
        match result {
            Err(CogniGraphError::Forbidden(message)) => {
                assert!(message.contains("system-reserved"), "message: {message}");
            }
            other => panic!("expected Forbidden, got {other:?}"),
        }
    }

    fn assert_managed_forbidden<T: std::fmt::Debug>(result: Result<T>) {
        match result {
            Err(CogniGraphError::Forbidden(message)) => {
                assert!(message.contains("managed"), "message: {message}");
                assert!(message.contains("read-only"), "message: {message}");
            }
            other => panic!("expected managed Forbidden, got {other:?}"),
        }
    }

    fn assert_generated_forbidden<T: std::fmt::Debug>(result: Result<T>) {
        match result {
            Err(CogniGraphError::Forbidden(message)) => {
                assert!(message.contains("machine-generated"), "message: {message}");
                assert!(message.contains("read-only"), "message: {message}");
            }
            other => panic!("expected generated Forbidden, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn document_operations_reject_system_collections() {
        let backend = guarded().await;
        assert_forbidden(backend.get_document("_users", "admin").await);
        assert_forbidden(backend.list_documents("_users", None, None).await);
        assert_forbidden(
            backend
                .list_documents_projected("_users", &["password_hash".into()], None, None)
                .await,
        );
        assert_forbidden(
            backend
                .list_documents_after_key("_users", None, &["password_hash".into()], 10)
                .await,
        );
        assert_forbidden(
            backend
                .list_documents_filtered("_users", &[], None, None, None)
                .await,
        );
        assert_forbidden(
            backend
                .create_document("_users", json!({"role": "Admin"}))
                .await,
        );
        assert_forbidden(
            backend
                .update_document("_users", "admin", json!({"role": "Admin"}))
                .await,
        );
        assert_forbidden(
            backend
                .replace_document("_users", "admin", json!({"role": "Admin"}))
                .await,
        );
        assert_forbidden(backend.delete_document("_users", "admin").await);
        // Non-system collections stay fully readable.
        assert!(backend.get_document("notes", "n1").await.unwrap().is_some());
    }

    #[tokio::test]
    async fn managed_collections_are_readable_but_direct_mutations_are_forbidden() {
        let backend = guarded().await;

        let neuron = backend
            .get_document("neurons", "neuron-1")
            .await
            .unwrap()
            .expect("managed documents remain readable");
        assert_eq!(neuron["status"], json!("accepted"));
        assert_eq!(
            backend
                .list_documents("neurons", None, None)
                .await
                .unwrap()
                .len(),
            1
        );

        for collection in MANAGED_COLLECTIONS {
            assert_managed_forbidden(
                backend
                    .create_document(collection, json!({"_key": "bypass"}))
                    .await,
            );
        }
        assert_managed_forbidden(
            backend
                .update_document("neurons", "neuron-1", json!({"status": "accepted"}))
                .await,
        );
        assert_managed_forbidden(
            backend
                .replace_document("neurons", "neuron-1", json!({"status": "accepted"}))
                .await,
        );
        assert_managed_forbidden(backend.delete_document("neurons", "neuron-1").await);

        // Failed writes cannot alter the readable authority document.
        assert_eq!(
            backend
                .get_document("neurons", "neuron-1")
                .await
                .unwrap()
                .unwrap()["status"],
            json!("accepted")
        );
    }

    #[tokio::test]
    async fn edges_and_traversal_reject_system_endpoints() {
        let backend = guarded().await;
        assert_forbidden(
            backend
                .upsert_edge("_rels", "notes/n1", "notes/n1", "self", json!({}))
                .await,
        );
        assert_forbidden(
            backend
                .upsert_edge("rels", "_users/admin", "notes/n1", "leak", json!({}))
                .await,
        );
        assert_forbidden(
            backend
                .upsert_edge(
                    "rels",
                    "notes/n1",
                    "notes/n1",
                    "leak",
                    json!({"_to": "_users/admin"}),
                )
                .await,
        );
        assert_forbidden(
            backend
                .create_edge("rels", json!({"_from": "notes/n1", "_to": "_users/admin"}))
                .await,
        );
        assert_forbidden(backend.get_edges("_rels", "notes/n1", Direction::Any).await);
        assert_forbidden(
            backend
                .get_edges("rels", "_users/admin", Direction::Any)
                .await,
        );

        fn opts(edge_collection: &str) -> TraversalOpts {
            TraversalOpts {
                max_depth: 2,
                min_depth: 1,
                direction: Direction::Any,
                edge_collection: edge_collection.into(),
                min_confidence: None,
                path_decay: 0.8,
            }
        }
        assert_forbidden(backend.traverse("_users/admin", &opts("rels")).await);
        assert_forbidden(backend.traverse("notes/n1", &opts("_tokens")).await);
    }

    #[tokio::test]
    async fn edge_mutations_cannot_touch_managed_collections_or_vertices() {
        let backend = guarded().await;
        assert_managed_forbidden(
            backend
                .create_edge("facts", json!({"_from": "entities/a", "_to": "entities/b"}))
                .await,
        );
        assert_managed_forbidden(
            backend
                .create_edge(
                    "document_relations",
                    json!({"_from": "notes/n1", "_to": "entities/a"}),
                )
                .await,
        );
        assert_managed_forbidden(
            backend
                .upsert_edge(
                    "document_relations",
                    "entities/a",
                    "notes/n1",
                    "bypass",
                    json!({}),
                )
                .await,
        );
    }

    #[tokio::test]
    async fn batch_search_and_schema_reject_system_collections() {
        let backend = guarded().await;
        assert_forbidden(
            backend
                .execute_batch(vec![
                    BatchOp::Insert {
                        collection: "notes".into(),
                        doc: json!({"_key": "ok"}),
                    },
                    BatchOp::Update {
                        collection: "_users".into(),
                        key: "admin".into(),
                        merge: json!({"role": "Admin"}),
                    },
                ])
                .await,
        );
        // The rejected batch must not have applied its first op either.
        assert!(backend.get_document("notes", "ok").await.unwrap().is_none());

        for op in [
            BatchOp::Insert {
                collection: "rels".into(),
                doc: json!({"_key": "leak", "_from": "notes/n1", "_to": "_users/admin"}),
            },
            BatchOp::Update {
                collection: "notes".into(),
                key: "n1".into(),
                merge: json!({"_to": "_users/admin"}),
            },
            BatchOp::Replace {
                collection: "notes".into(),
                key: "n1".into(),
                doc: json!({"_from": "notes/n1", "_to": "_users/admin"}),
            },
        ] {
            assert_forbidden(backend.execute_batch(vec![op]).await);
        }
        assert_forbidden(
            backend
                .create_document("rels", json!({"_from": "notes/n1", "_to": "_users/admin"}))
                .await,
        );
        assert_forbidden(
            backend
                .update_document("notes", "n1", json!({"_to": "_users/admin"}))
                .await,
        );
        assert_forbidden(
            backend
                .replace_document(
                    "notes",
                    "n1",
                    json!({"_from": "notes/n1", "_to": "_users/admin"}),
                )
                .await,
        );

        let opts = cognigraph_core::VectorSearchOpts {
            threshold: None,
            limit: 10,
            model_name: None,
        };
        assert_forbidden(backend.vector_search("_users", &[0.0], &opts).await);
        assert_forbidden(backend.text_search("_users", "secret", &[], 10).await);
        assert_forbidden(
            backend
                .ensure_collection("_shadow", CollectionType::Document)
                .await,
        );
        assert_forbidden(backend.drop_collection("_users").await);
    }

    #[tokio::test]
    async fn managed_batch_is_rejected_before_any_operation_applies() {
        let backend = guarded().await;
        assert_managed_forbidden(
            backend
                .execute_batch(vec![
                    BatchOp::Insert {
                        collection: "notes".into(),
                        doc: json!({"_key": "must-not-commit"}),
                    },
                    BatchOp::Update {
                        collection: "neurons".into(),
                        key: "neuron-1".into(),
                        merge: json!({"status": "accepted"}),
                    },
                ])
                .await,
        );
        assert!(
            backend
                .get_document("notes", "must-not-commit")
                .await
                .unwrap()
                .is_none()
        );
        assert_managed_forbidden(
            backend
                .execute_batch(vec![BatchOp::Insert {
                    collection: "document_relations".into(),
                    doc: json!({"_from": "notes/n1", "_to": "entities/a"}),
                }])
                .await,
        );
    }

    #[tokio::test]
    async fn managed_schema_mutations_are_forbidden() {
        let backend = guarded().await;
        assert_managed_forbidden(
            backend
                .ensure_collection("neurons", CollectionType::Document)
                .await,
        );
        assert_managed_forbidden(
            backend
                .ensure_index(
                    "neurons",
                    &IndexDef {
                        index_type: cognigraph_core::IndexType::Persistent,
                        fields: vec!["status".into()],
                        unique: false,
                        sparse: false,
                        name: None,
                    },
                )
                .await,
        );
        assert_managed_forbidden(backend.drop_collection("neurons").await);
    }

    #[tokio::test]
    async fn cgql_queries_reject_system_collections() {
        let backend = guarded().await;
        for query in [
            "FOR u IN _users RETURN u",
            "LET t = (FOR t IN _tokens RETURN t) RETURN t",
            "FOR u IN _users UPDATE u._key WITH { role: \"Admin\" } IN _users",
            "INSERT { user: \"u1\" } INTO _tokens",
        ] {
            assert_forbidden(backend.query(query, HashMap::new()).await);
        }
        // Ordinary queries pass through.
        let rows = backend
            .query("FOR n IN notes RETURN n.title", HashMap::new())
            .await
            .unwrap();
        assert_eq!(rows, vec![json!("public")]);

        // Runtime traversal starts are expressions/bind variables, not
        // referenced collections in the parsed AST. They must still pass
        // through GuardedBackend::traverse instead of the raw native backend.
        let mut vars = HashMap::new();
        vars.insert("start".into(), json!("_users/admin"));
        assert_forbidden(
            backend
                .query("FOR v, e, p IN 0..0 OUTBOUND @start rels RETURN v", vars)
                .await,
        );
    }

    #[tokio::test]
    async fn cgql_reads_managed_collections_but_rejects_every_mutation_kind() {
        let backend = guarded().await;
        let rows = backend
            .query("FOR neuron IN neurons RETURN neuron.status", HashMap::new())
            .await
            .unwrap();
        assert_eq!(rows, vec![json!("accepted")]);

        for query in [
            "INSERT { _key: \"bypass\" } INTO neurons",
            "UPDATE \"neuron-1\" WITH { status: \"accepted\" } IN neurons",
            "REPLACE \"neuron-1\" WITH { status: \"accepted\" } IN neurons",
            "REMOVE \"neuron-1\" IN neurons",
            "UPSERT { _key: \"neuron-1\" } INSERT { status: \"proposed\" } UPDATE { status: \"accepted\" } IN neurons",
        ] {
            assert_managed_forbidden(backend.query(query, HashMap::new()).await);
        }
    }

    #[tokio::test]
    async fn generated_side_views_are_readable_but_public_writes_are_forbidden() {
        // A side-view collection is written only by the generation job (through
        // the raw handle here); the public GuardedBackend may read it but must
        // reject any mutation, so users cannot forge or vandalize retrieval
        // surfaces. It is deliberately NOT a managed/authority collection.
        assert!(is_generated_collection(SIDE_VIEWS_COLLECTION));
        assert!(!is_managed_collection(SIDE_VIEWS_COLLECTION));

        let raw = NativeBackend::new();
        raw.create_document(
            SIDE_VIEWS_COLLECTION,
            json!({
                "_key": "sv1",
                "document_id": "notes/n1",
                "kind": "side_view",
                "question": "What is the capital of Portugal?",
                "answer": "Lisbon.",
                "embedding": [0.1, 0.2, 0.3],
            }),
        )
        .await
        .unwrap();
        let backend = GuardedBackend::new(Arc::new(raw));

        // Reads (and vector search) stay open — that is the whole point.
        let doc = backend
            .get_document(SIDE_VIEWS_COLLECTION, "sv1")
            .await
            .unwrap()
            .expect("side-views remain readable");
        assert_eq!(doc["kind"], json!("side_view"));

        // Typed public mutations are all forbidden.
        assert_generated_forbidden(
            backend
                .create_document(SIDE_VIEWS_COLLECTION, json!({"_key": "forged"}))
                .await,
        );
        assert_generated_forbidden(
            backend
                .update_document(SIDE_VIEWS_COLLECTION, "sv1", json!({"answer": "tampered"}))
                .await,
        );
        assert_generated_forbidden(
            backend
                .replace_document(SIDE_VIEWS_COLLECTION, "sv1", json!({"answer": "tampered"}))
                .await,
        );
        assert_generated_forbidden(backend.delete_document(SIDE_VIEWS_COLLECTION, "sv1").await);

        // CGQL reads pass; every mutation kind is rejected.
        let rows = backend
            .query(
                &format!("FOR sv IN {SIDE_VIEWS_COLLECTION} RETURN sv.answer"),
                HashMap::new(),
            )
            .await
            .unwrap();
        assert_eq!(rows, vec![json!("Lisbon.")]);
        for query in [
            format!("INSERT {{ _key: \"forged\" }} INTO {SIDE_VIEWS_COLLECTION}"),
            format!("UPDATE \"sv1\" WITH {{ answer: \"x\" }} IN {SIDE_VIEWS_COLLECTION}"),
            format!("REMOVE \"sv1\" IN {SIDE_VIEWS_COLLECTION}"),
        ] {
            assert_generated_forbidden(backend.query(&query, HashMap::new()).await);
        }

        assert!(deny_generated_collection_mutation("notes").is_ok());
    }

    /// The relation-semantics sidecar is generated, not governed: readable and
    /// joinable so a consumer can filter to the high-precision lane, but never
    /// publicly writable — a forged "clean" verdict must not be able to launder
    /// a suspect fact. It is deliberately NOT managed, so it stays out of the
    /// promotion/attestation machinery and the detector can evolve freely
    /// (decision_pilot_clinical_graph.md, D2).
    #[tokio::test]
    async fn fact_semantics_sidecar_is_readable_but_never_publicly_writable() {
        let sidecar = "fact_semantics";
        assert!(is_generated_collection(sidecar));
        assert!(
            !is_managed_collection(sidecar),
            "an advisory verdict must not enter the attested surface"
        );

        let raw = NativeBackend::new();
        raw.create_document(
            sidecar,
            json!({
                "_key": "f1",
                "fact_key": "f1",
                "space_id": "s1",
                "evidence_chunk_id": "c1",
                "suspect": true,
                "signals": ["target absent from the licensing sentence"],
                "detector_rev": "synthetic-test-revision",
            }),
        )
        .await
        .unwrap();
        let backend = GuardedBackend::new(Arc::new(raw));

        let doc = backend
            .get_document(sidecar, "f1")
            .await
            .unwrap()
            .expect("verdicts remain readable");
        assert_eq!(doc["suspect"], json!(true));

        assert_generated_forbidden(
            backend
                .create_document(sidecar, json!({"_key": "forged"}))
                .await,
        );
        assert_generated_forbidden(
            backend
                .update_document(sidecar, "f1", json!({"suspect": false}))
                .await,
        );
        assert_generated_forbidden(
            backend
                .replace_document(sidecar, "f1", json!({"suspect": false}))
                .await,
        );
        assert_generated_forbidden(backend.delete_document(sidecar, "f1").await);
        assert_generated_forbidden(
            backend
                .query(&format!("REMOVE \"f1\" IN {sidecar}"), HashMap::new())
                .await,
        );
    }

    #[tokio::test]
    async fn legacy_edges_into_system_vertices_are_never_returned() {
        let raw = NativeBackend::new();
        raw.create_document(
            "_users",
            json!({"_key": "admin", "password_hash": "bcrypt$secret"}),
        )
        .await
        .unwrap();
        raw.create_document("notes", json!({"_key": "n1"}))
            .await
            .unwrap();
        raw.create_edge(
            "legacy_rels",
            json!({"_key": "leak", "_from": "notes/n1", "_to": "_users/admin"}),
        )
        .await
        .unwrap();
        let backend = GuardedBackend::new(Arc::new(raw));
        assert_forbidden(backend.get_document("legacy_rels", "leak").await);
        assert_forbidden(backend.list_documents("legacy_rels", None, None).await);
        assert_forbidden(
            backend
                .get_edges("legacy_rels", "notes/n1", Direction::Outbound)
                .await,
        );
        assert_forbidden(
            backend
                .traverse(
                    "notes/n1",
                    &TraversalOpts {
                        max_depth: 1,
                        min_depth: 1,
                        direction: Direction::Outbound,
                        edge_collection: "legacy_rels".into(),
                        min_confidence: None,
                        path_decay: 0.8,
                    },
                )
                .await,
        );
    }

    #[tokio::test]
    async fn opaque_queries_are_rejected_before_any_backend_access() {
        let probe = Arc::new(cognigraph_core::contract::NoAccessBackend::default());
        let backend = GuardedBackend::new(probe.clone());
        for query in [
            "RETURN 1",
            "FOR row IN notes RETURN row",
            "RETURN DOCUMENT(CONCAT('_us', 'ers/admin'))",
            "INSERT {} INTO notes",
        ] {
            let error = backend.query(query, HashMap::new()).await.unwrap_err();
            assert!(matches!(error, CogniGraphError::Forbidden(message)
                if message.contains("parsed CGQL support")));
        }
        probe.assert_unused();
    }

    #[tokio::test]
    async fn catalog_hides_system_collections() {
        let backend = guarded().await;
        let names: Vec<String> = backend
            .list_collections()
            .await
            .unwrap()
            .into_iter()
            .map(|info| info.name)
            .collect();
        // Managed collections remain visible/readable; only underscore
        // control storage is removed from the public catalog.
        assert_eq!(names, vec!["neurons", "notes"]);
    }

    #[tokio::test]
    async fn snapshots_pass_through_for_admin_backup() {
        let backend = guarded().await;
        let snapshot = backend.export_snapshot().await.unwrap();
        assert!(snapshot["collections"].get("_users").is_some());
    }
}
