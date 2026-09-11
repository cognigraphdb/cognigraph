use async_trait::async_trait;
use std::collections::HashMap;
use tracing::debug;

use cognigraph_core::GraphBackend;
use cognigraph_core::{
    CogniGraphError, CollectionType, Direction, DocumentId, IndexDef, IndexType, Result, SearchHit,
    TraversalOpts, TraversalPath, VectorSearchOpts,
};

use crate::client::ArangoError;

use super::{ArangoBackend, extract_doc, extract_key, map_err};

#[async_trait]
impl GraphBackend for ArangoBackend {
    fn backend_name(&self) -> &str {
        "arango"
    }

    fn query_language(&self) -> cognigraph_core::QueryLanguage {
        cognigraph_core::QueryLanguage::Aql
    }

    async fn ping(&self) -> Result<()> {
        // AQL belongs inside this crate; callers should use ping(), not
        // hand-written probe queries.
        self.query("RETURN 1", std::collections::HashMap::new())
            .await
            .map(|_| ())
    }

    // --- Document operations ---

    async fn create_document(
        &self,
        collection: &str,
        doc: serde_json::Value,
    ) -> Result<DocumentId> {
        debug!(collection, "Creating document");
        let resp = self
            .create_with_implicit_collection(collection, &doc, CollectionType::Document)
            .await?;
        let key = extract_key(&resp)
            .ok_or_else(|| CogniGraphError::BackendError("No _key in response".into()))?;
        Ok(DocumentId::new(collection, key))
    }

    async fn get_document(&self, collection: &str, key: &str) -> Result<Option<serde_json::Value>> {
        debug!(collection, key, "Getting document");
        match self.client.get_document(collection, key).await {
            Ok(doc) => Ok(Some(doc)),
            Err(ArangoError::Server { code: 404, .. }) => Ok(None),
            Err(e) => Err(map_err(e)),
        }
    }

    async fn update_document(
        &self,
        collection: &str,
        key: &str,
        update: serde_json::Value,
    ) -> Result<serde_json::Value> {
        debug!(collection, key, "Updating document");
        let resp = self
            .client
            .update_document(collection, key, &update)
            .await
            .map_err(map_err)?;
        Ok(extract_doc(resp))
    }

    async fn replace_document(
        &self,
        collection: &str,
        key: &str,
        doc: serde_json::Value,
    ) -> Result<serde_json::Value> {
        debug!(collection, key, "Replacing document");
        let resp = self
            .client
            .replace_document(collection, key, &doc)
            .await
            .map_err(map_err)?;
        Ok(extract_doc(resp))
    }

    async fn delete_document(&self, collection: &str, key: &str) -> Result<bool> {
        debug!(collection, key, "Deleting document");
        match self.client.delete_document(collection, key).await {
            Ok(_) => Ok(true),
            Err(ArangoError::Server { code: 404, .. }) => Ok(false),
            Err(e) => Err(map_err(e)),
        }
    }

    async fn list_documents(
        &self,
        collection: &str,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<serde_json::Value>> {
        let aql = list_documents_aql(limit);
        let mut bind_vars = HashMap::new();
        bind_vars.insert("@collection".into(), serde_json::json!(collection));
        if let Some(limit) = limit {
            bind_vars.insert(
                "offset".into(),
                serde_json::json!(offset.unwrap_or_default()),
            );
            bind_vars.insert("limit".into(), serde_json::json!(limit));
        }

        let rows = self.client.query(aql, bind_vars).await.map_err(map_err)?;
        if limit.is_none() {
            Ok(rows.into_iter().skip(offset.unwrap_or_default()).collect())
        } else {
            Ok(rows)
        }
    }

    async fn list_documents_projected(
        &self,
        collection: &str,
        fields: &[String],
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<serde_json::Value>> {
        let aql = list_documents_projected_aql(limit);
        let mut projected = fields.to_vec();
        for field in ["_key", "_id"] {
            if !projected.iter().any(|existing| existing == field) {
                projected.push(field.into());
            }
        }
        let mut bind_vars = HashMap::new();
        bind_vars.insert("@collection".into(), serde_json::json!(collection));
        bind_vars.insert("fields".into(), serde_json::json!(projected));
        if let Some(limit) = limit {
            bind_vars.insert(
                "offset".into(),
                serde_json::json!(offset.unwrap_or_default()),
            );
            bind_vars.insert("limit".into(), serde_json::json!(limit));
        }

        let rows = self.client.query(aql, bind_vars).await.map_err(map_err)?;
        if limit.is_none() {
            Ok(rows.into_iter().skip(offset.unwrap_or_default()).collect())
        } else {
            Ok(rows)
        }
    }

    async fn list_documents_after_key(
        &self,
        collection: &str,
        after_key: Option<&str>,
        fields: &[String],
        limit: usize,
    ) -> Result<Vec<serde_json::Value>> {
        let mut projected = fields.to_vec();
        for field in ["_key", "_id"] {
            if !projected.iter().any(|existing| existing == field) {
                projected.push(field.into());
            }
        }
        let mut bind_vars = HashMap::new();
        bind_vars.insert("@collection".into(), serde_json::json!(collection));
        bind_vars.insert("fields".into(), serde_json::json!(projected));
        bind_vars.insert("limit".into(), serde_json::json!(limit));
        if let Some(after_key) = after_key {
            bind_vars.insert("after".into(), serde_json::json!(after_key));
        }

        self.client
            .query(list_documents_after_key_aql(after_key.is_some()), bind_vars)
            .await
            .map_err(map_err)
    }

    // --- Edge operations ---

    async fn create_edge(&self, collection: &str, edge: serde_json::Value) -> Result<DocumentId> {
        debug!(collection, "Creating edge");
        let resp = self
            .create_with_implicit_collection(collection, &edge, CollectionType::Edge)
            .await?;
        let key = extract_key(&resp)
            .ok_or_else(|| CogniGraphError::BackendError("No _key in response".into()))?;
        Ok(DocumentId::new(collection, key))
    }

    async fn upsert_edge(
        &self,
        collection: &str,
        from: &str,
        to: &str,
        relation_type: &str,
        data: serde_json::Value,
    ) -> Result<serde_json::Value> {
        debug!(collection, from, to, relation_type, "Upserting edge");

        // Merge _from, _to, relation_type into the data for INSERT
        let mut insert_doc = data.clone();
        if let Some(obj) = insert_doc.as_object_mut() {
            obj.insert("_from".into(), serde_json::json!(from));
            obj.insert("_to".into(), serde_json::json!(to));
            obj.insert("relation_type".into(), serde_json::json!(relation_type));
        }

        let aql = r#"
            UPSERT { _from: @from, _to: @to, relation_type: @relation_type }
            INSERT @insert_doc
            UPDATE MERGE(@data, { updated_at: DATE_NOW() / 1000 })
            IN @@collection
            RETURN NEW
        "#;

        let mut bind_vars = HashMap::new();
        bind_vars.insert("@collection".into(), serde_json::json!(collection));
        bind_vars.insert("from".into(), serde_json::json!(from));
        bind_vars.insert("to".into(), serde_json::json!(to));
        bind_vars.insert("relation_type".into(), serde_json::json!(relation_type));
        bind_vars.insert("insert_doc".into(), insert_doc);
        bind_vars.insert("data".into(), data);

        let mut results = match self.client.query(aql, bind_vars.clone()).await {
            Ok(results) => results,
            Err(error) if is_missing_collection_error(&error) => {
                self.ensure_collection(collection, CollectionType::Edge)
                    .await?;
                self.client.query(aql, bind_vars).await.map_err(map_err)?
            }
            Err(error) => return Err(map_err(error)),
        };
        results
            .pop()
            .ok_or_else(|| CogniGraphError::BackendError("Upsert returned no result".into()))
    }

    async fn get_edges(
        &self,
        collection: &str,
        vertex_id: &str,
        direction: Direction,
    ) -> Result<Vec<serde_json::Value>> {
        let aql = match direction {
            Direction::Outbound => "FOR e IN @@collection FILTER e._from == @vertex_id RETURN e",
            Direction::Inbound => "FOR e IN @@collection FILTER e._to == @vertex_id RETURN e",
            Direction::Any => {
                "FOR e IN @@collection FILTER e._from == @vertex_id OR e._to == @vertex_id RETURN e"
            }
        };

        let mut bind_vars = HashMap::new();
        bind_vars.insert("@collection".into(), serde_json::json!(collection));
        bind_vars.insert("vertex_id".into(), serde_json::json!(vertex_id));

        self.client.query(aql, bind_vars).await.map_err(map_err)
    }

    // --- Graph traversal ---

    async fn traverse(
        &self,
        start_vertex: &str,
        opts: &TraversalOpts,
    ) -> Result<Vec<TraversalPath>> {
        debug!(start_vertex, max_depth = opts.max_depth, direction = %opts.direction, "Traversing graph");

        let direction_keyword = match opts.direction {
            Direction::Outbound => "OUTBOUND",
            Direction::Inbound => "INBOUND",
            Direction::Any => "ANY",
        };

        // Prune at every hop, including before min_depth. A pruned endpoint
        // remains a traversal result unless also filtered. Depth zero has no
        // edge; missing/nonnumeric confidence defaults to 1, matching scoring.
        let confidence_filter = match opts.min_confidence {
            Some(_) => {
                "PRUNE rejected = e != null AND
                    (IS_NUMBER(e.confidence) ? e.confidence : 1) < @min_confidence
                 FILTER !rejected"
            }
            None => "",
        };

        let aql = format!(
            r#"
            FOR v, e, p IN @min_depth..@max_depth {direction_keyword} @start_vertex @@edge_collection
                {confidence_filter}
                RETURN {{
                    vertices: p.vertices,
                    edges: p.edges,
                    depth: LENGTH(p.edges)
                }}
            "#
        );

        let mut bind_vars = HashMap::new();
        bind_vars.insert("start_vertex".into(), serde_json::json!(start_vertex));
        bind_vars.insert(
            "@edge_collection".into(),
            serde_json::json!(opts.edge_collection),
        );
        bind_vars.insert("min_depth".into(), serde_json::json!(opts.min_depth));
        bind_vars.insert("max_depth".into(), serde_json::json!(opts.max_depth));

        if let Some(min_conf) = opts.min_confidence {
            bind_vars.insert("min_confidence".into(), serde_json::json!(min_conf));
        }

        let results = self.client.query(&aql, bind_vars).await.map_err(map_err)?;

        // Convert raw results into TraversalPath with computed scores
        let paths = results
            .into_iter()
            .filter_map(|row| {
                let vertices = row.get("vertices")?.as_array()?.clone();
                let edges = row.get("edges")?.as_array()?.clone();
                let depth = row.get("depth")?.as_u64()? as usize;

                let score = traversal_score(&edges, opts.path_decay);

                Some(TraversalPath {
                    vertices,
                    edges,
                    depth,
                    score,
                })
            })
            .collect();

        Ok(paths)
    }

    // --- Vector search ---

    async fn vector_search(
        &self,
        collection: &str,
        query_vector: &[f64],
        opts: &VectorSearchOpts,
    ) -> Result<Vec<SearchHit>> {
        self.search_vectors(collection, query_vector, opts).await
    }

    // --- Raw query ---

    async fn query(
        &self,
        query: &str,
        bind_vars: HashMap<String, serde_json::Value>,
    ) -> Result<Vec<serde_json::Value>> {
        self.client.query(query, bind_vars).await.map_err(map_err)
    }

    // --- Schema management ---

    async fn ensure_collection(&self, name: &str, collection_type: CollectionType) -> Result<()> {
        let type_code: u8 = match collection_type {
            CollectionType::Document => 2,
            CollectionType::Edge => 3,
        };

        match self.client.create_collection(name, type_code).await {
            Ok(_) => {
                debug!(name, "Collection created");
                Ok(())
            }
            // ArangoDB returns 409 if collection already exists
            Err(ArangoError::Server { code: 409, .. }) => {
                debug!(name, "Collection already exists");
                Ok(())
            }
            Err(e) => Err(map_err(e)),
        }
    }

    async fn ensure_index(&self, collection: &str, index: &IndexDef) -> Result<()> {
        let type_str = match index.index_type {
            IndexType::Persistent => "persistent",
            IndexType::Hash => "hash",
            IndexType::Fulltext => "fulltext",
            IndexType::Geo => "geo",
            IndexType::Ttl => "ttl",
            IndexType::Inverted => "inverted",
            IndexType::Vector => "vector",
        };

        let mut def = serde_json::json!({
            "type": type_str,
            "fields": index.fields,
            "unique": index.unique,
            "sparse": index.sparse,
        });

        if let Some(ref name) = index.name {
            def["name"] = serde_json::json!(name);
        }

        match self.client.create_index(collection, &def).await {
            Ok(_) => {
                debug!(collection, r#type = type_str, "Index created");
                Ok(())
            }
            // Index already exists — not an error
            Err(ArangoError::Server { code: 409, .. }) => Ok(()),
            Err(e) => Err(map_err(e)),
        }
    }

    async fn drop_collection(&self, name: &str) -> Result<()> {
        match self.client.drop_collection(name).await {
            Ok(_) => Ok(()),
            Err(ArangoError::Server { code: 404, .. }) => Ok(()),
            Err(e) => Err(map_err(e)),
        }
    }
}

impl ArangoBackend {
    async fn create_with_implicit_collection(
        &self,
        collection: &str,
        document: &serde_json::Value,
        collection_type: CollectionType,
    ) -> Result<serde_json::Value> {
        match self.client.create_document(collection, document).await {
            Ok(response) => Ok(response),
            Err(error) if is_missing_collection_error(&error) => {
                self.ensure_collection(collection, collection_type).await?;
                self.client
                    .create_document(collection, document)
                    .await
                    .map_err(map_err)
            }
            Err(error) => Err(map_err(error)),
        }
    }
}

fn is_missing_collection_error(error: &ArangoError) -> bool {
    let ArangoError::Server { code: 404, message } = error else {
        return false;
    };
    let message = message.to_ascii_lowercase();
    message.contains("collection not found") || message.contains("collection or view not found")
}

fn list_documents_aql(limit: Option<usize>) -> &'static str {
    if limit.is_some() {
        "FOR doc IN @@collection LIMIT @offset, @limit RETURN doc"
    } else {
        "FOR doc IN @@collection RETURN doc"
    }
}

fn list_documents_projected_aql(limit: Option<usize>) -> &'static str {
    if limit.is_some() {
        "FOR doc IN @@collection LIMIT @offset, @limit RETURN KEEP(doc, @fields)"
    } else {
        "FOR doc IN @@collection RETURN KEEP(doc, @fields)"
    }
}

fn list_documents_after_key_aql(has_after_key: bool) -> &'static str {
    if has_after_key {
        "FOR doc IN @@collection FILTER doc._key > @after SORT doc._key ASC LIMIT @limit RETURN KEEP(doc, @fields)"
    } else {
        "FOR doc IN @@collection SORT doc._key ASC LIMIT @limit RETURN KEEP(doc, @fields)"
    }
}

fn traversal_score(edges: &[serde_json::Value], path_decay: f64) -> f64 {
    let confidence_product = edges.iter().fold(1.0, |score, edge| {
        score
            * edge
                .get("confidence")
                .and_then(serde_json::Value::as_f64)
                .unwrap_or(1.0)
    });
    confidence_product * path_decay.powi(edges.len() as i32)
}

#[cfg(test)]
mod contract_regression_tests {
    use super::{
        is_missing_collection_error, list_documents_after_key_aql, list_documents_aql,
        list_documents_projected_aql, traversal_score,
    };
    use crate::client::ArangoError;

    #[test]
    fn implicit_creation_retry_is_limited_to_missing_collection_errors() {
        assert!(is_missing_collection_error(&ArangoError::Server {
            code: 404,
            message: "collection or view not found".into(),
        }));
        assert!(!is_missing_collection_error(&ArangoError::Server {
            code: 404,
            message: "document not found in collection".into(),
        }));
        assert!(!is_missing_collection_error(&ArangoError::Server {
            code: 409,
            message: "unique constraint violated".into(),
        }));
    }

    #[test]
    fn unbounded_list_query_has_no_implicit_limit() {
        assert_eq!(
            list_documents_aql(None),
            "FOR doc IN @@collection RETURN doc"
        );
        assert!(!list_documents_aql(None).contains("LIMIT"));
        assert!(list_documents_aql(Some(100)).contains("LIMIT"));
        assert!(list_documents_projected_aql(Some(100)).contains("KEEP(doc, @fields)"));
    }

    #[test]
    fn after_key_query_has_explicit_stable_order_and_projection() {
        let first = list_documents_after_key_aql(false);
        assert!(first.contains("SORT doc._key ASC"));
        assert!(first.contains("LIMIT @limit"));
        assert!(first.contains("KEEP(doc, @fields)"));
        assert!(!first.contains("@after"));

        let resumed = list_documents_after_key_aql(true);
        assert!(resumed.contains("FILTER doc._key > @after"));
        assert!(resumed.contains("SORT doc._key ASC"));
        assert!(resumed.contains("LIMIT @limit"));
        assert!(resumed.contains("KEEP(doc, @fields)"));
    }

    #[test]
    fn traversal_score_combines_confidence_and_per_hop_decay() {
        let edges = [
            serde_json::json!({"confidence": 0.9}),
            serde_json::json!({"confidence": 0.8}),
        ];
        assert!((traversal_score(&[], 0.8) - 1.0).abs() < 1e-12);
        assert!((traversal_score(&edges[..1], 0.8) - 0.72).abs() < 1e-12);
        assert!((traversal_score(&edges, 0.8) - 0.4608).abs() < 1e-12);
    }

    #[test]
    fn traversal_score_defaults_missing_confidence_to_one() {
        let edges = [
            serde_json::json!({}),
            serde_json::json!({"confidence": 0.5}),
        ];
        assert!((traversal_score(&edges, 0.8) - 0.32).abs() < 1e-12);
    }
}
