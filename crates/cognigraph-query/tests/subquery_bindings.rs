use cognigraph_query::{ValidationError, parse_query, validate_query};

#[test]
fn nested_lifts_preserve_bind_collection_and_repeatable_parsing() {
    let source = include_str!("corpus/exec/sq_nested_binds.cgql");
    let first = parse_query(source).unwrap();
    validate_query(&first).unwrap();
    assert_eq!(first.bind_vars, vec!["increment", "offset"]);

    // A separate query must not affect later parses of the original input.
    let unrelated = parse_query("RETURN LENGTH((RETURN 1))").unwrap();
    validate_query(&unrelated).unwrap();
    assert_eq!(first, parse_query(source).unwrap());
}

#[test]
fn nested_lifts_keep_user_shadowing_and_depth_restrictions() {
    let shadowed =
        parse_query("LET n = LENGTH((RETURN 1)) RETURN (FOR n IN [1] RETURN FIRST((RETURN n)))")
            .unwrap();
    assert_eq!(
        validate_query(&shadowed),
        Err(ValidationError::DuplicateVariable("n".into()))
    );

    for depth in 1..=5 {
        let mut nested = "RETURN 1".to_string();
        for _ in 0..depth {
            nested = format!("RETURN FIRST(({nested}))");
        }
        let query = parse_query(&format!("LET n = LENGTH((RETURN 1)) {nested}")).unwrap();
        if depth <= 4 {
            validate_query(&query).unwrap();
        } else {
            assert_eq!(
                validate_query(&query),
                Err(ValidationError::SubqueryTooDeep(4))
            );
        }
    }
}
