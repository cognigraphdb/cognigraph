use cognigraph_query::{
    BinaryOp, Direction, Expr, ForClause, LimitClause, LimitValue, ObjectField, SortDirection,
    UnaryOp, parse_query,
};

#[test]
fn parses_collection_query() {
    let query = parse_query(
        r#"
        FOR d IN documents
        FILTER d.category == @category
        SORT d.created_at DESC
        LIMIT 20
        RETURN d
        "#,
    )
    .unwrap();

    assert_eq!(
        query.for_clause(),
        Some(ForClause::Collection {
            var: "d".into(),
            collection: "documents".into()
        })
    );
    assert_eq!(query.filters().len(), 1);
    assert_eq!(query.sort.unwrap().keys[0].direction, SortDirection::Desc);
    assert_eq!(
        query.limit,
        Some(LimitClause {
            offset: None,
            count: LimitValue::Literal(20)
        })
    );
    assert_eq!(
        query.return_expr.unwrap(),
        Expr::Identifier(vec!["d".into()])
    );
    assert_eq!(query.bind_vars, vec!["category"]);
}

#[test]
fn parses_lowercase_keywords() {
    let query = parse_query(
        r#"
        for d in documents
        filter d.category in @categories
        sort d.created_at desc
        limit 20
        return d
        "#,
    )
    .unwrap();

    assert_eq!(
        query.for_clause(),
        Some(ForClause::Collection {
            var: "d".into(),
            collection: "documents".into()
        })
    );
    assert_eq!(query.sort.unwrap().keys[0].direction, SortDirection::Desc);
    assert_eq!(query.bind_vars, vec!["categories"]);
}

#[test]
fn parses_mixed_case_keywords() {
    let query = parse_query(
        r#"
        FoR d In documents
        FiLtEr NoT d.deleted AnD d.active == TrUe
        ReTuRn { doc: d, missing: NuLl }
        "#,
    )
    .unwrap();

    assert_eq!(query.filters().len(), 1);
    assert!(matches!(
        query.return_expr.unwrap(),
        Expr::Object(ref fields) if fields.len() == 2
    ));
}

#[test]
fn keeps_identifiers_case_sensitive() {
    let query = parse_query("FOR Doc IN Documents RETURN Doc").unwrap();

    assert_eq!(
        query.for_clause(),
        Some(ForClause::Collection {
            var: "Doc".into(),
            collection: "Documents".into()
        })
    );
    assert_eq!(
        query.return_expr.unwrap(),
        Expr::Identifier(vec!["Doc".into()])
    );
}

#[test]
fn parses_limit_with_offset() {
    let query = parse_query("FOR d IN documents LIMIT 10, 25 RETURN d").unwrap();

    assert_eq!(
        query.limit,
        Some(LimitClause {
            offset: Some(LimitValue::Literal(10)),
            count: LimitValue::Literal(25)
        })
    );
}

#[test]
fn parses_traversal_query() {
    let query = parse_query(
        r#"
        FOR v, e, p IN 1..3 OUTBOUND @start relationships
        FILTER e.confidence >= @min_confidence
        RETURN { vertex: v, edge: e, path: p }
        "#,
    )
    .unwrap();

    assert_eq!(
        query.for_clause(),
        Some(ForClause::Traversal {
            vertex_var: "v".into(),
            edge_var: "e".into(),
            path_var: "p".into(),
            min_depth: 1,
            max_depth: 3,
            direction: Direction::Outbound,
            start: Expr::BindVar("start".into()),
            edge_collection: "relationships".into()
        })
    );
    assert_eq!(query.bind_vars, vec!["min_confidence", "start"]);
}

#[test]
fn parses_any_direction_traversal() {
    let query =
        parse_query("FOR v, e, p IN 0..2 ANY \"documents/a\" relationships RETURN v").unwrap();

    match query.for_clause() {
        Some(ForClause::Traversal {
            direction,
            min_depth,
            max_depth,
            ..
        }) => {
            assert_eq!(direction, Direction::Any);
            assert_eq!(min_depth, 0);
            assert_eq!(max_depth, 2);
        }
        other => panic!("expected traversal query, got {other:?}"),
    }
}

#[test]
fn parses_vector_source() {
    let query = parse_query(
        r#"
        FOR d IN VECTOR_SEARCH(documents, @embedding)
        LIMIT 10
        RETURN { document: d, score: d._score }
        "#,
    )
    .unwrap();

    assert_eq!(
        query.for_clause(),
        Some(ForClause::VectorSearch {
            var: "d".into(),
            collection: "documents".into(),
            vector: Expr::BindVar("embedding".into())
        })
    );
    assert_eq!(query.bind_vars, vec!["embedding"]);
}

#[test]
fn parses_lowercase_vector_source_keyword() {
    let query = parse_query("for d in vector_search(documents, @embedding) return d").unwrap();

    assert_eq!(
        query.for_clause(),
        Some(ForClause::VectorSearch {
            var: "d".into(),
            collection: "documents".into(),
            vector: Expr::BindVar("embedding".into())
        })
    );
}

#[test]
fn parses_object_projection() {
    let query = parse_query("FOR d IN documents RETURN { id: d._id, title: d.title }").unwrap();

    assert_eq!(
        query.return_expr.unwrap(),
        Expr::Object(vec![
            ObjectField {
                name: "id".into(),
                value: Expr::Identifier(vec!["d".into(), "_id".into()])
            },
            ObjectField {
                name: "title".into(),
                value: Expr::Identifier(vec!["d".into(), "title".into()])
            }
        ])
    );
}

#[test]
fn parses_boolean_expression() {
    let query = parse_query(
        "FOR d IN documents FILTER d.active == true AND d.category IN @categories RETURN d",
    )
    .unwrap();

    assert!(matches!(
        query.filters().first(),
        Some(Expr::Binary {
            op: BinaryOp::And,
            ..
        })
    ));
    assert_eq!(query.bind_vars, vec!["categories"]);
}

#[test]
fn parses_literal_values() {
    let query = parse_query(
        r#"FOR d IN documents RETURN { active: true, missing: null, label: "line\none" }"#,
    )
    .unwrap();

    assert_eq!(
        query.return_expr.unwrap(),
        Expr::Object(vec![
            ObjectField {
                name: "active".into(),
                value: Expr::Bool(true)
            },
            ObjectField {
                name: "missing".into(),
                value: Expr::Null
            },
            ObjectField {
                name: "label".into(),
                value: Expr::String("line\none".into())
            }
        ])
    );
}

#[test]
fn preserves_operator_precedence() {
    let query = parse_query("FOR d IN documents FILTER d.a == 1 OR d.b == 2 AND d.c == 3 RETURN d")
        .unwrap();

    match query.filters().first().unwrap() {
        Expr::Binary {
            op: BinaryOp::Or,
            right,
            ..
        } => assert!(matches!(
            right.as_ref(),
            Expr::Binary {
                op: BinaryOp::And,
                ..
            }
        )),
        other => panic!("expected OR expression, got {other:?}"),
    }
}

#[test]
fn parses_parenthesized_expression() {
    let query =
        parse_query("FOR d IN documents FILTER (d.a == 1 OR d.b == 2) AND d.c == 3 RETURN d")
            .unwrap();

    match query.filters().first().unwrap() {
        Expr::Binary {
            op: BinaryOp::And,
            left,
            ..
        } => assert!(matches!(
            left.as_ref(),
            Expr::Binary {
                op: BinaryOp::Or,
                ..
            }
        )),
        other => panic!("expected AND expression, got {other:?}"),
    }
}

#[test]
fn parses_unary_operations() {
    let query = parse_query("FOR d IN documents FILTER NOT d.deleted RETURN -1").unwrap();

    assert!(matches!(
        query.filters().first(),
        Some(Expr::Unary {
            op: UnaryOp::Not,
            ..
        })
    ));
    assert!(matches!(
        query.return_expr.unwrap(),
        Expr::Unary {
            op: UnaryOp::Neg,
            ..
        } | Expr::Number(-1.0)
    ));
}

#[test]
fn allows_line_comments() {
    let query = parse_query(
        r#"
        // start from documents
        FOR d IN documents
        // only published
        FILTER d.published == true
        RETURN d
        "#,
    )
    .unwrap();

    assert_eq!(query.filters().len(), 1);
}

#[test]
fn parses_function_call() {
    let query = parse_query("FOR d IN documents FILTER LENGTH(d.title) > 3 RETURN d").unwrap();

    assert!(matches!(
        query.filters().first(),
        Some(Expr::Binary {
            op: BinaryOp::Gt,
            ..
        })
    ));
}

#[test]
fn parses_array_literals() {
    let query =
        parse_query(r#"FOR d IN documents FILTER d.category IN ["research", "notes"] RETURN d"#)
            .unwrap();

    assert!(matches!(
        query.filters().first(),
        Some(Expr::Binary {
            op: BinaryOp::In,
            ..
        })
    ));
}

#[test]
fn parses_nested_object_and_array_projection() {
    let query = parse_query(
        r#"FOR d IN documents RETURN { doc: { id: d._id, tags: ["a", "b"] }, scores: [1, 2, 3] }"#,
    )
    .unwrap();

    match query.return_expr {
        Some(Expr::Object(fields)) => {
            assert_eq!(fields.len(), 2);
            assert_eq!(fields[0].name, "doc");
            assert_eq!(fields[1].name, "scores");
        }
        other => panic!("expected object projection, got {other:?}"),
    }
}

#[test]
fn parses_empty_object_and_array_literals() {
    let query =
        parse_query("FOR d IN documents RETURN { empty_object: {}, empty_array: [] }").unwrap();

    match query.return_expr {
        Some(Expr::Object(fields)) => {
            assert!(matches!(fields[0].value, Expr::Object(ref items) if items.is_empty()));
            assert!(matches!(fields[1].value, Expr::Array(ref items) if items.is_empty()));
        }
        other => panic!("expected object projection, got {other:?}"),
    }
}

#[test]
fn parses_escaped_string_literals() {
    let query = parse_query(r#"FOR d IN documents RETURN "quote: \" slash: \\ tab:\t""#).unwrap();

    assert_eq!(
        query.return_expr.unwrap(),
        Expr::String("quote: \" slash: \\ tab:\t".into())
    );
}

#[test]
fn parses_decimal_numbers() {
    let query = parse_query("FOR d IN documents FILTER d.score >= 0.75 RETURN d").unwrap();

    assert!(matches!(
        query.filters().first(),
        Some(Expr::Binary {
            op: BinaryOp::Ge,
            ..
        })
    ));
}

#[test]
fn collects_unique_bind_vars_in_sorted_order() {
    let query =
        parse_query("FOR d IN documents FILTER d.a == @z FILTER d.b == @a RETURN { z: @z, a: @a }")
            .unwrap();

    assert_eq!(query.bind_vars, vec!["a", "z"]);
}

#[test]
fn parses_all_json_escapes() {
    let query = parse_query(r#"FOR d IN documents RETURN "a\/b\bc\fd""#).unwrap();
    assert_eq!(
        query.return_expr.unwrap(),
        Expr::String("a/b\u{8}c\u{c}d".to_string())
    );
}
