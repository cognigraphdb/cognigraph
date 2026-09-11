use super::*;
use cognigraph_core::CogniGraphError;
use cognigraph_query::{
    ExecutionBudget, ExecutionError, QueryMode, execute_backend_plan,
    parse_and_execute_backend_with_options, parse_and_plan,
};

const QUERIES: &[&str] = &[
    r#"RETURN DOCUMENT(@id)"#,
    r#"RETURN DOCUMENT(["persons/p1", @id, "persons/gone"])"#,
    r#"FOR e IN party FILTER DOCUMENT(@id).name == "absent" RETURN e._id"#,
    r#"FOR e IN party LET doc=DOCUMENT(@id) LET name=doc.name SORT e._id LIMIT 1 RETURN name"#,
    r#"LET doc=DOCUMENT("persons/p1") RETURN DOCUMENT(CONCAT("faults/", doc._id))"#,
    r#"FOR e IN party LET docs=(FOR n IN [1] RETURN DOCUMENT(@id)) SORT e._id LIMIT 1 RETURN docs"#,
    r#"FOR e IN party COLLECT kind=1 INTO docs=DOCUMENT(@id) RETURN docs"#,
];

#[tokio::test]
async fn document_failures_abort_normal_analyzed_and_compiled_queries() {
    let failures: [fn() -> CogniGraphError; 5] = [
        || CogniGraphError::Forbidden("synthetic denial".into()),
        || CogniGraphError::ConnectionError("synthetic outage".into()),
        || CogniGraphError::BackendError("synthetic storage failure".into()),
        || CogniGraphError::CollectionNotFound("faults".into()),
        || CogniGraphError::DocumentNotFound {
            collection: "faults".into(),
            key: "a".into(),
        },
    ];
    for failure in failures {
        let expected = match failure() {
            CogniGraphError::Forbidden(message) => ExecutionError::Forbidden(message),
            CogniGraphError::ConnectionError(message) => ExecutionError::Connection(message),
            other => ExecutionError::Backend(other.to_string()),
        };
        for query in QUERIES {
            let vars = if query.contains("@id") {
                binds([("id", json!("faults/a"))])
            } else {
                HashMap::new()
            };
            for prefix in ["", "EXPLAIN ANALYZE "] {
                let input = format!("{prefix}{query}");
                for mode in [QueryMode::ReadOnly, QueryMode::ReadWrite] {
                    let backend = DocumentLookupBackend {
                        lookup_failure: Some(failure),
                        ..Default::default()
                    };
                    let error = parse_and_execute_backend_with_options(
                        &input,
                        &backend,
                        &vars,
                        mode,
                        ExecutionBudget::default(),
                    )
                    .await
                    .unwrap_err();
                    assert_eq!(error, expected, "{input}");
                    assert_eq!(
                        backend
                            .fetches
                            .lock()
                            .unwrap()
                            .iter()
                            .filter(|id| id.starts_with("faults/"))
                            .count(),
                        1,
                        "a failed fetch must abort without retry: {input}"
                    );
                }
                let backend = DocumentLookupBackend {
                    lookup_failure: Some(failure),
                    ..Default::default()
                };
                let error = execute_backend_plan(&parse_and_plan(&input).unwrap(), &backend, &vars)
                    .await
                    .unwrap_err();
                assert_eq!(error, expected, "compiled: {input}");
            }
        }
    }
}

#[tokio::test]
async fn only_absence_and_non_identifiers_return_null() {
    let backend = DocumentLookupBackend {
        lookup_failure: Some(|| panic!("malformed ids must not reach the backend")),
        ..Default::default()
    };
    let query = r#"RETURN DOCUMENT(["faults", "", null, 42, {}, "persons/gone", "persons/p1"])"#;
    let rows = parse_and_execute_backend(query, &backend, &HashMap::new())
        .await
        .unwrap();
    assert!(rows[0].as_array().unwrap()[..6].iter().all(Value::is_null));
    assert_eq!(rows[0][6]["full_name"], "Ada Lovelace");
    assert_eq!(
        *backend.fetches.lock().unwrap(),
        ["persons/gone", "persons/p1"]
    );
    let analyzed = parse_and_execute_backend(
        &format!("EXPLAIN ANALYZE {query}"),
        &backend,
        &HashMap::new(),
    )
    .await
    .unwrap();
    assert_eq!(analyzed[0]["stats"]["result_rows"], 1);
    assert_eq!(analyzed[0]["stats"]["document_fetches"], 2);
}

#[tokio::test]
async fn failed_document_lookup_stops_the_batch_and_plain_explain_does_not_fetch() {
    for prefix in ["", "EXPLAIN ANALYZE "] {
        let backend = DocumentLookupBackend {
            lookup_failure: Some(|| {
                CogniGraphError::BackendError("synthetic storage failure".into())
            }),
            ..Default::default()
        };
        let query = r#"RETURN DOCUMENT(["faults/a", "faults/b", "persons/p1"])"#;
        parse_and_execute_backend(&format!("EXPLAIN {query}"), &backend, &HashMap::new())
            .await
            .unwrap();
        assert!(backend.fetches.lock().unwrap().is_empty());
        let error =
            parse_and_execute_backend(&format!("{prefix}{query}"), &backend, &HashMap::new())
                .await
                .unwrap_err();
        assert!(matches!(error, ExecutionError::Backend(_)));
        assert_eq!(*backend.fetches.lock().unwrap(), ["faults/a"]);
    }
}
