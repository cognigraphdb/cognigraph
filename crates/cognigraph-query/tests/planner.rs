use cognigraph_query::{
    Direction, Expr, LimitClause, PlanError, PlanSource, SortClause, SortDirection, SortKey,
    ValidationError, parse_and_plan,
};

#[test]
fn plans_collection_scan() {
    let plan = parse_and_plan(
        r#"
        FOR d IN documents
        FILTER d.category == @category
        SORT d.created_at DESC
        LIMIT 20
        RETURN { id: d._id, title: d.title }
        "#,
    )
    .unwrap();

    assert_eq!(plan.first_source(), Some(collection_source("documents")));
    assert_eq!(plan.filters().len(), 1);
    assert_eq!(
        plan.sort,
        Some(SortClause {
            collation: None,
            keys: vec![SortKey {
                expr: Expr::Identifier(vec!["d".into(), "created_at".into()]),
                direction: SortDirection::Desc
            }]
        })
    );
    assert_eq!(
        plan.limit,
        Some(LimitClause {
            offset: None,
            count: 20
        })
    );
    assert_eq!(plan.bind_vars, vec!["category"]);
}

#[test]
fn plans_vector_search() {
    let plan = parse_and_plan(
        r#"
        FOR d IN vector_search(documents, @embedding)
        LIMIT 10
        RETURN { document: d, score: d._score }
        "#,
    )
    .unwrap();

    assert_eq!(
        plan.first_source(),
        Some(PlanSource::VectorSearch {
            collection: "documents".into(),
            vector: Expr::BindVar("embedding".into())
        })
    );
    assert_eq!(plan.bind_vars, vec!["embedding"]);
}

#[test]
fn plans_traversal() {
    let plan = parse_and_plan(
        r#"
        FOR v, e, p IN 1..3 ANY @start relationships
        FILTER e.confidence >= @min_confidence
        RETURN { vertex: v, edge: e, path: p }
        "#,
    )
    .unwrap();

    assert_eq!(
        plan.first_source(),
        Some(PlanSource::Traversal {
            edge_var: "e".into(),
            path_var: "p".into(),
            min_depth: 1,
            max_depth: 3,
            direction: Direction::Any,
            start: Expr::BindVar("start".into()),
            edge_collection: "relationships".into()
        })
    );
    assert_eq!(plan.filters().len(), 1);
    assert_eq!(plan.bind_vars, vec!["min_confidence", "start"]);
}

#[test]
fn planner_rejects_semantically_invalid_query() {
    let err = parse_and_plan("FOR d IN documents RETURN missing").unwrap_err();

    assert_eq!(
        err,
        PlanError::Validation(ValidationError::UnknownIdentifier("missing".into()))
    );
}

#[test]
fn planner_rejects_syntactically_invalid_query() {
    let err = parse_and_plan("SELECT * FROM documents").unwrap_err();

    assert!(matches!(err, PlanError::Parse(_)));
}

#[test]
fn logical_plan_is_serializable() {
    let plan = parse_and_plan("FOR d IN documents LIMIT 5 RETURN d").unwrap();
    let json = serde_json::to_value(&plan).unwrap();

    assert_eq!(
        json,
        serde_json::json!({
            "body": [
                {
                    "For": {
                        "var": "d",
                        "source": { "CollectionScan": { "collection": "documents" } }
                    }
                }
            ],
            "distinct": false,
            "collect": null,
            "mutation": null,
            "sort": null,
            "limit": {
                "offset": null,
                "count": 5
            },
            "projection": {
                "Identifier": ["d"]
            },
            "bind_vars": [],
            "explain": false,
            "analyze": false
        })
    );
}

fn collection_source(collection: &str) -> PlanSource {
    PlanSource::CollectionScan {
        collection: collection.into(),
    }
}
