use std::collections::HashMap;

use async_trait::async_trait;
use cognigraph_core::{
    CollectionType, Direction, DocumentId, GraphBackend, IndexDef, QueryLanguage, Result,
    SearchHit, TraversalOpts, TraversalPath, VectorSearchOpts,
};
use cognigraph_query::parse_and_execute_backend;
use serde_json::{Value, json};

#[path = "backend_executor/document_errors.rs"]
mod document_errors;
#[path = "backend_executor/dynamic_analysis.rs"]
mod dynamic_analysis;
#[path = "backend_executor/limit_binds.rs"]
mod limit_binds;

#[tokio::test]
async fn executes_collection_query_against_graph_backend() {
    let backend = MockBackend;
    let rows = parse_and_execute_backend(
        r#"
        FOR d IN documents
        FILTER d.category == @category
        SORT d.title ASC
        RETURN d.title
        "#,
        &backend,
        &binds([("category", json!("research"))]),
    )
    .await
    .unwrap();

    assert_eq!(rows, vec![json!("Alpha"), json!("Beta")]);
}

#[tokio::test]
async fn executes_vector_query_against_graph_backend() {
    let backend = MockBackend;
    let rows = parse_and_execute_backend(
        r#"
        FOR d IN VECTOR_SEARCH(documents, @embedding)
        LIMIT 1
        RETURN { id: d._id, score: d._score }
        "#,
        &backend,
        &binds([("embedding", json!([1.0, 0.0]))]),
    )
    .await
    .unwrap();

    assert_eq!(rows, vec![json!({ "id": "documents/a", "score": 0.95 })]);
}

#[tokio::test]
async fn executes_traversal_query_against_graph_backend() {
    let backend = MockBackend;
    let rows = parse_and_execute_backend(
        r#"
        FOR v, e, p IN 1..2 OUTBOUND @start relationships
        RETURN { vertex: v._id, relation: e.relation_type, depth: p.depth }
        "#,
        &backend,
        &binds([("start", json!("documents/a"))]),
    )
    .await
    .unwrap();

    assert_eq!(
        rows,
        vec![json!({
            "vertex": "documents/b",
            "relation": "links",
            "depth": 1
        })]
    );
}

#[derive(Debug, Default)]
struct MockBackend;

#[async_trait]
impl GraphBackend for MockBackend {
    fn backend_name(&self) -> &str {
        "mock"
    }

    fn query_language(&self) -> QueryLanguage {
        QueryLanguage::Cgql
    }

    async fn create_document(&self, _collection: &str, _doc: Value) -> Result<DocumentId> {
        unimplemented!()
    }

    async fn get_document(&self, _collection: &str, _key: &str) -> Result<Option<Value>> {
        unimplemented!()
    }

    async fn update_document(
        &self,
        _collection: &str,
        _key: &str,
        _update: Value,
    ) -> Result<Value> {
        unimplemented!()
    }

    async fn replace_document(&self, _collection: &str, _key: &str, _doc: Value) -> Result<Value> {
        unimplemented!()
    }

    async fn delete_document(&self, _collection: &str, _key: &str) -> Result<bool> {
        unimplemented!()
    }

    async fn list_documents(
        &self,
        collection: &str,
        _limit: Option<usize>,
        _offset: Option<usize>,
    ) -> Result<Vec<Value>> {
        assert_eq!(collection, "documents");
        Ok(vec![
            json!({
                "_id": "documents/b",
                "title": "Beta",
                "category": "research"
            }),
            json!({
                "_id": "documents/a",
                "title": "Alpha",
                "category": "research"
            }),
            json!({
                "_id": "documents/c",
                "title": "Gamma",
                "category": "notes"
            }),
        ])
    }

    async fn create_edge(&self, _collection: &str, _edge: Value) -> Result<DocumentId> {
        unimplemented!()
    }

    async fn upsert_edge(
        &self,
        _collection: &str,
        _from: &str,
        _to: &str,
        _relation_type: &str,
        _data: Value,
    ) -> Result<Value> {
        unimplemented!()
    }

    async fn get_edges(
        &self,
        _collection: &str,
        _vertex_id: &str,
        _direction: Direction,
    ) -> Result<Vec<Value>> {
        unimplemented!()
    }

    async fn traverse(
        &self,
        start_vertex: &str,
        opts: &TraversalOpts,
    ) -> Result<Vec<TraversalPath>> {
        assert_eq!(start_vertex, "documents/a");
        assert_eq!(opts.edge_collection, "relationships");
        Ok(vec![TraversalPath {
            vertices: vec![json!({"_id": "documents/a"}), json!({"_id": "documents/b"})],
            edges: vec![json!({
                "_from": "documents/a",
                "_to": "documents/b",
                "relation_type": "links"
            })],
            depth: 1,
            score: 0.8,
        }])
    }

    async fn vector_search(
        &self,
        collection: &str,
        query_vector: &[f64],
        opts: &VectorSearchOpts,
    ) -> Result<Vec<SearchHit>> {
        assert_eq!(collection, "documents");
        assert_eq!(query_vector, &[1.0, 0.0]);
        assert_eq!(opts.limit, 1);
        Ok(vec![SearchHit {
            document: json!({
                "_id": "documents/a",
                "title": "Alpha"
            }),
            score: 0.95,
            source: Some("mock".into()),
        }])
    }

    async fn query(&self, _query: &str, _bind_vars: HashMap<String, Value>) -> Result<Vec<Value>> {
        unimplemented!()
    }

    async fn ensure_collection(&self, _name: &str, _collection_type: CollectionType) -> Result<()> {
        unimplemented!()
    }

    async fn ensure_index(&self, _collection: &str, _index: &IndexDef) -> Result<()> {
        unimplemented!()
    }

    async fn drop_collection(&self, _name: &str) -> Result<()> {
        unimplemented!()
    }
}

fn binds<const N: usize>(items: [(&str, Value); N]) -> HashMap<String, Value> {
    items
        .into_iter()
        .map(|(key, value)| (key.to_string(), value))
        .collect()
}

#[tokio::test]
async fn pushes_limit_down_for_plain_collection_scans() {
    let backend = RecordingBackend::default();
    parse_and_execute_backend(
        "FOR d IN documents LIMIT 3, 2 RETURN d",
        &backend,
        &HashMap::new(),
    )
    .await
    .unwrap();
    assert_eq!(*backend.last_list_limit.lock().unwrap(), Some(Some(5)));

    parse_and_execute_backend(
        "FOR d IN documents FILTER d.x == 1 LIMIT 2 RETURN d",
        &backend,
        &HashMap::new(),
    )
    .await
    .unwrap();
    assert_eq!(
        *backend.last_list_limit.lock().unwrap(),
        Some(None),
        "filters must disable limit pushdown"
    );

    parse_and_execute_backend(
        "FOR d IN documents SORT d.x ASC LIMIT 2 RETURN d",
        &backend,
        &HashMap::new(),
    )
    .await
    .unwrap();
    assert_eq!(
        *backend.last_list_limit.lock().unwrap(),
        Some(None),
        "sort must disable limit pushdown"
    );
}

/// Records the limit passed to `list_documents`.
#[derive(Default)]
struct RecordingBackend {
    last_list_limit: std::sync::Mutex<Option<Option<usize>>>,
}

#[async_trait]
impl GraphBackend for RecordingBackend {
    fn backend_name(&self) -> &str {
        "recording"
    }

    async fn create_document(&self, _: &str, _: Value) -> Result<DocumentId> {
        unimplemented!()
    }
    async fn get_document(&self, _: &str, _: &str) -> Result<Option<Value>> {
        unimplemented!()
    }
    async fn update_document(&self, _: &str, _: &str, _: Value) -> Result<Value> {
        unimplemented!()
    }
    async fn replace_document(&self, _: &str, _: &str, _: Value) -> Result<Value> {
        unimplemented!()
    }
    async fn delete_document(&self, _: &str, _: &str) -> Result<bool> {
        unimplemented!()
    }

    async fn list_documents(
        &self,
        _collection: &str,
        limit: Option<usize>,
        _offset: Option<usize>,
    ) -> Result<Vec<Value>> {
        *self.last_list_limit.lock().unwrap() = Some(limit);
        Ok(Vec::new())
    }

    async fn create_edge(&self, _: &str, _: Value) -> Result<DocumentId> {
        unimplemented!()
    }
    async fn upsert_edge(&self, _: &str, _: &str, _: &str, _: &str, _: Value) -> Result<Value> {
        unimplemented!()
    }
    async fn get_edges(&self, _: &str, _: &str, _: Direction) -> Result<Vec<Value>> {
        unimplemented!()
    }
    async fn traverse(&self, _: &str, _: &TraversalOpts) -> Result<Vec<TraversalPath>> {
        unimplemented!()
    }
    async fn vector_search(
        &self,
        _: &str,
        _: &[f64],
        _: &VectorSearchOpts,
    ) -> Result<Vec<SearchHit>> {
        unimplemented!()
    }
    async fn query(&self, _: &str, _: HashMap<String, Value>) -> Result<Vec<Value>> {
        unimplemented!()
    }
    async fn ensure_collection(&self, _: &str, _: CollectionType) -> Result<()> {
        unimplemented!()
    }
    async fn ensure_index(&self, _: &str, _: &IndexDef) -> Result<()> {
        unimplemented!()
    }
    async fn drop_collection(&self, _: &str) -> Result<()> {
        unimplemented!()
    }
}

#[tokio::test]
async fn execution_budgets_are_enforced() {
    use cognigraph_query::{ExecutionBudget, QueryMode, parse_and_execute_backend_with_options};
    let backend = MockBackend;

    // Row budget counts MATERIALIZED source rows. A non-pushable filter
    // (function call) materializes all 3 mock documents, exceeding 2.
    let err = parse_and_execute_backend_with_options(
        "FOR d IN documents FILTER CONTAINS(d.title, \"a\") RETURN d.title",
        &backend,
        &HashMap::new(),
        QueryMode::ReadOnly,
        ExecutionBudget {
            max_source_rows: Some(2),
            time_budget_ms: None,
        },
    )
    .await
    .unwrap_err();
    assert_eq!(
        err.to_string(),
        "query exceeded the source row budget (2 rows)"
    );

    // The same budget passes when the filter pushes down: only the 2
    // matching rows ever materialize (filter pushdown shrinks budget
    // pressure by design).
    parse_and_execute_backend_with_options(
        "FOR d IN documents FILTER d.category == \"research\" RETURN d.title",
        &backend,
        &HashMap::new(),
        QueryMode::ReadOnly,
        ExecutionBudget {
            max_source_rows: Some(2),
            time_budget_ms: None,
        },
    )
    .await
    .unwrap();

    // Within budget: identical query succeeds.
    let rows = parse_and_execute_backend_with_options(
        "FOR d IN documents FILTER d.category == \"research\" RETURN d.title",
        &backend,
        &HashMap::new(),
        QueryMode::ReadOnly,
        ExecutionBudget {
            max_source_rows: Some(10),
            time_budget_ms: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(rows.len(), 2);

    // Time budget of zero: the first per-row deadline check fires.
    let err = parse_and_execute_backend_with_options(
        "FOR d IN documents FILTER d.category == \"research\" RETURN d",
        &backend,
        &HashMap::new(),
        QueryMode::ReadOnly,
        ExecutionBudget {
            max_source_rows: None,
            time_budget_ms: Some(0),
        },
    )
    .await
    .unwrap_err();
    assert_eq!(err.to_string(), "query exceeded its time budget");
}

/// Edges whose targets live in two different collections, plus a fetch counter.
///
/// This is the shape that motivated `DOCUMENT()`: a party edge points at either
/// a person or an organization, and the two carry the display name under
/// different field names.
#[derive(Default)]
struct DocumentLookupBackend {
    fetches: std::sync::Mutex<Vec<String>>,
    traversals: std::sync::Mutex<Vec<String>>,
    /// Collection scans served — how many times the plan's head ran.
    scans: std::sync::Mutex<usize>,
    fetch_delay: Option<std::time::Duration>,
    lookup_failure: Option<fn() -> cognigraph_core::CogniGraphError>,
}

#[async_trait]
impl GraphBackend for DocumentLookupBackend {
    fn backend_name(&self) -> &str {
        "document-lookup"
    }

    async fn get_document(&self, collection: &str, key: &str) -> Result<Option<Value>> {
        self.fetches
            .lock()
            .unwrap()
            .push(format!("{collection}/{key}"));
        if let Some(delay) = self.fetch_delay {
            tokio::time::sleep(delay).await;
        }
        if collection == "faults"
            && let Some(failure) = self.lookup_failure
        {
            return Err(failure());
        }
        Ok(match (collection, key) {
            ("chain", key) => Some(json!({
                "_id":format!("chain/{key}"),
                "ref":format!("chain/{}", key.parse::<usize>().unwrap() + 1)
            })),
            ("persons", "p1") => Some(json!({
                "_id": "persons/p1", "full_name": "Ada Lovelace", "specialty": "Oncology",
                "ref": "organizations/o1"
            })),
            ("persons", "p2") => Some(json!({
                "_id": "persons/p2", "full_name": "Alan Turing", "specialty": "Cardiology"
            })),
            ("organizations", "o1") => Some(json!({
                "_id": "organizations/o1", "name": "Mercy Clinic"
            })),
            _ => None,
        })
    }

    async fn list_documents(
        &self,
        collection: &str,
        _limit: Option<usize>,
        _offset: Option<usize>,
    ) -> Result<Vec<Value>> {
        assert_eq!(collection, "party");
        *self.scans.lock().unwrap() += 1;
        Ok(vec![
            json!({ "_id": "party/1", "_from": "contracts/c1", "_to": "persons/p1" }),
            json!({ "_id": "party/2", "_from": "contracts/c1", "_to": "organizations/o1" }),
            json!({ "_id": "party/3", "_from": "contracts/c2", "_to": "persons/p2" }),
            // Repeats p1: the batch must not fetch it twice.
            json!({ "_id": "party/4", "_from": "contracts/c2", "_to": "persons/p1" }),
            // Dangling and malformed targets both answer null.
            json!({ "_id": "party/5", "_from": "contracts/c3", "_to": "persons/gone" }),
            json!({ "_id": "party/6", "_from": "contracts/c3", "_to": "not-an-id" }),
        ])
    }

    async fn create_document(&self, _: &str, _: Value) -> Result<DocumentId> {
        unimplemented!()
    }
    async fn update_document(&self, _: &str, _: &str, _: Value) -> Result<Value> {
        unimplemented!()
    }
    async fn replace_document(&self, _: &str, _: &str, _: Value) -> Result<Value> {
        unimplemented!()
    }
    async fn delete_document(&self, _: &str, _: &str) -> Result<bool> {
        unimplemented!()
    }
    async fn create_edge(&self, _: &str, _: Value) -> Result<DocumentId> {
        unimplemented!()
    }
    async fn upsert_edge(&self, _: &str, _: &str, _: &str, _: &str, _: Value) -> Result<Value> {
        unimplemented!()
    }
    async fn get_edges(&self, _: &str, _: &str, _: Direction) -> Result<Vec<Value>> {
        unimplemented!()
    }
    async fn traverse(&self, start: &str, opts: &TraversalOpts) -> Result<Vec<TraversalPath>> {
        self.traversals.lock().unwrap().push(start.to_string());
        assert_eq!(opts.edge_collection, "party");
        let edges = self.list_documents("party", None, None).await?;
        let mut paths = Vec::new();
        for edge in edges {
            if edge.get("_from").and_then(Value::as_str) != Some(start) {
                continue;
            }
            let to = edge.get("_to").and_then(Value::as_str).unwrap_or_default();
            let (collection, key) = to.split_once('/').unwrap_or(("", ""));
            let vertex = self
                .get_document(collection, key)
                .await?
                .unwrap_or(Value::Null);
            paths.push(TraversalPath {
                vertices: vec![vertex],
                edges: vec![edge],
                depth: 1,
                score: 1.0,
            });
        }
        Ok(paths)
    }
    async fn vector_search(
        &self,
        _: &str,
        _: &[f64],
        _: &VectorSearchOpts,
    ) -> Result<Vec<SearchHit>> {
        unimplemented!()
    }
    async fn query(&self, _: &str, _: HashMap<String, Value>) -> Result<Vec<Value>> {
        unimplemented!()
    }
    async fn ensure_collection(&self, _: &str, _: CollectionType) -> Result<()> {
        unimplemented!()
    }
    async fn ensure_index(&self, _: &str, _: &IndexDef) -> Result<()> {
        unimplemented!()
    }
    async fn drop_collection(&self, _: &str) -> Result<()> {
        unimplemented!()
    }
}

#[tokio::test]
async fn correlated_traversal_starts_from_the_enclosing_row() {
    let backend = DocumentLookupBackend::default();
    // Four party edges name only two distinct contracts, so a traversal per
    // outer ROW would call the backend four times. One per distinct START is
    // the point of the batch.
    let rows = parse_and_execute_backend(
        r#"
        FOR e IN party
        FILTER e._to != "not-an-id"
        FOR v IN 1..1 OUTBOUND e._from party
        RETURN COALESCE(v.full_name, v.name)
        "#,
        &backend,
        &HashMap::new(),
    )
    .await
    .unwrap();

    // The FILTER drops one of the 6 edges, leaving 5 outer rows. Each contract
    // has exactly 2 outgoing edges — including c3, whose second edge the FILTER
    // removed from the OUTER side only, not from the traversal — so 5 x 2.
    assert_eq!(rows.len(), 10);

    let mut starts = backend.traversals.lock().unwrap().clone();
    starts.sort();
    starts.dedup();
    assert_eq!(
        starts,
        vec![
            "contracts/c1".to_string(),
            "contracts/c2".to_string(),
            "contracts/c3".to_string()
        ]
    );
    assert_eq!(
        backend.traversals.lock().unwrap().len(),
        3,
        "one backend traversal per distinct start, not per outer row"
    );
}

#[tokio::test]
async fn document_lookup_resolves_across_collections_on_the_backend() {
    let backend = DocumentLookupBackend::default();
    let rows = parse_and_execute_backend(
        r#"
        FOR e IN party
        LET d = DOCUMENT(e._to)
        RETURN COALESCE(d.full_name, d.name)
        "#,
        &backend,
        &HashMap::new(),
    )
    .await
    .unwrap();

    assert_eq!(
        rows,
        vec![
            json!("Ada Lovelace"),
            json!("Mercy Clinic"),
            json!("Alan Turing"),
            json!("Ada Lovelace"),
            Value::Null, // dangling id
            Value::Null, // not an identifier
        ]
    );

    // Each distinct id is fetched once for the whole query, and a target that
    // is not an identifier is never sent to the backend at all.
    let fetches = backend.fetches.lock().unwrap().clone();
    assert_eq!(
        fetches,
        vec![
            "organizations/o1".to_string(),
            "persons/gone".to_string(),
            "persons/p1".to_string(),
            "persons/p2".to_string(),
        ],
        "distinct ids fetched exactly once, in one batch"
    );
}

#[tokio::test]
async fn document_lookup_works_in_filter_and_collect() {
    let backend = DocumentLookupBackend::default();
    let rows = parse_and_execute_backend(
        r#"
        FOR e IN party
        FILTER DOCUMENT(e._to).specialty == "Oncology"
        COLLECT WITH COUNT INTO n
        RETURN { oncology: n }
        "#,
        &backend,
        &HashMap::new(),
    )
    .await
    .unwrap();
    assert_eq!(rows, vec![json!({ "oncology": 2 })]);
}

#[tokio::test]
async fn document_lookup_converges_when_nested() {
    let backend = DocumentLookupBackend::default();
    // The inner lookup produces the id the outer one needs, so this only
    // resolves if the fetch-and-retry loop runs more than one round.
    let rows = parse_and_execute_backend(
        r#"
        FOR e IN party
        LET again = DOCUMENT(DOCUMENT(e._to)._id)
        FILTER again != null
        RETURN again._id
        "#,
        &backend,
        &HashMap::new(),
    )
    .await
    .unwrap();
    assert_eq!(
        rows,
        vec![
            json!("persons/p1"),
            json!("organizations/o1"),
            json!("persons/p2"),
            json!("persons/p1"),
        ]
    );
}

#[tokio::test]
async fn deferred_document_lookups_fetch_only_surviving_rows() {
    let backend = DocumentLookupBackend::default();
    // `d` decorates the projection only, so it runs after LIMIT: of the six
    // party edges, exactly two survive, and only their targets may be
    // fetched. Without deferral this fetched every distinct target.
    let rows = parse_and_execute_backend(
        r#"
        FOR e IN party
        LET d = DOCUMENT(e._to)
        SORT e._id ASC
        LIMIT 2
        RETURN COALESCE(d.full_name, d.name)
        "#,
        &backend,
        &HashMap::new(),
    )
    .await
    .unwrap();

    assert_eq!(rows, vec![json!("Ada Lovelace"), json!("Mercy Clinic")]);
    let fetches = backend.fetches.lock().unwrap().clone();
    assert_eq!(
        fetches,
        vec!["organizations/o1".to_string(), "persons/p1".to_string()],
        "only the two surviving rows' targets are fetched"
    );
    // The head cannot miss (no DOCUMENT before the deferral split), so the
    // scan+sort+limit ran once; only the cheap tail retried.
    assert_eq!(
        *backend.scans.lock().unwrap(),
        1,
        "head must run exactly once"
    );
}

#[tokio::test]
async fn projection_only_document_reads_do_not_rerun_the_head() {
    let backend = DocumentLookupBackend::default();
    // DOCUMENT appears only in RETURN: the head (scan) settles in one pass and
    // the retry rounds replay the projection alone.
    let rows = parse_and_execute_backend(
        r#"
        FOR e IN party
        FILTER e._to != "not-an-id"
        RETURN DOCUMENT(e._to)._id
        "#,
        &backend,
        &HashMap::new(),
    )
    .await
    .unwrap();
    assert_eq!(rows.len(), 5);
    assert_eq!(rows[0], json!("persons/p1"));
    assert_eq!(
        *backend.scans.lock().unwrap(),
        1,
        "head must run exactly once"
    );
}

#[tokio::test]
async fn nested_deferred_document_converges_without_rerunning_the_head() {
    let backend = DocumentLookupBackend::default();
    // Two dependent fetch generations, both confined to a deferred LET: the
    // tail loops twice, the head still runs once.
    let rows = parse_and_execute_backend(
        r#"
        FOR e IN party
        LET again = DOCUMENT(DOCUMENT(e._to)._id)
        SORT e._id ASC
        LIMIT 2
        RETURN again._id
        "#,
        &backend,
        &HashMap::new(),
    )
    .await
    .unwrap();
    assert_eq!(rows, vec![json!("persons/p1"), json!("organizations/o1")]);
    assert_eq!(
        *backend.scans.lock().unwrap(),
        1,
        "head must run exactly once"
    );
}
