use super::*;
use cognigraph_query::{
    ExecutionBudget, ExecutionError, QueryMode, execute_backend_plan,
    parse_and_execute_backend_with_options, parse_and_plan,
};

fn stage<'a>(report: &'a Value, label: &str) -> &'a Value {
    report["stages"]
        .as_array()
        .unwrap()
        .iter()
        .rev()
        .find(|s| s["stage"] == label)
        .unwrap()
}

const FILTER: &str =
    r#"FOR e IN party FILTER DOCUMENT(e._to).specialty == "Oncology" RETURN e._id"#;
const DEFERRED: &str = r#"FOR e IN party LET d=DOCUMENT(e._to) LET label=COALESCE(d.full_name,d.name) SORT e._id LIMIT 2 RETURN label"#;
const DEPENDENT: &str = r#"FOR e IN party FILTER e._to == "persons/p1" LET d=DOCUMENT(e._to) LET org=DOCUMENT(d.ref) RETURN org.name"#;
const TRAVERSAL: &str = r#"FOR e IN party FILTER e._from == "contracts/c1" FOR v IN 1..1 OUTBOUND e._from party RETURN COALESCE(v.full_name,v.name)"#;
const MIXED: &str = r#"FOR e IN party FILTER e._from == "contracts/c1" FOR v IN 1..1 OUTBOUND e._from party LET d=DOCUMENT(v._id) RETURN COALESCE(d.full_name,d.name)"#;
const ARRAY: &str = r#"FOR d IN DOCUMENT(["persons/p1","persons/p2"]) RETURN d.full_name"#;
const SUBQUERY: &str = r#"FOR e IN party FILTER e._from == "contracts/c1" LET xs=(FOR n IN [1,2] RETURN DOCUMENT(e._to)._id) SORT e._id LIMIT 1 RETURN xs"#;
const INTO: &str = r#"FOR e IN party COLLECT kind=1 INTO labels=DOCUMENT(e._to)._id RETURN labels"#;

#[tokio::test]
async fn nested_lifts_preserve_dynamic_reads_analysis_and_row_budgets() {
    let query = r#"
        LET n = LENGTH((FOR e IN party RETURN 1))
        RETURN {n, labels: (FOR p IN ["persons/p1", "persons/p2"]
            RETURN FIRST((RETURN DOCUMENT(p).full_name)))}
    "#;
    let expected = vec![json!({"n": 6, "labels": ["Ada Lovelace", "Alan Turing"]})];
    let rows = execute_backend_plan(
        &parse_and_plan(query).unwrap(),
        &DocumentLookupBackend::default(),
        &HashMap::new(),
    )
    .await
    .unwrap();
    assert_eq!(rows, expected);

    let analyzed = format!("EXPLAIN ANALYZE {query}");
    let report = parse_and_execute_backend(
        &analyzed,
        &DocumentLookupBackend::default(),
        &HashMap::new(),
    )
    .await
    .unwrap();
    assert_eq!(report[0]["stats"]["result_rows"], 1);
    assert_eq!(report[0]["stats"]["document_fetches"], 2);
    let cost = report[0]["stats"]["source_rows"].as_u64().unwrap();
    assert!(cost > 1);
    for input in [query, &analyzed] {
        for cap in [cost - 1, cost] {
            let result = parse_and_execute_backend_with_options(
                input,
                &DocumentLookupBackend::default(),
                &HashMap::new(),
                QueryMode::ReadOnly,
                ExecutionBudget {
                    max_source_rows: Some(cap),
                    time_budget_ms: None,
                },
            )
            .await;
            if cap < cost {
                assert_eq!(result.unwrap_err(), ExecutionError::RowBudgetExceeded(cap));
            } else if input == query {
                assert_eq!(result.unwrap(), expected);
            } else {
                assert_eq!(result.unwrap()[0]["stats"]["source_rows"], cost);
            }
        }
    }
}

#[tokio::test]
async fn analyzed_backend_reads_match_normal_execution_and_compiled_plans() {
    for (query, count) in [
        (FILTER, 2),
        (DEFERRED, 2),
        (DEPENDENT, 2),
        (TRAVERSAL, 4),
        (MIXED, 4),
        (ARRAY, 2),
        (SUBQUERY, 1),
        (INTO, 1),
    ] {
        let rows =
            parse_and_execute_backend(query, &DocumentLookupBackend::default(), &HashMap::new())
                .await
                .unwrap();
        assert_eq!(rows.len(), count, "{query}");
        if query == INTO {
            assert_eq!(
                rows,
                vec![json!([
                    "persons/p1",
                    "organizations/o1",
                    "persons/p2",
                    "persons/p1",
                    null,
                    null
                ])]
            );
        }
        let input = format!("EXPLAIN ANALYZE {query}");
        let backend = DocumentLookupBackend::default();
        let report = parse_and_execute_backend(&input, &backend, &HashMap::new())
            .await
            .unwrap();
        assert_eq!(report[0]["stats"]["result_rows"], count, "{query}");
        assert_eq!(stage(&report[0], "RETURN")["rows"], count, "{query}");
        let planned = execute_backend_plan(
            &parse_and_plan(&input).unwrap(),
            &DocumentLookupBackend::default(),
            &HashMap::new(),
        )
        .await
        .unwrap();
        assert_eq!(
            planned[0]["stats"]["result_rows"], count,
            "compiled: {query}"
        );
    }
}

#[tokio::test]
async fn statistics_separate_settled_rows_from_retry_work() {
    let backend = DocumentLookupBackend::default();
    let report = parse_and_execute_backend(
        &format!("EXPLAIN ANALYZE {FILTER}"),
        &backend,
        &HashMap::new(),
    )
    .await
    .unwrap();
    let report = &report[0];
    assert_eq!(stage(report, "FOR e")["rows"], 6);
    assert_eq!(stage(report, "FOR e")["runs"], 1);
    assert_eq!(stage(report, "FOR e")["attempted_rows"], 12);
    assert_eq!(stage(report, "FOR e")["attempted_runs"], 2);
    assert_eq!(stage(report, "FILTER")["rows"], 2);
    assert_eq!(report["sources"][0]["rows"], 6);
    assert_eq!(report["sources"][0]["fetches"], 1);
    assert_eq!(report["stats"]["source_rows"], 10);
    assert_eq!(report["stats"]["document_fetches"], 4);
    assert_eq!(*backend.scans.lock().unwrap(), 1);
    assert_eq!(backend.fetches.lock().unwrap().len(), 4);
}

#[tokio::test]
async fn deferred_retries_keep_settled_head_and_nested_stage_counts() {
    for (query, for_label, logical, attempted, count) in
        [(DEFERRED, "LET d", 2, 4, 2), (SUBQUERY, "FOR n", 2, 4, 1)]
    {
        let report = parse_and_execute_backend(
            &format!("EXPLAIN ANALYZE {query}"),
            &DocumentLookupBackend::default(),
            &HashMap::new(),
        )
        .await
        .unwrap();
        let report = &report[0];
        assert_eq!(stage(report, "FOR e")["attempted_runs"], 1, "{query}");
        assert_eq!(stage(report, for_label)["rows"], logical, "{query}");
        assert_eq!(
            stage(report, for_label)["attempted_rows"],
            attempted,
            "{query}"
        );
        assert_eq!(report["stats"]["result_rows"], count, "{query}");
    }
}

#[tokio::test]
async fn dynamic_budget_spans_materialization_fetches_and_all_rounds() {
    // Scan rows count once. Each lookup costs one unit; correlated paths cost
    // their fetch plus each expansion; expression arrays cost each expansion.
    for (query, cost) in [
        (FILTER, 10),
        (DEFERRED, 8),
        (DEPENDENT, 4),
        (TRAVERSAL, 8),
        (MIXED, 14),
        (ARRAY, 6),
        (SUBQUERY, 7),
        (INTO, 10),
    ] {
        for analyze in [false, true] {
            let query = if analyze {
                format!("EXPLAIN ANALYZE {query}")
            } else {
                query.into()
            };
            for cap in [cost - 1, cost] {
                let result = parse_and_execute_backend_with_options(
                    &query,
                    &DocumentLookupBackend::default(),
                    &HashMap::new(),
                    QueryMode::ReadOnly,
                    ExecutionBudget {
                        max_source_rows: Some(cap),
                        time_budget_ms: None,
                    },
                )
                .await;
                if cap < cost {
                    assert_eq!(
                        result.unwrap_err(),
                        ExecutionError::RowBudgetExceeded(cap),
                        "{query}"
                    );
                } else {
                    let rows = result.unwrap_or_else(|e| panic!("{query}: {e}"));
                    if analyze {
                        assert_eq!(rows[0]["stats"]["source_rows"], cost, "{query}");
                    }
                }
            }
        }
    }
}

#[tokio::test]
async fn lookup_deduplication_and_malformed_ids_have_explicit_costs() {
    for (ids, cost) in [
        (r#"["persons/gone","persons/gone","not-an-id",null]"#, 1),
        (r#"["not-an-id",42,null]"#, 0),
        (r#"["persons/p1","persons/p1"]"#, 1),
    ] {
        let backend = DocumentLookupBackend::default();
        let report = parse_and_execute_backend_with_options(
            &format!("EXPLAIN ANALYZE RETURN DOCUMENT({ids})"),
            &backend,
            &HashMap::new(),
            QueryMode::ReadOnly,
            ExecutionBudget {
                max_source_rows: Some(cost),
                time_budget_ms: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(report[0]["stats"]["source_rows"], cost);
        assert_eq!(report[0]["stats"]["document_fetches"], cost);
        assert_eq!(report[0]["stats"]["result_rows"], 1);
        assert_eq!(backend.fetches.lock().unwrap().len(), cost as usize);
    }
}

#[tokio::test]
async fn correlated_traversal_fetch_statistics_count_distinct_starts() {
    let backend = DocumentLookupBackend::default();
    let report = parse_and_execute_backend(
        &format!("EXPLAIN ANALYZE {MIXED}"),
        &backend,
        &HashMap::new(),
    )
    .await
    .unwrap();
    assert_eq!(report[0]["sources"][1]["rows"], 2);
    assert_eq!(report[0]["sources"][1]["fetches"], 1);
    assert!(report[0]["sources"][1]["fetch_ms"].is_number());
    assert_eq!(stage(&report[0], "FOR v")["rows"], 4);
    assert_eq!(stage(&report[0], "FOR v")["attempted_rows"], 8);
    assert_eq!(backend.traversals.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn document_resolution_preserves_deadline_and_convergence_errors_under_analysis() {
    for prefix in ["", "EXPLAIN ANALYZE "] {
        let backend = DocumentLookupBackend {
            fetch_delay: Some(std::time::Duration::from_millis(50)),
            ..Default::default()
        };
        let err = parse_and_execute_backend_with_options(
            &format!(r#"{prefix}RETURN DOCUMENT(["persons/p1","persons/p2"])"#),
            &backend,
            &HashMap::new(),
            QueryMode::ReadOnly,
            ExecutionBudget {
                max_source_rows: None,
                time_budget_ms: Some(20),
            },
        )
        .await
        .unwrap_err();
        assert_eq!(err, ExecutionError::TimeBudgetExceeded);
        assert!(
            backend.fetches.lock().unwrap().len() <= 1,
            "deadline stops the lookup batch"
        );
        let err = parse_and_execute_backend(
            &format!(r#"{prefix}RETURN DOCUMENT(DOCUMENT(DOCUMENT(DOCUMENT(DOCUMENT("chain/0").ref).ref).ref).ref)._id"#),
            &DocumentLookupBackend::default(), &HashMap::new(),
        ).await.unwrap_err();
        assert!(
            err.to_string()
                .contains("did not resolve within the allowed rounds")
        );
    }
}

#[tokio::test]
async fn document_budget_stops_fetching_at_the_limit_including_missing_ids() {
    for analyze in [false, true] {
        let prefix = if analyze { "EXPLAIN ANALYZE " } else { "" };
        let backend = DocumentLookupBackend::default();
        let result = parse_and_execute_backend_with_options(
            &format!(r#"{prefix}RETURN DOCUMENT(["persons/gone","persons/p1"] )"#),
            &backend,
            &HashMap::new(),
            QueryMode::ReadOnly,
            ExecutionBudget {
                max_source_rows: Some(1),
                time_budget_ms: None,
            },
        )
        .await;
        assert_eq!(result.unwrap_err(), ExecutionError::RowBudgetExceeded(1));
        assert_eq!(*backend.fetches.lock().unwrap(), vec!["persons/gone"]);
    }
}
